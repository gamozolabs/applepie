use cpu;
use core::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};

static BOOT_TIME:  AtomicUsize = ATOMIC_USIZE_INIT;
static RDTSC_RATE: AtomicUsize = ATOMIC_USIZE_INIT;

pub fn future(microseconds: u64) -> u64
{
	cpu::rdtsc() + (microseconds * RDTSC_RATE.load(Ordering::SeqCst) as u64)
}

/// Returns system uptime in seconds as a float
pub fn uptime() -> f64
{
    rdtsc_elapsed(BOOT_TIME.load(Ordering::SeqCst) as u64)
}

pub fn rdtsc_elapsed(start_time: u64) -> f64
{
    (cpu::rdtsc() - start_time) as f64 /
        RDTSC_RATE.load(Ordering::SeqCst) as f64 / 1_000_000.0
}

pub fn sleep(microseconds: u64)
{
    let waitval = future(microseconds);
    while cpu::rdtsc() < waitval {
        unsafe { asm!("pause" :::: "volatile"); }
    }
}

pub fn rdtsc() -> u64
{
    cpu::rdtsc()
}

/// Using the PIT, determine the frequency of rdtsc. Round this frequency to
/// the nearest 100MHz.
pub unsafe fn calibrate()
{
    // Store off the current rdtsc value
    let start = cpu::rdtsc();

    print!("rdtsc at boot is: {}\n", start);

    // Store this off as the system boot time
    BOOT_TIME.store(start as usize, Ordering::SeqCst);

    let start = cpu::rdtsc();

    // Program the PIT to use mode 0 (interrupt after countdown) to
    // count down from 65535. This causes an interrupt to occur after
    // about 54.92 milliseconds (65535 / 1193182). We mask interrupts
    // from the PIT, thus we poll by sending the read back command
    // to check whether the output pin is set to 1, indicating the
    // countdown completed.
    cpu::out8(0x43, 0x30);
    cpu::out8(0x40, 0xff);
    cpu::out8(0x40, 0xff);

    loop {
        // Send the read back command to latch status on channel 0
        cpu::out8(0x43, 0xe2);

        // If the output pin is high, then we know the countdown is
        // done. Break from the loop.
        if (cpu::in8(0x40) & 0x80) != 0 {
            break;
        }
    }

    // Compute the time, in seconds, that the countdown was supposed to
    // take
    let elapsed = 65535f64 / 1193182f64;

    // Compute MHz / second for the rdtsc
    let computed_rate = ((cpu::rdtsc() - start) as f64) /
        elapsed / 1000000.0;

    // Round to the nearest 100MHz value
    let rounded_rate = (((computed_rate / 100.0) + 0.5) as u64) * 100;

    print!("CPU frequency calibrated to be {} MHz\n", rounded_rate);

    // Store the rounded rate in RDSTC_RATE
    RDTSC_RATE.store(rounded_rate as usize, Ordering::SeqCst);
}

