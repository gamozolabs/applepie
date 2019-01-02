#![feature(asm)]
#![allow(non_upper_case_globals)]

pub mod whvp;
pub mod time;

use std::cell::RefCell;
use crate::whvp::{Whvp, WhvpContext};
use crate::whvp::{PERM_READ, PERM_WRITE, PERM_EXECUTE};
use whvp_bindings::winhvplatform::*;

// Bochs permissions used for `get_memory_backing`
const BX_READ:    i32 = 0;
const BX_WRITE:   i32 = 1;
const BX_EXECUTE: i32 = 2;

/// Routines passed by Bochs to use for manipulating the Bochs state
#[repr(C)]
pub struct BochsRoutines {
    /// Set the Bochs context to the `context` provided
    set_context: extern fn(context: &WhvpContext),

    /// Get the Bochs context into the `context` provided
    get_context: extern fn(context: &mut WhvpContext),

    /// Step the device emulation portion of Bochs by `steps`. For example if
    /// ips=1000000 in your bochsrc and you pass 1000000 as `steps`, this will
    /// effectively emulate 1 second of hardware/timer/interrupts
    step_device: extern fn(steps: u64),

    /// Step the CPU by `steps`. Depending on the Bochs optimization features
    /// this either steps `steps` instructions, or `steps` 'chains' which are
    /// Bochs's linked instructions (similar to a basic block)
    step_cpu: extern fn(steps: u64),

    /// Get the backing address of a physical address `addr` with an access type
    /// `typ` from Bochs. The access type should be a combination of the
    /// `BX_READ`, `BX_WRITE`, and `BX_EXECUTE` constants
    get_memory_backing: extern fn(addr: u64, typ: i32) -> usize,
}

/// Named structure for tracking memory regions in Bochs
/// 
/// This is just for convience
struct MemoryRegion {
    /// Physical address of the base of this memory region
    paddr: usize,

    /// Pointer to memory which represents this region in bochs
    backing: usize,

    /// Permissions allowed on this region
    /// These are the WHVP constants: `PERM_READ`, `PERM_WRITE`, `PERM_EXECUTE`
    perms: i32,

    /// Size of the memory region
    size: usize,
}

/// Kicker thread. This thread kicks the WHVP partition approx. 1000 times per
/// second to give us an opportunity to step a bit in Bochs and emulate devices
/// and potentially deliver interrupts.
/// 
/// This is _really_ gross but I don't see anything in the WHVP API that has
/// an alternative.
/// 
/// Further we use a busyloop here instead of a Sleep() so we can get a higher
/// frequency kick (to get 1000/second this seems necessary).
fn kicker(handle: usize) {
    // Cast the handle
    let handle = handle as WHV_PARTITION_HANDLE;

    // Determine this processors TSC rate
    let tickrate = time::calibrate_tsc();

    // Calculate the amount of cycles between WHVP cancellations
    let advance = (tickrate / 1000.) as u64;

    // Kick forever
    loop {
        // Busyloop until the time has elapsed
        let future = time::rdtsc() + advance;
        while time::rdtsc() < future {}

        // Kick the hypervisor to cause it to exit
        unsafe { WHvCancelRunVirtualProcessor(handle, 0, 0); }
    }
}

// Due to bochs using longjmps we must place a lot of state in this structure
// so we don't lose it when our code suddenly gets hijacked and reenters from
// the start
#[derive(Default)]
struct PersistState {
    /// WHVP API context
    hypervisor: Option<Whvp>,

    /// Cached tickrate for the TSC on this processor
    tickrate: Option<f64>,

    /// Future time (as a TSC value) to print the status messages
    future_report: u64,

    /// Cycle count when the hypervisor was first created. This is used to get
    /// uptime
    start: u64,

    /// Estimated number of cycles spent inside of the hypervisor executing
    vm_elapsed: u64,

    /// Last TSC value when Bochs device state was synced with the wall clock
    last_sync_cycles: u64,
}

thread_local! {
    /// Thread locals. I really hate this but it's the only way to survive the
    /// re-entry due to Bochs using `longjmp()`
    static PERSIST: RefCell<PersistState> = RefCell::new(Default::default());
}

/// Rust CPU loop for Bochs which uses both emulation and hypervisor for running
/// a guest
#[no_mangle]
pub extern "C" fn bochs_cpu_loop(routines: &BochsRoutines, pmem_size: u64) {
    /// Make sure the physical memory size reported by Bochs is sane
    assert!(pmem_size & 0xfff == 0,
        "Physical memory size was not 4 KiB aligned");

    /// This is the hardcoded target IPS value we expect bochs to run at
    const TARGET_IPS: f64 = 1000000.0;

    // Obtain the thread local
    PERSIST.with(|x| {
        // Borrow the thread local
        let mut persist = x.borrow_mut();

        // Create a context to be used for all register sync operations
        let mut context = WhvpContext::default();

        // Cache the TSC rate if it's not already been cached
        if persist.tickrate.is_none() {
            persist.tickrate = Some(time::calibrate_tsc());
        }

        // Run first-time initialization of the hypervisor and other context
        if persist.hypervisor.is_none() {
            print!("Creating hypervisor!\n");

            // Create a new hypervisor :)
            let mut new_hyp = Whvp::new();

            // Memory regions of (paddr, backing memory, size in bytes)
            let mut mem_regions: Vec<MemoryRegion> = Vec::new();
            'next_mem: for paddr in (0usize..pmem_size as usize).step_by(4096) {
                // Get the bochs memory backing for this physical address
                let backing_read =
                    (routines.get_memory_backing)(paddr as u64, BX_READ);
                let backing_write =
                    (routines.get_memory_backing)(paddr as u64, BX_WRITE);
                let backing_execute =
                    (routines.get_memory_backing)(paddr as u64, BX_EXECUTE);

                // Skip unmapped memory
                if backing_read == 0 && backing_write == 0 &&
                    backing_execute == 0 { continue; }

                // Accumulate permissions and backing information
                let mut backing = None;
                let mut perms   = 0;

                // Check if this region is readable
                if backing_read != 0 {
                    if let Some(backing) = backing {
                        // If it's backed at the same location then mark it's
                        // also readable
                        if backing == backing_read {
                            perms |= PERM_READ;
                        }
                    } else {
                        // This is the first region registered, update
                        // permissions and set the backing
                        backing = Some(backing_read);
                        perms |= PERM_READ;
                    }
                }

                // Check if this region is writable
                if backing_write != 0 {
                    if let Some(backing) = backing {
                        // If it's backed at the same location then mark it's
                        // also readable
                        if backing == backing_write {
                            perms |= PERM_WRITE;
                        }
                    } else {
                        // This is the first region registered, update
                        // permissions and set the backing
                        backing = Some(backing_write);
                        perms |= PERM_WRITE;
                    }
                }

                // Check if this region is executable
                if backing_execute != 0 {
                    if let Some(backing) = backing {
                        // If it's backed at the same location then mark it's
                        // also readable
                        if backing == backing_execute {
                            perms |= PERM_EXECUTE;
                        }
                    } else {
                        // This is the first region registered, update
                        // permissions and set the backing
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

                // Search for a memory region which is connected to this one.
                // If the region is linear in both host address, guest physical
                // address, and has the same permissions. We merge it into one
                // larger region
                for mr in mem_regions.iter_mut() {
                    if mr.perms == perms && mr.paddr + mr.size == paddr &&
                            mr.backing + mr.size == backing {
                        // Allow contiguous memory in both physical and backing
                        // memory to be combined
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
                // Print some nice info about what we're mapping into the
                // hypervisor's address space and the permissions
                print!("Memory region: start {:016x} end {:016x} \
                        backing {:016x} perm {:02x}\n",
                    mr.paddr, mr.paddr + mr.size - 1, mr.backing, mr.perms);

                // Should never happen
                assert!(mr.size > 0 && mr.backing > 0);

                // Slice the backing to get the Rust representation
                let sliced = unsafe {
                    std::slice::from_raw_parts_mut(
                        mr.backing as *mut u8, mr.size)
                };

                // Map the memory :)
                new_hyp.map_memory(mr.paddr, sliced, mr.perms);
            }

            // Get the raw handle for this partition and create the kicker
            // thread which is responsible for on an interval causing VMEXITS
            // which gives us a chance to deliver interrupts
            let handle = new_hyp.handle() as usize;
            std::thread::spawn(move || kicker(handle));

            // Save the hypervisor into the persitent storage
            persist.hypervisor = Some(new_hyp);

            // Compute first report time
            persist.future_report =
                time::rdtsc() + persist.tickrate.unwrap() as u64;

            // Save the time of hypervisor creation
            persist.start = time::rdtsc();

            // Initialize VM cycle count
            persist.vm_elapsed = 0;

            // Record the TSC value for the last time Bochs device state was
            // synced with the wall clock
            persist.last_sync_cycles = time::rdtsc();
        }

        // Value which counts how many blocks we want to emulate in bochs
        // When this is 0 we use the hypervisor, when this is non-zero we run
        // that many steps in Bochses emulator
        let mut emulating = 0;

        loop {
            {
                // Get the current TSC
                let current_time = time::rdtsc();

                // Calculate cycles elapsed since the last Bochs device sync
                // Saturating in case we reschedule cores and underflow here
                let elapsed_cycles = current_time
                    .saturating_sub(persist.last_sync_cycles);

                // Update the most recent sync time
                persist.last_sync_cycles = current_time;

                // Compute the number of seconds that have elapsed since the
                // last sync
                let elapsed_secs = elapsed_cycles as f64 /
                    persist.tickrate.unwrap();

                // Convert the number of seconds into the number of ticks Bochs
                // expects for this corresponding time.
                // For example if on a 4GHz processor we ran for 1 second, the
                // tick count would be ~4 billion. The `elapsed_secs` would then
                // be ~1.0, which then will be multiplied by the `TARGET_IPS`
                // constant that indicates the Bochs IPS frequency. This gives
                // us an integer value which is the number of cycles in Bochs
                // which corresponds with this same amount of wall-clock time
                let elapsed_adj_cycles = (TARGET_IPS * elapsed_secs) as u64;

                // Tick devices along in Bochs to emulate the time that has
                // passed
                (routines.step_device)(elapsed_adj_cycles);
            }

            // If the TSC is past the future report time, it's time to do our
            // prints :D
            if time::rdtsc() >= persist.future_report {
                // Compute the total number of cycles elapsed since start
                let total_cycles = time::rdtsc() - persist.start;

                // Print some stats
                print!("VM run percentage {:8.6} | Uptime {:14.6}\n",
                    persist.vm_elapsed as f64 / total_cycles as f64,
                    total_cycles as f64 / persist.tickrate.unwrap());

                // Update the next report time
                persist.future_report = time::rdtsc() +
                    persist.tickrate.unwrap() as u64;
            }

            // If we're requesting emulation, step using Bochs
            if emulating > 0 {
                // Emulate instructions with Bochs!
                (routines.step_cpu)(emulating);

                // Indicate no more desire to emulate, loop again
                emulating = 0;
                continue;
            }

            // Sync bochs register state to hypervisor register state
            (routines.get_context)(&mut context);
            persist.hypervisor.as_mut().unwrap().set_context(&context);

            // Get the current TSC
            let vmstart_cycles = time::rdtsc();

            // Run the hypervisor until exit!
            let vmexit = persist.hypervisor.as_mut().unwrap().run();

            // Compute number of cycles spent in the call to `vm.run()`
            let elapsed = time::rdtsc() - vmstart_cycles;

            // Subtract off the API overhead of the call to approximate the
            // number of cycles elapsed inside of the VM itself. Saturating to
            // handle some noise and potential integer underflow.
            let vm_run_time = elapsed.saturating_sub(
                persist.hypervisor.as_mut().unwrap().overhead());

            // Update statistics about number of cycles spent in the hypervisor
            persist.vm_elapsed += vm_run_time;

            // Sync hypervisor register state to Bochs register state
            context = persist.hypervisor.as_mut().unwrap().get_context();
            (routines.set_context)(&context);

            // Determine the reason the hypervisor exited
            match vmexit.ExitReason {
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonMemoryAccess => {
                    // Emulate MMIO by emulating using Bochs for a bit
                    emulating = 100;
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64IoPortAccess => {
                    // Emulate I/O by emulating using Bochs for a bit
                    emulating = 100;
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64Halt => {
                    // Emulate halts by emulating using Bochs for a bit
                    emulating = 100;
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonCanceled => {
                    // Check if rflags.IF=0
                    if (unsafe { context.rflags.Reg64 } & (1 << 9)) == 0 {
                        // If interrupts are disabled, request to be notified
                        // next time they are enabled so we can potentially
                        // deliver interrupts
                        persist.hypervisor.as_mut().unwrap()
                            .register_interrupt_window();
                    }
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonInvalidVpRegisterValue => {
                    // This was observed in Windows 7, however if we emulate a
                    // bit the issue seems to go away. So that's our "solution".
                    // Not sure which state is going bad here, or if it's some
                    // CPUID/MSR desync issue with Bochs
                    print!("Warning: Invalid VP state, emulating for a bit\n");
                    emulating = 100;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64InterruptWindow => {
                    // We got an interrupt window! Well we don't have to do
                    // anything as now Bochs knows it can deliver async events
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
