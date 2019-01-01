/// Misc timing routines often used for benchmarking

use std::time::Instant;

/// Performs a rdtsc instruction, returns 64-bit TSC value
pub fn rdtsc() -> u64 {
    let high: u32;
    let low:  u32;

    unsafe {
        asm!("rdtsc" :
             "={edx}"(high), "={eax}"(low) :::
             "volatile", "intel");
    }

    ((high as u64) << 32) | (low as u64)
}

pub fn calibrate_tsc() -> f64 {
    let start = rdtsc();
    let start_time = Instant::now();
    loop {
        let ef = elapsed_from(&start_time);
        if ef >= 0.1 {
            return (rdtsc() - start) as f64 / ef;
        }
    }
}

/// Get elapsed time in seconds
pub fn elapsed_from(start: &Instant) -> f64 {
    let dur = start.elapsed();
    dur.as_secs() as f64 + dur.subsec_nanos() as f64 / 1_000_000_000.0
}
