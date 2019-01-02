#![feature(asm)]
#![allow(non_upper_case_globals)]

pub mod whvp;
pub mod time;

use std::cell::RefCell;
use crate::whvp::{Whvp, WhvpContext};
use crate::whvp::{PERM_READ, PERM_WRITE, PERM_EXECUTE};
use std::collections::HashMap;
use whvp_bindings::winhvplatform::*;

const BX_READ:    i32 = 0;
const BX_WRITE:   i32 = 1;
const BX_EXECUTE: i32 = 2;
const BX_RW:      i32 = 3;

#[repr(C)]
pub struct BochsRoutines {
    set_context:        extern fn(context: &WhvpContext),
    get_context:        extern fn(context: &mut WhvpContext),
    step_device:        extern fn(steps: u32),
    step_cpu:           extern fn(steps: u32),
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

thread_local! {
    static HYPERVISOR: RefCell<Option<Whvp>> = RefCell::new(None);
}

#[no_mangle]
pub extern "C" fn bochs_cpu_loop(routines: &BochsRoutines) {
    let mut context = WhvpContext::default();

    const TARGET_IPS: f64 = 100000000.0;
    let tickrate = time::calibrate_tsc();

    HYPERVISOR.with(|x| {
        let mut hypervisor = x.borrow_mut();

        if hypervisor.is_none() {
            print!("Creating hypervisor!\n");

            let mut new_hyp = Whvp::new();

            // Memory regions of (paddr, backing memory, size in bytes)
            let mut mem_regions: Vec<MemoryRegion> = Vec::new();
            'next_mem: for paddr in (0usize..1*1024*1024*1024).step_by(4096) {
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

            *hypervisor = Some(new_hyp);
        } else {
            //print!("Skipping hypervisor creation as it already exists\n");
        }
        let hypervisor = hypervisor.as_mut().unwrap();

        let mut emulating = 0;

        let mut vmexits = HashMap::new();
        let mut num_vmexits = 0u64;

        let mut future_report = time::rdtsc() + tickrate as u64;

        let mut start = time::rdtsc();
        let mut vm_elapsed = 0;

        loop {
            if time::rdtsc() >= future_report {
                let total_cycles = time::rdtsc() - start;
                print!("VM run percentage {:8.6}\n",
                    vm_elapsed as f64 / total_cycles as f64);
                print!("{:#?}\n", vmexits);
                future_report = time::rdtsc() + tickrate as u64;

                vm_elapsed = 0;
                start = time::rdtsc();
            }

            if emulating > 0 {
                (routines.step_cpu)(emulating);
                emulating = 0;
                continue;
            }

            //print!("Running hypervisor\n");
            (routines.get_context)(&mut context);
            hypervisor.set_context(&context);
            let pre_rip = context.rip();
            let start = time::rdtsc();
            let vmexit = hypervisor.run();
            let elapsed = time::rdtsc() - start;
            let vm_run_time = elapsed.saturating_sub(hypervisor.overhead());
            vm_elapsed += vm_run_time;
            let elapsed_secs = vm_run_time as f64 / tickrate;
            context = hypervisor.get_context();
            (routines.set_context)(&context);
            let post_rip = context.rip();

            if post_rip != pre_rip {
                let elapsed_adj_cycles = (TARGET_IPS * elapsed_secs) as u32;
                (routines.step_device)(elapsed_adj_cycles);
            }

            //print!("Hypervisor ran for {}\n", elapsed);

            if !vmexits.contains_key(&vmexit.ExitReason) {
                vmexits.insert(vmexit.ExitReason, 0u64);
            }
            *vmexits.get_mut(&vmexit.ExitReason).unwrap() += 1;
            num_vmexits += 1;

            match vmexit.ExitReason {
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonMemoryAccess => {
                    /*print!("Mem fault on PA {:016x}\n", unsafe {
                        vmexit.__bindgen_anon_1.MemoryAccess.Gpa
                    });*/

                    emulating += 1000;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64IoPortAccess => {
                    emulating += 1000;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64Halt => {
                    emulating += 1000;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonCanceled => {
                    if (unsafe { context.rflags.Reg64 } & (1 << 9)) == 0 {
                        hypervisor.register_interrupt_window();
                    }
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64InterruptWindow => {}
                _ => {
                    print!("{}\n", context);
                    panic!("Unhandled VM exit reason {}", vmexit.ExitReason);
                }
            }
        }
    });

}
