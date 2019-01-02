#![feature(asm)]
#![allow(non_upper_case_globals)]

pub mod whvp;
pub mod time;

use std::cell::RefCell;
use crate::whvp::{Whvp, WhvpContext};
use crate::whvp::{PERM_READ, PERM_WRITE, PERM_EXECUTE};
use whvp_bindings::winhvplatform::*;

const BX_READ:    i32 = 0;
const BX_WRITE:   i32 = 1;
const BX_EXECUTE: i32 = 2;

#[repr(C)]
pub struct BochsRoutines {
    set_context:        extern fn(context: &WhvpContext),
    get_context:        extern fn(context: &mut WhvpContext),
    step_device:        extern fn(steps: u64),
    step_cpu:           extern fn(steps: u64),
    get_memory_backing: extern fn(addr: u64, typ: i32) -> usize,
}

struct MemoryRegion {
    paddr:   usize,
    backing: usize,
    perms:   i32,
    size:    usize,
}

fn kicker(handle: usize) {
    let handle = handle as WHV_PARTITION_HANDLE;

    let tickrate = time::calibrate_tsc();
    let advance  = (tickrate / 1000.) as u64;
    print!("Advance rate calculated at {} cycles\n", advance);

    loop {
        let future = time::rdtsc() + advance;
        while time::rdtsc() < future {}
        unsafe { WHvCancelRunVirtualProcessor(handle, 0, 0); }
    }
}

// Due to bochs using longjmps we must place a lot of state in this structure
// so we don't lose it when our code suddenly gets hijacked and reenters from
// the start
#[derive(Default)]
struct PersistState {
    hypervisor:       Option<Whvp>,
    tickrate:         Option<f64>,
    future_report:    u64,
    start:            u64,
    vm_elapsed:       u64,
    last_sync_cycles: u64,
}

thread_local! {
    static PERSIST: RefCell<PersistState> = RefCell::new(Default::default());
}

#[no_mangle]
pub extern "C" fn bochs_cpu_loop(routines: &BochsRoutines, pmem_size: u64) {
    assert!(pmem_size & 0xfff == 0,
        "Physical memory size was not 4 KiB aligned");

    const TARGET_IPS: f64 = 1000000.0;

    PERSIST.with(|x| {
        let mut persist = x.borrow_mut();

        // Create a context to be used for all register sync operations
        let mut context = WhvpContext::default();

        if persist.tickrate.is_none() {
            persist.tickrate = Some(time::calibrate_tsc());
        }

        if persist.hypervisor.is_none() {
            print!("Creating hypervisor!\n");

            let mut new_hyp = Whvp::new();

            // Memory regions of (paddr, backing memory, size in bytes)
            let mut mem_regions: Vec<MemoryRegion> = Vec::new();
            'next_mem: for paddr in (0usize..pmem_size as usize).step_by(4096) {
                // Get the bochs memory backing for this physical address
                let backing_read    = (routines.get_memory_backing)(paddr as u64, BX_READ);
                let backing_write   = (routines.get_memory_backing)(paddr as u64, BX_WRITE);
                let backing_execute = (routines.get_memory_backing)(paddr as u64, BX_EXECUTE);

                // Skip unmapped memory
                if backing_read == 0 && backing_write == 0 &&
                    backing_execute == 0 { continue; }

                let mut backing = None;
                let mut perms   = 0;

                if backing_read != 0 {
                    if let Some(backing) = backing {
                        if backing == backing_read {
                            perms |= PERM_READ;
                        }
                    } else {
                        backing = Some(backing_read);
                        perms |= PERM_READ;
                    }
                }

                if backing_write != 0 {
                    if let Some(backing) = backing {
                        if backing == backing_write {
                            perms |= PERM_WRITE;
                        }
                    } else {
                        backing = Some(backing_write);
                        perms |= PERM_WRITE;
                    }
                }

                if backing_execute != 0 {
                    if let Some(backing) = backing {
                        if backing == backing_execute {
                            perms |= PERM_EXECUTE;
                        }
                    } else {
                        backing = Some(backing_execute);
                        perms |= PERM_EXECUTE;
                    }
                }

                // Don't map BIOS memory. There's some weird read/write states
                // we cannot accurately reflect with EPT.
                if paddr >= 0x000c0000 && paddr < 0x00100000 {
                    continue;
                }

                // Must be filled in by now so we can unwrap
                let backing = backing.unwrap();

                // Attempt to merge this region into another if it's contiguous
                for mr in mem_regions.iter_mut() {
                    if mr.perms == perms && mr.paddr + mr.size == paddr && mr.backing + mr.size == backing {
                        // Allow contiguous memory in both physical and backing memory
                        // to be combined
                        mr.size += 4096;
                        continue 'next_mem;
                    }
                }

                // Region does not extend an existing region, create a new one!
                mem_regions.push(MemoryRegion {
                    paddr:   paddr,
                    backing: backing,
                    size:    4096,
                    perms:   perms,
                });
            }

            // List all the memory regions
            for mr in &mem_regions {
                print!("Memory region: start {:016x} end {:016x} backing {:016x} perm {:02x}\n",
                    mr.paddr, mr.paddr + mr.size - 1, mr.backing, mr.perms);

                // Should never happen
                assert!(mr.size > 0 && mr.backing > 0);

                let sliced = unsafe {
                    std::slice::from_raw_parts_mut(mr.backing as *mut u8, mr.size)
                };

                new_hyp.map_memory(mr.paddr, sliced, mr.perms);
            }

            let handle = new_hyp.handle() as usize;
            std::thread::spawn(move || kicker(handle));

            persist.hypervisor = Some(new_hyp);

            persist.future_report = time::rdtsc() + persist.tickrate.unwrap() as u64;
            persist.start = time::rdtsc();
            persist.vm_elapsed = 0;
            persist.last_sync_cycles = time::rdtsc();
        }

        let mut emulating = 0;

        loop {
            {
                let current_time = time::rdtsc();
                let elapsed_cycles = current_time
                    .saturating_sub(persist.last_sync_cycles);
                persist.last_sync_cycles = current_time;
                let elapsed_secs = elapsed_cycles as f64 / persist.tickrate.unwrap();

                let elapsed_adj_cycles = (TARGET_IPS * elapsed_secs) as u64;
                (routines.step_device)(elapsed_adj_cycles);
            }

            if time::rdtsc() >= persist.future_report {
                let total_cycles = time::rdtsc() - persist.start;

                print!("VM run percentage {:8.6} | Uptime {:14.6}\n",
                    persist.vm_elapsed as f64 / total_cycles as f64,
                    total_cycles as f64 / persist.tickrate.unwrap());

                persist.future_report = time::rdtsc() + persist.tickrate.unwrap() as u64;
            }

            if emulating > 0 {
                (routines.step_cpu)(emulating);
                emulating = 0;
                continue;
            }

            // Sync bochs register state to hypervisor register state
            (routines.get_context)(&mut context);
            persist.hypervisor.as_mut().unwrap().set_context(&context);

            let vmstart_cycles = time::rdtsc();
            let vmexit = persist.hypervisor.as_mut().unwrap().run();
            let elapsed = time::rdtsc() - vmstart_cycles;
            let vm_run_time = elapsed.saturating_sub(persist.hypervisor.as_mut().unwrap().overhead());
            persist.vm_elapsed += vm_run_time;

            // Sync hypervisor register state to bochs register state
            context = persist.hypervisor.as_mut().unwrap().get_context();
            (routines.set_context)(&context);

            if vm_run_time > 1000000000 {
                print!("Ran for a really long time\n");
            }

            match vmexit.ExitReason {
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonMemoryAccess => {
                    // Emulate MMIO
                    emulating = 100;
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64IoPortAccess => {
                    // Emulate I/O
                    emulating = 100;
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64Halt => {
                    // Emulate halts
                    emulating = 100;
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonCanceled => {
                    if (unsafe { context.rflags.Reg64 } & (1 << 9)) == 0 {
                        // If interrupts are masked request to be notified next
                        // time they are enabled
                        persist.hypervisor.as_mut().unwrap().register_interrupt_window();
                    }
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonInvalidVpRegisterValue => {
                    // This was observed in Windows 7, however if we emulate a
                    // bit the issue seems to go away. So that's our "solution"
                    print!("Warning: Invalid VP state, emulating for a bit\n");
                    emulating = 100;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64InterruptWindow => {
                    // We got an interrupt window! Well we don't have to do
                    // anything as now bochs knows it can deliver async events
                    // and will when we `step_device()`
                }
                _ => {
                    // Hard panic on unhandled vmexits. This will dump the
                    // context and print the reason. These will probably be
                    // common for a while until we test more and more OSes under
                    // this hypervisor.
                    print!("{}\n", context);
                    panic!("Unhandled VM exit reason {}", vmexit.ExitReason);
                }
            }
        }
    });

}
