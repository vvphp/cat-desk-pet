//! Feature-gated wgpu renderer for the A/B experiment in issue #6.

use std::mem::size_of;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::atlas;
use crate::pet::{FlyerKind, Mode, ToyKind};
use crate::renderer::{
    FrameKey, FrameViewport, NativeRenderer, RenderOutcome, RenderSnapshot, Renderer,
};
use crate::sprite;

const MAX_ATLAS_LAYERS: usize = 32;
const MAX_PRIMITIVE_VERTICES: usize = 16_384;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ViewUniform {
    size: [f32; 2],
    flash: f32,
    _padding: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AtlasInstance {
    local_rect: [f32; 4],
    atlas_rect: [f32; 4],
    matrix: [f32; 4],
    translation: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PrimitiveVertex {
    position: [f32; 2],
    color: [f32; 4],
}

struct FallbackFrame {
    native: NativeRenderer,
    texture: Option<wgpu::Texture>,
    bind_group: Option<wgpu::BindGroup>,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

struct OffscreenFrame {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl OffscreenFrame {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cat premultiplied frame"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cat premultiplied frame bind group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        Self {
            _texture: texture,
            view,
            bind_group,
        }
    }
}

impl FallbackFrame {
    fn new() -> Self {
        Self {
            native: NativeRenderer::new(),
            texture: None,
            bind_group: None,
            width: 0,
            height: 0,
            rgba: Vec::new(),
        }
    }

    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        snapshot: &RenderSnapshot<'_>,
        viewport: FrameViewport,
    ) {
        self.native.render(snapshot, viewport);
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        if self.texture.is_none() || self.width != width || self.height != height {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("cat fallback texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cat fallback bind group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            });
            self.texture = Some(texture);
            self.bind_group = Some(bind_group);
            self.width = width;
            self.height = height;
        }

        let pixels = self.native.pixels();
        self.rgba.resize((width * height * 4) as usize, 0);
        for (target, pixel) in self.rgba.chunks_exact_mut(4).zip(pixels.iter().copied()) {
            target[0] = ((pixel >> 16) & 0xff) as u8;
            target[1] = ((pixel >> 8) & 0xff) as u8;
            target[2] = (pixel & 0xff) as u8;
            target[3] = ((pixel >> 24) & 0xff) as u8;
        }
        queue.write_texture(
            self.texture
                .as_ref()
                .expect("created above")
                .as_image_copy(),
            &self.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
}

pub struct WgpuRenderer {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    atlas_pipeline: wgpu::RenderPipeline,
    primitive_pipeline: wgpu::RenderPipeline,
    fallback_pipeline: wgpu::RenderPipeline,
    surface_pipeline: wgpu::RenderPipeline,
    atlas_bind_group: wgpu::BindGroup,
    view_bind_group: wgpu::BindGroup,
    fallback_layout: wgpu::BindGroupLayout,
    surface_layout: wgpu::BindGroupLayout,
    fallback_sampler: wgpu::Sampler,
    view_buffer: wgpu::Buffer,
    atlas_instance_buffer: wgpu::Buffer,
    primitive_buffer: wgpu::Buffer,
    palette_texture: wgpu::Texture,
    layer_count: u32,
    primitive_before_count: u32,
    primitive_total_count: u32,
    fallback: Option<FallbackFrame>,
    offscreen: Option<OffscreenFrame>,
    last_frame_key: Option<FrameKey>,
    direct_frame: bool,
    needs_unpremultiply: bool,
    reported_direct: Option<bool>,
}

impl WgpuRenderer {
    pub fn new(window: Arc<Window>) -> Result<Self, String> {
        let backends = if cfg!(target_os = "macos") {
            wgpu::Backends::METAL
        } else if cfg!(target_os = "windows") {
            wgpu::Backends::DX12
        } else {
            wgpu::Backends::PRIMARY
        };
        let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_descriptor.backends = backends;
        let instance = wgpu::Instance::new(instance_descriptor);
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| format!("create wgpu surface: {error}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
        }))
        .map_err(|error| format!("request wgpu adapter: {error}"))?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cat wgpu device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| format!("request wgpu device: {error}"))?;

        let capabilities = surface.get_capabilities(&adapter);
        let alpha_mode = [
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
        ]
        .into_iter()
        .find(|mode| capabilities.alpha_modes.contains(mode))
        .ok_or_else(|| {
            format!(
                "wgpu surface has no transparent alpha mode: {:?}",
                capabilities.alpha_modes
            )
        })?;
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("wgpu surface has no format")?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cat indexed atlas"),
            size: wgpu::Extent3d {
                width: atlas::ATLAS_WIDTH,
                height: atlas::ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            atlas_texture.as_image_copy(),
            atlas::pixels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas::ATLAS_WIDTH * 2),
                rows_per_image: Some(atlas::ATLAS_HEIGHT),
            },
            wgpu::Extent3d {
                width: atlas::ATLAS_WIDTH,
                height: atlas::ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        let palette_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cat palette"),
            size: wgpu::Extent3d {
                width: 32,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cat atlas layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let palette_view = palette_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cat atlas bind group"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&palette_view),
                },
            ],
        });

        let view_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cat view uniform"),
            contents: bytemuck::bytes_of(&ViewUniform {
                size: [config.width as f32, config.height as f32],
                flash: 0.0,
                _padding: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cat view layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let view_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cat view bind group"),
            layout: &view_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view_buffer.as_entire_binding(),
            }],
        });

        let fallback_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cat fallback layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let fallback_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("cat fallback sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let surface_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cat surface conversion layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let atlas_pipeline = create_atlas_pipeline(&device, format, &atlas_layout, &view_layout);
        let primitive_pipeline = create_primitive_pipeline(&device, format, &view_layout);
        let fallback_pipeline =
            create_fallback_pipeline(&device, format, &fallback_layout, &view_layout);
        let surface_pipeline = create_surface_pipeline(&device, format, &surface_layout);
        let needs_unpremultiply = alpha_mode == wgpu::CompositeAlphaMode::PostMultiplied;
        let offscreen = needs_unpremultiply.then(|| {
            OffscreenFrame::new(
                &device,
                &surface_layout,
                format,
                config.width,
                config.height,
            )
        });
        let atlas_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cat atlas instances"),
            size: (MAX_ATLAS_LAYERS * size_of::<AtlasInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let primitive_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cat primitive vertices"),
            size: (MAX_PRIMITIVE_VERTICES * size_of::<PrimitiveVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        eprintln!(
            "cat-desk-pet: renderer=wgpu adapter={:?} alpha={alpha_mode:?} format={format:?}",
            info.name
        );
        Ok(Self {
            _instance: instance,
            surface,
            device,
            queue,
            config,
            atlas_pipeline,
            primitive_pipeline,
            fallback_pipeline,
            surface_pipeline,
            atlas_bind_group,
            view_bind_group,
            fallback_layout,
            surface_layout,
            fallback_sampler,
            view_buffer,
            atlas_instance_buffer,
            primitive_buffer,
            palette_texture,
            layer_count: 0,
            primitive_before_count: 0,
            primitive_total_count: 0,
            fallback: None,
            offscreen,
            last_frame_key: None,
            direct_frame: false,
            needs_unpremultiply,
            reported_direct: None,
        })
    }

    pub fn present(&mut self, viewport: FrameViewport) -> Result<bool, String> {
        self.ensure_surface_size(viewport.width, viewport.height);
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(texture)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
                    other => {
                        return Err(format!("acquire wgpu surface after reconfigure: {other:?}"))
                    }
                }
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("wgpu surface validation error".to_owned());
            }
        };
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cat frame encoder"),
            });
        {
            let target_view = self
                .offscreen
                .as_ref()
                .map(|frame| &frame.view)
                .unwrap_or(&surface_view);
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cat transparent pass"),
                color_attachments: &attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if self.direct_frame {
                if self.primitive_before_count > 0 {
                    pass.set_pipeline(&self.primitive_pipeline);
                    pass.set_bind_group(0, &self.view_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.primitive_buffer.slice(..));
                    pass.draw(0..self.primitive_before_count, 0..1);
                }
                if self.layer_count > 0 {
                    pass.set_pipeline(&self.atlas_pipeline);
                    pass.set_bind_group(0, &self.atlas_bind_group, &[]);
                    pass.set_bind_group(1, &self.view_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.atlas_instance_buffer.slice(..));
                    pass.draw(0..6, 0..self.layer_count);
                }
                if self.primitive_total_count > self.primitive_before_count {
                    pass.set_pipeline(&self.primitive_pipeline);
                    pass.set_bind_group(0, &self.view_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.primitive_buffer.slice(..));
                    pass.draw(
                        self.primitive_before_count..self.primitive_total_count,
                        0..1,
                    );
                }
            } else if let Some(bind_group) = self
                .fallback
                .as_ref()
                .and_then(|fallback| fallback.bind_group.as_ref())
            {
                pass.set_pipeline(&self.fallback_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.set_bind_group(1, &self.view_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        if self.needs_unpremultiply {
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &surface_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cat postmultiplied surface pass"),
                color_attachments: &attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.surface_pipeline);
            pass.set_bind_group(
                0,
                &self
                    .offscreen
                    .as_ref()
                    .expect("postmultiplied surface needs offscreen frame")
                    .bind_group,
                &[],
            );
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(surface_texture);
        Ok(true)
    }

    fn ensure_surface_size(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        if self.needs_unpremultiply {
            self.offscreen = Some(OffscreenFrame::new(
                &self.device,
                &self.surface_layout,
                self.config.format,
                width,
                height,
            ));
        }
    }
}

impl Renderer for WgpuRenderer {
    fn render(&mut self, snapshot: &RenderSnapshot<'_>, viewport: FrameViewport) -> RenderOutcome {
        let frame_key = FrameKey::new(snapshot, viewport);
        if self.last_frame_key == Some(frame_key) {
            return RenderOutcome {
                frame_key,
                rasterized: false,
            };
        }
        let direct_blocker = direct_blocker(snapshot);
        self.direct_frame = direct_blocker.is_none();
        if self.reported_direct != Some(self.direct_frame) {
            if let Some(blocker) = direct_blocker {
                eprintln!("cat-desk-pet: wgpu path=native-upload-fallback reason={blocker}");
            } else {
                eprintln!("cat-desk-pet: wgpu path=atlas-direct");
            }
            self.reported_direct = Some(self.direct_frame);
        }
        let view = ViewUniform {
            size: [viewport.width.max(1) as f32, viewport.height.max(1) as f32],
            flash: if self.direct_frame {
                snapshot.flash.clamp(0.0, 1.0) as f32
            } else {
                0.0
            },
            _padding: 0.0,
        };
        self.queue
            .write_buffer(&self.view_buffer, 0, bytemuck::bytes_of(&view));

        if self.direct_frame {
            let instances = atlas_instances(snapshot, viewport);
            assert!(instances.len() <= MAX_ATLAS_LAYERS);
            self.queue.write_buffer(
                &self.atlas_instance_buffer,
                0,
                bytemuck::cast_slice(&instances),
            );
            self.layer_count = instances.len() as u32;
            let palette = atlas::palette_texture(snapshot.coat);
            self.queue.write_texture(
                self.palette_texture.as_image_copy(),
                bytemuck::cast_slice(&palette),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(32 * 4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: 32,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            let primitives = build_primitives(snapshot, viewport);
            let mut vertices = primitives.before;
            self.primitive_before_count = vertices.len() as u32;
            vertices.extend(primitives.after);
            assert!(vertices.len() <= MAX_PRIMITIVE_VERTICES);
            if !vertices.is_empty() {
                self.queue
                    .write_buffer(&self.primitive_buffer, 0, bytemuck::cast_slice(&vertices));
            }
            self.primitive_total_count = vertices.len() as u32;
        } else {
            let fallback = self.fallback.get_or_insert_with(FallbackFrame::new);
            fallback.update(
                &self.device,
                &self.queue,
                &self.fallback_layout,
                &self.fallback_sampler,
                snapshot,
                viewport,
            );
            self.layer_count = 0;
            self.primitive_before_count = 0;
            self.primitive_total_count = 0;
        }
        self.last_frame_key = Some(frame_key);
        RenderOutcome {
            frame_key,
            rasterized: true,
        }
    }
}

#[cfg(test)]
fn supports_direct(snapshot: &RenderSnapshot<'_>) -> bool {
    direct_blocker(snapshot).is_none()
}

fn direct_blocker(snapshot: &RenderSnapshot<'_>) -> Option<&'static str> {
    if snapshot.feed.is_some() {
        return Some("feed");
    }
    if snapshot.gift.is_some() {
        return Some("gift");
    }
    if snapshot.bubble.is_some() {
        return Some("bubble");
    }
    if !snapshot.particles.is_empty() {
        return Some("particles");
    }
    if snapshot.toy.is_some_and(|toy| toy.kind != ToyKind::Laser) {
        return Some("toy");
    }
    if snapshot
        .flyer
        .is_some_and(|flyer| flyer.kind != FlyerKind::Bird)
    {
        return Some("flyer");
    }
    None
}

fn atlas_instances(snapshot: &RenderSnapshot<'_>, viewport: FrameViewport) -> Vec<AtlasInstance> {
    let scale = viewport.scale.max(0.01);
    let center_x = snapshot.x - viewport.origin_x;
    let center_y = snapshot.y - viewport.origin_y;
    let facing = if snapshot.facing < 0.0 { -1.0 } else { 1.0 };
    let bob = sprite::bob(snapshot);
    sprite::atlas_layer_plan(snapshot)
        .into_iter()
        .map(|layer| {
            let transform = layer.transform;
            let angle = transform.rotation_deg.to_radians();
            let (sin, cos) = angle.sin_cos();
            let layer_tx = transform.pivot_x - cos * transform.pivot_x
                + sin * transform.pivot_y
                + transform.translate_x;
            let layer_ty = transform.pivot_y - sin * transform.pivot_x - cos * transform.pivot_y
                + transform.translate_y;
            let matrix = [
                (facing * cos * scale) as f32,
                (facing * -sin * scale) as f32,
                (sin * scale) as f32,
                (cos * scale) as f32,
            ];
            let translation = [
                ((center_x - facing * 60.0 + facing * layer_tx) * scale) as f32,
                ((center_y + bob - 60.5 + layer_ty) * scale) as f32,
            ];
            AtlasInstance {
                local_rect: [
                    (layer.region.source_x as f64 / atlas::ATLAS_SCALE) as f32,
                    (layer.region.source_y as f64 / atlas::ATLAS_SCALE) as f32,
                    (layer.region.width as f64 / atlas::ATLAS_SCALE) as f32,
                    (layer.region.height as f64 / atlas::ATLAS_SCALE) as f32,
                ],
                atlas_rect: [
                    layer.region.x as f32,
                    layer.region.y as f32,
                    layer.region.width as f32,
                    layer.region.height as f32,
                ],
                matrix,
                translation,
            }
        })
        .collect()
}

struct PrimitiveScene {
    before: Vec<PrimitiveVertex>,
    after: Vec<PrimitiveVertex>,
}

struct PrimitiveBuilder {
    scale: f64,
    before: Vec<PrimitiveVertex>,
    after: Vec<PrimitiveVertex>,
}

impl PrimitiveBuilder {
    fn new(scale: f64) -> Self {
        Self {
            scale: scale.max(0.01),
            before: Vec::new(),
            after: Vec::new(),
        }
    }

    fn finish(self) -> PrimitiveScene {
        PrimitiveScene {
            before: self.before,
            after: self.after,
        }
    }

    fn color(rgb: [u8; 3], alpha: u8) -> [f32; 4] {
        [
            rgb[0] as f32 / 255.0,
            rgb[1] as f32 / 255.0,
            rgb[2] as f32 / 255.0,
            alpha as f32 / 255.0,
        ]
    }

    fn target(&mut self, before: bool) -> &mut Vec<PrimitiveVertex> {
        if before {
            &mut self.before
        } else {
            &mut self.after
        }
    }

    fn ellipse(
        &mut self,
        before: bool,
        center: [f64; 2],
        radius: [f64; 2],
        rgb: [u8; 3],
        alpha: u8,
    ) {
        const SEGMENTS: usize = 28;
        let scale = self.scale;
        let center = [center[0] * scale, center[1] * scale];
        let radius = [radius[0] * scale, radius[1] * scale];
        let color = Self::color(rgb, alpha);
        let target = self.target(before);
        for index in 0..SEGMENTS {
            let a0 = index as f64 * std::f64::consts::TAU / SEGMENTS as f64;
            let a1 = (index + 1) as f64 * std::f64::consts::TAU / SEGMENTS as f64;
            target.extend([
                PrimitiveVertex {
                    position: [center[0] as f32, center[1] as f32],
                    color,
                },
                PrimitiveVertex {
                    position: [
                        (center[0] + a0.cos() * radius[0]) as f32,
                        (center[1] + a0.sin() * radius[1]) as f32,
                    ],
                    color,
                },
                PrimitiveVertex {
                    position: [
                        (center[0] + a1.cos() * radius[0]) as f32,
                        (center[1] + a1.sin() * radius[1]) as f32,
                    ],
                    color,
                },
            ]);
        }
    }

    fn rect(&mut self, before: bool, origin: [f64; 2], size: [f64; 2], rgb: [u8; 3], alpha: u8) {
        let scale = self.scale;
        let x0 = (origin[0] * scale) as f32;
        let y0 = (origin[1] * scale) as f32;
        let x1 = ((origin[0] + size[0]) * scale) as f32;
        let y1 = ((origin[1] + size[1]) * scale) as f32;
        let color = Self::color(rgb, alpha);
        self.target(before).extend([
            PrimitiveVertex {
                position: [x0, y0],
                color,
            },
            PrimitiveVertex {
                position: [x1, y0],
                color,
            },
            PrimitiveVertex {
                position: [x0, y1],
                color,
            },
            PrimitiveVertex {
                position: [x0, y1],
                color,
            },
            PrimitiveVertex {
                position: [x1, y0],
                color,
            },
            PrimitiveVertex {
                position: [x1, y1],
                color,
            },
        ]);
    }

    fn triangle(&mut self, before: bool, points: [[f64; 2]; 3], rgb: [u8; 3], alpha: u8) {
        let scale = self.scale;
        let color = Self::color(rgb, alpha);
        self.target(before)
            .extend(points.map(|point| PrimitiveVertex {
                position: [(point[0] * scale) as f32, (point[1] * scale) as f32],
                color,
            }));
    }

    fn line(
        &mut self,
        before: bool,
        from: [f64; 2],
        to: [f64; 2],
        thickness: f64,
        rgb: [u8; 3],
        alpha: u8,
    ) {
        let dx = to[0] - from[0];
        let dy = to[1] - from[1];
        let length = (dx * dx + dy * dy).sqrt();
        if length < 0.01 {
            return;
        }
        let nx = -dy / length * thickness;
        let ny = dx / length * thickness;
        let points = [
            [from[0] + nx, from[1] + ny],
            [to[0] + nx, to[1] + ny],
            [from[0] - nx, from[1] - ny],
            [to[0] - nx, to[1] - ny],
        ];
        self.triangle(before, [points[0], points[1], points[2]], rgb, alpha);
        self.triangle(before, [points[2], points[1], points[3]], rgb, alpha);
    }
}

fn build_primitives(snapshot: &RenderSnapshot<'_>, viewport: FrameViewport) -> PrimitiveScene {
    let mut builder = PrimitiveBuilder::new(viewport.scale);
    let local = |x: f64, y: f64| [x - viewport.origin_x, y - viewport.origin_y];
    let center = local(snapshot.x, snapshot.y);

    if matches!(snapshot.mode, Mode::InBed | Mode::GoingHome) {
        let home = local(snapshot.home_x, snapshot.home_y + 18.0);
        builder.ellipse(
            true,
            [home[0], home[1] + 6.0],
            [58.0, 16.0],
            [0x6B, 0x4E, 0x3A],
            255,
        );
        builder.ellipse(true, home, [48.0, 12.0], [0xC4, 0xA4, 0x84], 255);
        builder.ellipse(
            true,
            [home[0] - 6.0, home[1] - 2.0],
            [18.0, 4.0],
            [0xE8, 0xD4, 0xB8],
            255,
        );
    }
    if matches!(snapshot.mode, Mode::Sleeping | Mode::InBed) {
        let alpha = (((snapshot.sleep_t * 1.2).sin() * 0.5 + 0.5) * 220.0) as u8;
        let x = center[0] + 36.0;
        let y = center[1] - 36.0 + sprite::bob(snapshot);
        builder.rect(false, [x, y], [10.0, 2.0], [0x55, 0x44, 0x66], alpha);
        builder.rect(
            false,
            [x + 2.0, y + 5.0],
            [10.0, 2.0],
            [0x55, 0x44, 0x66],
            alpha,
        );
        for index in 0..8 {
            builder.rect(
                false,
                [x + 8.0 - index as f64, y + 1.0 + index as f64 * 0.6],
                [2.0, 2.0],
                [0x55, 0x44, 0x66],
                alpha,
            );
        }
    }
    if let Some(toy) = snapshot.toy.filter(|toy| toy.kind == ToyKind::Laser) {
        let points: Vec<_> = snapshot.laser_trail.iter().copied().collect();
        for index in 1..points.len() {
            let opacity = ((index as f64 / points.len() as f64) * 0.55 * 255.0) as u8;
            builder.line(
                false,
                local(points[index - 1].x, points[index - 1].y),
                local(points[index].x, points[index].y),
                1.0 + index as f64 * 0.2,
                [0xFF, 0x35, 0x35],
                opacity,
            );
        }
        let laser = local(toy.x, toy.y);
        let pulse = 1.0 + (toy.age * 28.0).sin() * 0.18;
        let radius = 5.0 * pulse;
        builder.ellipse(
            false,
            laser,
            [radius + 3.0, radius + 3.0],
            [0xFF, 0x80, 0x80],
            255,
        );
        builder.ellipse(false, laser, [radius, radius], [0xFF, 0x35, 0x35], 255);
        builder.ellipse(
            false,
            [laser[0] - 1.0, laser[1] - 1.0],
            [1.8, 1.8],
            [0xFF, 0xC8, 0xC8],
            255,
        );
    }
    if let Some(flyer) = snapshot.flyer.filter(|flyer| flyer.kind == FlyerKind::Bird) {
        let bird = local(flyer.x, flyer.y);
        let direction = if flyer.vx >= 0.0 { 1.0 } else { -1.0 };
        builder.ellipse(false, bird, [10.0, 7.0], [0x5A, 0x8A, 0xC8], 255);
        builder.ellipse(
            false,
            [bird[0] + direction * 8.0, bird[1] - 1.0],
            [5.0, 4.5],
            [0x5A, 0x8A, 0xC8],
            255,
        );
        builder.triangle(
            false,
            [
                [bird[0] + direction * 12.0, bird[1]],
                [bird[0] + direction * 18.0, bird[1] - 2.0],
                [bird[0] + direction * 18.0, bird[1] + 2.0],
            ],
            [0xE8, 0xA0, 0x40],
            255,
        );
        builder.ellipse(
            false,
            [bird[0] - direction * 4.0, bird[1] - 6.0],
            [7.0, 3.0],
            [0x4A, 0x70, 0xA8],
            255,
        );
    }
    builder.finish()
}

fn create_atlas_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    atlas_layout: &wgpu::BindGroupLayout,
    view_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x4,
        1 => Float32x4,
        2 => Float32x4,
        3 => Float32x2
    ];
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/atlas.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cat atlas pipeline layout"),
        bind_group_layouts: &[Some(atlas_layout), Some(view_layout)],
        immediate_size: 0,
    });
    create_pipeline(
        device,
        "cat atlas pipeline",
        &shader,
        &layout,
        format,
        &[Some(wgpu::VertexBufferLayout {
            array_stride: size_of::<AtlasInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        })],
    )
}

fn create_primitive_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    view_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/primitive.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cat primitive pipeline layout"),
        bind_group_layouts: &[Some(view_layout)],
        immediate_size: 0,
    });
    create_pipeline(
        device,
        "cat primitive pipeline",
        &shader,
        &layout,
        format,
        &[Some(wgpu::VertexBufferLayout {
            array_stride: size_of::<PrimitiveVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        })],
    )
}

fn create_fallback_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    fallback_layout: &wgpu::BindGroupLayout,
    view_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/fallback.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cat fallback pipeline layout"),
        bind_group_layouts: &[Some(fallback_layout), Some(view_layout)],
        immediate_size: 0,
    });
    create_pipeline(
        device,
        "cat fallback pipeline",
        &shader,
        &layout,
        format,
        &[],
    )
}

fn create_surface_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    surface_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/surface.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("cat surface conversion pipeline layout"),
        bind_group_layouts: &[Some(surface_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("cat surface conversion pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_pipeline(
    device: &wgpu::Device,
    label: &'static str,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers,
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::Pet;

    #[test]
    fn forced_benchmark_scenes_use_the_direct_path() {
        let mut pet = Pet::new(1440.0, 900.0);
        for mode in [Mode::Sleeping, Mode::Idle, Mode::Walking] {
            pet.mode = mode;
            let snapshot = RenderSnapshot::from(&pet);
            assert!(supports_direct(&snapshot), "{mode:?}");
        }
        pet.spawn_bird_flyby();
        pet.spawn_toy(ToyKind::Laser);
        assert!(supports_direct(&RenderSnapshot::from(&pet)));
    }

    #[test]
    fn forced_stress_scene_stays_on_the_direct_path() {
        let mut pet = Pet::new(1440.0, 900.0);
        pet.force_stress_scene();
        for _ in 0..600 {
            pet.update(1.0 / 30.0);
            assert!(supports_direct(&RenderSnapshot::from(&pet)));
        }
    }

    #[test]
    fn unsupported_content_selects_the_visual_fallback() {
        let mut pet = Pet::new(1440.0, 900.0);
        pet.spawn_toy(ToyKind::Ball);
        assert!(!supports_direct(&RenderSnapshot::from(&pet)));
    }
}
