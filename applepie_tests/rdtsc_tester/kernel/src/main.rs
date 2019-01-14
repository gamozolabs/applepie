#![feature(global_asm, asm, panic_info_message)]
#![feature(alloc, lang_items)]
#![no_std]
#![no_main]

#[macro_use] extern crate alloc;

pub mod core_reqs;
#[macro_use] pub mod disp;
pub mod cpu;
pub mod time;
pub mod mm;
pub mod interrupts;

use core::panic::PanicInfo;

#[lang = "oom"]
#[no_mangle]
pub extern fn rust_oom(_layout: alloc::alloc::Layout) -> ! {
    panic!("Out of memory");
}

/// Global allocator
#[global_allocator]
pub static mut GLOBAL_ALLOCATOR: mm::GlobalAllocator = mm::GlobalAllocator {};

/// Panic implementation
#[panic_handler]
#[no_mangle]
pub fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        print!("\0\x0c!!! PANIC !!! {}:{} ",
            location.file(), location.line(),);
    } else {
        print!("\0\x0c!!! PANIC !!! Panic with no location info ");
    }

    if let Some(&args) = info.message() {
        use core::fmt::write;
        let _ = write(&mut disp::Writer, args);
        print!("\n");
    } else {
        print!("No arguments\n");
    }

    cpu::halt();
}

/// Main entry point for this codebase
#[no_mangle]
pub extern fn entry() -> ! {
    print!("Unnamed OS booting... version 0.1\n");
    unsafe { time::calibrate(); }
    interrupts::init_interrupts();

    print!("BOOTED!\n");

    let mut last = time::rdtsc();
    for ii in 0u64.. {
        let new = time::rdtsc();

        if last >= new {
            panic!("OH NO NON MONOTONIC RDTSC\n\
                    Old was {}\n\
                    New was {}",
                last, new);
        }

        last = new;
    }

    // Halt forever
    cpu::halt();
}
