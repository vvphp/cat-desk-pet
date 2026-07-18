<div align="center">

![Cat Desk Pet](docs/banner.png)

# 🐾 Cat Desk Pet

[简体中文](README.md) · **English**

**A hand-drawn little animal that lives on your desktop.**  
Native small window · Rust state machine · SVG (resvg) · **no WebView**

</div>

---

> This repo is the **v2 native** implementation. The old fullscreen WebView / Tauri app lives in [`vvphp/desktop-cat`](https://github.com/vvphp/desktop-cat) for reference only.

## Download

Grab builds from **[Releases](../../releases)**. Unsigned is expected.

```bash
git clone https://github.com/vvphp/cat-desk-pet
cd cat-desk-pet
cargo run --release
./tools/run-pet.sh           # detach from terminal
./tools/package-macos.sh     # .app + .dmg
```

## Architecture

Tray / context menu → ~180×180 always-on-top transparent window → SVG via resvg → CALayer (macOS) / softbuffer (elsewhere) ← Rust behavior state machine.

See [`docs/perf.md`](docs/perf.md) for CPU notes.

## License

MIT
