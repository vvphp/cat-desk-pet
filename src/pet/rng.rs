//! Tiny xorshift RNG (process-seeded; tests call seed()).

use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};

thread_local! {
    // 0 → seed lazily from process start so ambient events aren't identical
    // every launch. Tests call `seed` for determinism.
    static S: Cell<u64> = const { Cell::new(0) };
}

#[cfg(test)]
pub fn seed(seed: u64) {
    S.with(|s| s.set(if seed == 0 { 1 } else { seed }));
}

pub fn next_u64() -> u64 {
    S.with(|s| {
        let mut x = s.get();
        if x == 0 {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0xC0FFEE);
            x = nanos ^ ((std::process::id() as u64).wrapping_shl(32)) ^ 0xA5A5_5A5A;
            if x == 0 {
                x = 0xC0FFEE;
            }
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        x
    })
}
