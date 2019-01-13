#![feature(asm)]
#![allow(non_upper_case_globals)]

pub mod whvp;
pub mod time;
pub mod virtmem;
pub mod win32;
pub mod symdumper;
pub mod symloader;
pub mod disk;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap};
use crate::whvp::{Whvp, WhvpContext};
use crate::whvp::{PERM_READ, PERM_WRITE, PERM_EXECUTE};
use whvp_bindings::winhvplatform::*;
use crate::win32::{get_modlist, find_kernel_modlist};
use crate::symloader::Symbols;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::win32::{ModuleList};
use std::fs::File;
use std::io::Write;
use std::time::SystemTime;
use std::ffi::CString;

/// Number of instructions to step in emulation mode after a vmexit
const EMULATE_STEPS: u64 = 250;

/// Disables coverage entirely if this is `true`
/// This helps a lot with performance if you're not concerned with coverage info
const COVERAGE_DISABLE: bool = true;

/// Logs coverage using symbols to the file `coverage.txt`.
const LOG_COVERAGE_SYMBOLS: bool = false;

/// Maximum amount of instructions to emulate at a given time
const MAX_EMULATE: u64 = 1000;

/// Discard reads/writes to the framebuffer when in the hypervisor. This breaks
/// screen updates but gives a performance boost if you only care about RDP/SSH
/// into the guest
const DEVNULL_FRAMEBUFFERS: bool = false;

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

    // Get the CPUID result from Bochs for a given leaf:subleaf combination
    cpuid: extern fn(leaf: u32, subleaf: u32, eax: &mut u32, ebx: &mut u32,
        ecx: &mut u32, edx: &mut u32),

    /// Write an MSR `value` to the MSR specified by `index`
    write_msr: extern fn(index: u32, value: u64),

    /// Notify devices that a restore just occured. This does things like
    /// redraw the screen, check CPU state is valid, and such.
    after_restore: extern fn(),

    /// Reset all devices and CPUs in Bochs
    reset_all: extern fn(),

    /// Take a Bochs snapshot and save it to `folder_name`
    take_snapshot: extern fn(folder_name: *const i8) -> !,
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

static KICKER_ACTIVE: AtomicUsize = AtomicUsize::new(0);

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
        while KICKER_ACTIVE.load(Ordering::SeqCst) == 0 {}

        // Busyloop until the time has elapsed
        let future = time::rdtsc() + advance;
        while KICKER_ACTIVE.load(Ordering::SeqCst) != 0 &&
            time::rdtsc() < future {}

        // Potentially the kicker was deactivated by this point, so skip the
        // cancel
        if KICKER_ACTIVE.load(Ordering::SeqCst) == 0 { continue; }

        // Kick the hypervisor to cause it to exit
        unsafe { WHvCancelRunVirtualProcessor(handle, 0, 0); }
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
enum VmExitReason {
    None,
    MemoryAccess,
    IoPortAccess,
    UnrecoverableException,
    InvalidVpRegisterValue,
    UnsupportedFeature,
    InterruptWindow,
    Halt,
    ApicEoi,
    MsrAccess,
    Cpuid,
    Exception,
    Canceled,
}

impl VmExitReason {
    fn from_whvp(whvp_reason: WHV_RUN_VP_EXIT_REASON) -> Self {
        match whvp_reason {
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonNone =>
                VmExitReason::None,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonMemoryAccess =>
                VmExitReason::MemoryAccess,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64IoPortAccess =>
                VmExitReason::IoPortAccess,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonUnrecoverableException =>
                VmExitReason::UnrecoverableException,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonInvalidVpRegisterValue =>
                VmExitReason::InvalidVpRegisterValue,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonUnsupportedFeature =>
                VmExitReason::UnsupportedFeature,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64InterruptWindow =>
                VmExitReason::InterruptWindow,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64Halt =>
                VmExitReason::Halt,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64ApicEoi =>
                VmExitReason::ApicEoi,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64MsrAccess =>
                VmExitReason::MsrAccess,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64Cpuid =>
                VmExitReason::Cpuid,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonException =>
                VmExitReason::Exception,
            WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonCanceled =>
                VmExitReason::Canceled,
            _ => panic!("Invalid vm exit reason {}\n", whvp_reason),
        }
    }
}

/// All kinds of statistics for tracking what we're doing
#[derive(Default, Debug)]
struct Statistics {
    /// Number coverage callback invocations
    coverage_callbacks: u64,

    /// Number of module list walks
    module_list_walks: u64,

    /// Number of fuzz cases
    num_fuzz_cases: u64,
}

/// A single module worth of coverage information
/// 
/// There is one of these structures for each module
#[derive(Clone)]
struct CoverageEntry {
    /// Bitmap of covered offsets in this module. One bit per byte of the
    /// image size
    bitmap: Vec<u8>,

    /// Number of unique offsets observed in this module
    unique: u64,
}

// Dummy aligned structure
#[repr(C, align(4096))]
#[derive(Clone, Copy)]
struct Page([u8; 4096]);

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

    /// VM exit reason frequencies
    vmexits: HashMap<VmExitReason, u64>,

    /// Memory reader for physical and virtual access
    memory: MemReader,

    /// Code coverage information per module
    coverage: Vec<Option<CoverageEntry>>,

    /// Symbols
    symbols: Symbols,

    /// Pointer to `nt!PsLoadedModuleList` global
    kernel_module_list: Option<usize>,

    /// Module list cache
    module_list_cache: ModuleList,

    /// Coverage log file (contains list of new coverage)
    coverage_log_file: Option<File>,

    /// Statistics
    stats: Statistics,

    /// Number of instructions to emulate next CPU loop iteration
    emulating: u64,

    /// Normal framebuffer backing memory 0xa0000-0xbffff
    normal_fb: Vec<Page>,

    /// Linear framebuffer backing memory 0xe0000000-0xe0ffffff
    linear_fb: Vec<Page>,
}

thread_local! {
    /// Thread locals. I really hate this but it's the only way to survive the
    /// re-entry due to Bochs using `longjmp()`
    static PERSIST: RefCell<PersistState> = RefCell::new(Default::default());
}

#[derive(Default)]
pub struct MemReader {
    /// List of all of the memory regions mapped into the guest physical space
    regions: Vec<MemoryRegion>
}

macro_rules! read_virt_declare {
    ($name:ident, $vt:ty) => (
        pub fn $name(&mut self, cr3: usize, addr: usize) -> Result<$vt, ()> {
            let mut val = [0u8; std::mem::size_of::<$vt>()];
            if self.read_virt(cr3, addr, &mut val) == val.len() {
                Ok(<($vt)>::from_ne_bytes(val))
            } else {
                Err(())
            }
        }
    )
}

impl MemReader {
    /// Create a new memory reader based on a list of memory regions
    fn new(memory_regions: Vec<MemoryRegion>) -> Self {
        MemReader { regions: memory_regions }
    }

    /// Read physical memory at `paddr` into an output buffer. Returns number
    /// of bytes read, this may be smaller than `output.len()` on partial reads
    pub fn read_phys(&mut self, paddr: usize, output: &mut [u8]) -> usize {
        // Sanity check
        assert!(output.len() > 0, "Output buffer was zero size");

        // Track number of bytes read
        let mut bread = 0usize;

        while bread < output.len() {
            let mut matched_something = false;
            
            for mr in &self.regions {
                // Check if this address falls in this region
                if paddr >= mr.paddr {
                    // Compute offset and remainder of region
                    let offset = paddr - mr.paddr;
                    let remain = mr.size.saturating_sub(offset);

                    // Nothing in this region for us
                    if remain <= 0 { continue; }

                    // Convert to Rust slice
                    let region = unsafe {
                        std::slice::from_raw_parts(
                            mr.backing as *const u8, mr.size)
                    };

                    // Compute bytes to copy
                    let remain = std::cmp::min(output.len() - bread, remain);

                    // Copy bytes
                    output[bread..bread+remain].copy_from_slice(
                        &region[offset..offset+remain]);

                    // Update read amount
                    bread += remain;
                    matched_something = true;

                    if bread >= output.len() {
                        assert!(bread == output.len(), "Whoa we overshot");
                        return bread;
                    }
                }
            }

            // Failed to find a region that contains this byte, bail out
            if !matched_something { break; }
        }

        assert!(bread < output.len(), "Success path shouldn't go here");

        bread
    }

    /// Write physical memory at `paddr` from an input buffer. Returns number
    /// of bytes written, this may be smaller than `input.len()` on partial
    /// writes
    pub fn write_phys(&mut self, paddr: usize, input: &[u8]) -> usize {
        // Sanity check
        assert!(input.len() > 0, "Input buffer was zero size");

        // Track number of bytes read
        let mut bread = 0usize;

        while bread < input.len() {
            let mut matched_something = false;
            
            for mr in &self.regions {
                // Check if this address falls in this region
                if paddr >= mr.paddr {
                    // Compute offset and remainder of region
                    let offset = paddr - mr.paddr;
                    let remain = mr.size.saturating_sub(offset);

                    // Nothing in this region for us
                    if remain <= 0 { continue; }

                    // Convert to Rust slice
                    let region = unsafe {
                        std::slice::from_raw_parts_mut(
                            mr.backing as *mut u8, mr.size)
                    };

                    // Compute bytes to copy
                    let remain = std::cmp::min(input.len() - bread, remain);

                    // Copy bytes
                    region[offset..offset+remain].copy_from_slice(
                        &input[bread..bread+remain]);

                    // Update read amount
                    bread += remain;
                    matched_something = true;

                    if bread >= input.len() {
                        assert!(bread == input.len(), "Whoa we overshot");
                        return bread;
                    }
                }
            }

            // Failed to find a region that contains this byte, bail out
            if !matched_something { break; }
        }

        assert!(bread < input.len(), "Success path shouldn't go here");

        bread
    }

    /// Read virtual memory at `vaddr` using page table `cr3` into `buf`.
    /// Returns number of bytes read (can be less than `buf.len()` on error)
    pub fn read_virt(&mut self, cr3: usize, vaddr: usize,
                     buf: &mut [u8]) -> usize
    {
        // Cached physical translation
        let mut guest_phys = 0;

        // Go through each byte
        for offset in 0..buf.len() {
            // Update translation on new pages
            if (guest_phys & 0xfff) == 0 {
                // Translate vaddr to paddr
                let mut guest_pt = unsafe {
                    virtmem::PageTable::from_existing(cr3 as *mut u64, self)
                };
                guest_phys = match guest_pt.virt_to_phys_dirty(
                        (vaddr + offset) as u64, false) {
                    Ok(Some((phys, _))) => phys,
                    _                   => return offset,
                };
            }

            // Read one byte from memory
            if self.read_phys(guest_phys as usize,
                    &mut buf[offset..offset+1]) != 1 {
                // Failed to read, return bytes read to this point
                return offset;
            }

            // Update physical pointer
            guest_phys += 1;
        }

        // Return bytes read
        buf.len()
    }

    /// Writes virtual memory at `vaddr` using page table `cr3` from `buf`.
    /// Returns number of bytes written (can be less than `buf.len()` on error)
    pub fn write_virt(&mut self, cr3: usize, vaddr: usize,
                      buf: &[u8]) -> usize
    {
        // Cached physical translation
        let mut guest_phys = 0;

        // Go through each byte
        for offset in 0..buf.len() {
            // Update translation on new pages
            if (guest_phys & 0xfff) == 0 {
                // Translate vaddr to paddr
                let mut guest_pt = unsafe {
                    virtmem::PageTable::from_existing(cr3 as *mut u64, self)
                };
                guest_phys = match guest_pt.virt_to_phys_dirty(
                        (vaddr + offset) as u64, false) {
                    Ok(Some((phys, _))) => phys,
                    _                   => return offset,
                };
            }

            // Read one byte from memory
            if self.write_phys(guest_phys as usize,
                    &buf[offset..offset+1]) != 1 {
                // Failed to read, return bytes read to this point
                return offset;
            }

            // Update physical pointer
            guest_phys += 1;
        }

        // Return bytes read
        buf.len()
    }

    read_virt_declare!(read_virt_u8, u8);
    read_virt_declare!(read_virt_u16, u16);
    read_virt_declare!(read_virt_u32, u32);
    read_virt_declare!(read_virt_u64, u64);
    read_virt_declare!(read_virt_usize, usize);
}

impl virtmem::PhysMem for MemReader {
    fn alloc_page(&mut self) -> Option<*mut u8> {
        panic!("Alloc page not supported");
    }

    fn read_phys_int(&mut self, addr: *mut u64) -> Result<u64, &'static str> {
        let mut buf = [0u8; 8];
        if self.read_phys(addr as usize, &mut buf) != buf.len() {
            return Err("Failed to read physical memory");
        }
        Ok(u64::from_ne_bytes(buf))
    }

    fn write_phys(&mut self, _addr: *mut u64,
            _val: u64) ->Result<(), &'static str> {
        panic!("write_phys not supported");
    }

    fn probe_vaddr(&mut self, _addr: usize, _length: usize) -> bool {
        panic!("probe_vaddr not supported");
    }
}

/// Dump the coverage table to the console
fn dump_coverage(coverage: &Vec<Option<CoverageEntry>>) {
    // Nothing to report
    if coverage.len() <= 0 { return; }

    // Track total number of unique coverage entries
    let mut sum = 0;

    // Track the largest filename so we can print cleanly
    let mut largest_filename = 0;

    // Create a copy of the coverage listing
    let mut coverage_sorted = Vec::new();
    for (ordinal, entry) in coverage.iter().enumerate() {
        if let Some(entry) = entry {
            // Get the module info for this ordinal
            let modinfo = win32::ordinal_to_modinfo(ordinal as win32::Ordinal)
                .expect("Got coverage on ordinal that doesn't exist!?");

            // Update the largest filename stat
            largest_filename =
                std::cmp::max(largest_filename, modinfo.name().len());

            coverage_sorted.push((ordinal, entry.unique));
        }
    }

    // Sort by frequency
    coverage_sorted.sort_by_key(|x| x.1);

    print!("Coverage:\n");

    // Print out all the per-module coverage info
    for (ordinal, unique) in coverage_sorted.iter() {
        let modinfo = win32::ordinal_to_modinfo(*ordinal as win32::Ordinal)
            .expect("Got coverage on ordinal that doesn't exist!?");
        print!("{:width$} | {:7} unique offsets\n",
            modinfo.name(), unique, width = largest_filename);
        sum += unique;
    }

    print!("Modules containing coverage: {:10}\n", coverage_sorted.len());
    print!("Coverage total:              {:10}\n", sum);
}

/// Types for all shadow data types used in snapshots
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum ShadowType {
  Data,    // bx_shadow_data_c, raw data pointer and a size
  Fileptr, // bx_shadow_filedata_c, FILE**
  Bit64s,  // bx_shadow_num_c int64_t
  Bit64u,  // bx_shadow_num_c uint64_t
  Bit32s,  // bx_shadow_num_c int32_t
  Bit32u,  // bx_shadow_num_c uint32_t
  Bit16s,  // bx_shadow_num_c int16_t
  Bit16u,  // bx_shadow_num_c uint16_t
  Bit8s,   // bx_shadow_num_c int8_t
  Bit8u,   // bx_shadow_num_c uint8_t
  Float,   // bx_shadow_num_c float
  Double,  // bx_shadow_num_c double
  Bool,    // bx_shadow_bool_c bool
}

/// This represents the state of a given device
struct DeviceState {
    /// Address to the memory in Bochs which holds this state
    addr: usize,

    /// Size of the state
    len: usize,

    /// If set, this is the saved state from the initial execution of the CPU
    /// loop.
    /// 
    /// This is what we restore to during a device restore
    original: Option<Vec<u8>>,
}

thread_local! {
    /// Vector of all the device states in Bochs
    static DEVICE_STATE: RefCell<Vec<DeviceState>> =
        RefCell::new(Vec::new());

    /// Determines whether device state lists can be updated
    /// 
    /// This is locked when the first call to the Bochs CPU loop is invoked,
    /// which prevents us from registering new state when running.
    static DEVICE_STATE_LOCKED: Cell<bool> = Cell::new(false);
}

/// Convert a null-terminated C string to a Rust string
unsafe fn string_from_cstring(cstring: *const u8) -> Option<String> {
    // Kill null pointers
    if cstring.is_null() { return None; }

    // strlen()
    let mut length = 0;
    loop {
        if *cstring.offset(length as isize) == 0 { break; }
        length += 1;
    }

    // Convert it to a Rust string
    Some(std::str::from_utf8(std::slice::from_raw_parts(cstring, length))
        .expect("Invalid C string").into())
}

/// Callback for handling Bochs device state registration. Data is a pointer,
/// size is a size of the data in bytes, and typ is the type of data.
/// 
/// Technically the type doesn't matter as we only care about the raw data
/// but it's nice to have
#[no_mangle]
pub extern "C" fn register_state(name: *const u8, label: *const u8,
        data: usize, size: usize, _typ: ShadowType) {           
    // Ensure device state is not locked for editing
    DEVICE_STATE_LOCKED.with(|locked| {
        assert!(locked.get() == false, "Device state change during lock")
    });

    assert!(size > 0, "Tried to register device state with 0 size");

    // Convert the name and label to Rust strings
    let name   = unsafe { string_from_cstring(name)  };
    let _label = unsafe { string_from_cstring(label) };

    // We don't track memory here
    if name == Some("ram.memory.bochs.bochs".into()) {
        return;
    }

    // We don't track VRAM here
    if name == Some("memory.vgacore.vga.bochs.bochs".into()) {
        return;
    }
    
    // Add this device to the device state list
    DEVICE_STATE.with(|x| {
        let mut x = x.borrow_mut();

        // Save it to the list of device state!
        x.push(DeviceState {
            addr: data, len: size, original: None
        });
    });
}

#[no_mangle]
/// Callback for handling coverage events
pub extern "C" fn report_coverage(cr3: usize, lma: bool, gs_base: usize,
        cs: u16, rip: usize, _rsp: usize) -> bool {
    if COVERAGE_DISABLE { return false; }

    // Fast path, we only collect 64-bit coverage right now
    if !lma { return false; }

    // Obtain the thread local
    PERSIST.with(|x| {
        // Borrow the thread local
        let persist = &mut *x.borrow_mut();

        persist.stats.coverage_callbacks += 1;

        let mut new_coverage = false;

        // Quick and dirty coverage
        let kml = persist.kernel_module_list;

        // Split structure references to help with borrowck
        let mlc      = &mut persist.module_list_cache;
        let coverage = &mut persist.coverage;
        let memory   = &mut persist.memory;
        let stats    = &mut persist.stats;

        // Get the module offset for this RIP
        let mut cached = mlc.get_modoff(rip);

        if cached.0.is_none() {
            // Module didn't resolve from the cache, rewalk the module list
            // to check for updates
            stats.module_list_walks += 1;
            if let Ok(ml) = get_modlist(memory, cr3, lma, gs_base, cs, kml) {
                //print!("Updating module list cache\n");
                *mlc = ml;
                cached = mlc.get_modoff(rip);
            } else {
                // Couldn't resolve module and couldn't update module list
                // we can't do anything at this point
                return false;
            }
        }

        // If we were able to resolve the module, report the coverage
        if let (Some(module), offset) = cached {
            let size    = module.size() as usize;
            let ordinal = module.ordinal() as usize;

            // Make sure there are enough entries in the coverage list for our
            // ordinal to be valid
            while ordinal >= coverage.len() {
                coverage.push(None);
            }

            // The above loop makes sure the ordinal is a valid index into
            // coverage but it is still potentially a None value.
            if coverage[ordinal].is_none() {
                // Get the image size rounded up to the nearest byte boundary
                let imagesize = (size + 7) & !7;

                let covent = CoverageEntry {
                    bitmap: vec![0u8; imagesize],
                    unique: 0,
                };

                coverage[ordinal] = Some(covent);
            }

            // Get a mutable reference to this corresponding coverage entry
            let covent = coverage[ordinal].as_mut().unwrap();

            // Calculate bitmap offsets
            let byte = offset / 8;
            let bit  = offset % 8;

            // Check if we've seen this offset before
            if (covent.bitmap[byte] & (1 << bit)) == 0 {
                // Record that we got new coverage so we can return that
                // information to the caller
                new_coverage = true;

                // Update bitmap
                covent.bitmap[byte] |= 1 << bit;

                // Update unique count
                covent.unique += 1;

                // Only use symbol coverage if requested
                if LOG_COVERAGE_SYMBOLS {
                    // Try to look up the symbol for this new coverage
                    if let Some(sym) = persist.symbols.resolve(module, offset) {
                        // Create the log file if it's not already open
                        if persist.coverage_log_file.is_none() {
                            persist.coverage_log_file = Some(
                                File::create("coverage.txt")
                                    .expect("Failed to open coverage output")
                            );
                        }

                        // Write the symbol to the log file
                        let clf = persist.coverage_log_file.as_mut().unwrap();
                        clf.write(format!("{}\n", sym).as_bytes())
                            .expect("Failed to write coverage entry");
                        clf.flush().expect("Failed to flush coverage file");
                    }
                }
            }
        } else {
            // Unknown module
        }

        // If we got new coverage, emulate for longer
        if new_coverage {
            persist.emulating += 100;
        }

        new_coverage
    })
}

/// Reset all dirty pages in the guest based on the dirty bit L1 and L2 tables
/// 
/// This will reset the regions in `memory` which are dirty based on
/// `dirty_bits_l1` and `dirty_bits_l2` with the original memory contents of
/// the snapshot `orig_memory`.
/// 
/// This also clears the dirty bits as the memory is reset
pub fn reset_dirty_pages(orig_memory: &[u8], memory: &mut [u8],
        dirty_bits_l1: &mut [u64], dirty_bits_l2: &mut [u64]) {
    assert!(orig_memory.len() == memory.len(), "Whoa this should never happen");

    // Go through each qword in the L1 dirty bits
    for (l1idx, l1ent) in dirty_bits_l1.iter_mut().enumerate() {
        // Fast path, no dirty bits, skip this entry
        if *l1ent == 0 { continue; }

        // There is a dirty bit somewhwere in here, find it
        for l1bit in 0..64 {
            if *l1ent & (1 << l1bit) != 0 {
                // Compute the address of this 1 MiB dirty region
                let dirty_addr = l1idx * 1024 * 1024 * 64 + l1bit * 1024 * 1024;

                // Compute the L2 4 KiB region indicies corresponding to this
                // range
                let qword_l2     = dirty_addr / (4096 * 64);
                let qword_l2_end = (dirty_addr + 1024*1024) / (4096 * 64);

                //print!("Dirty L1 entry {:x}\n", dirty_addr);

                // Go through all the 4 KiB entries for this 1 MiB range
                for (l2idx, l2ent) in dirty_bits_l2[qword_l2..qword_l2_end]
                        .iter_mut().enumerate() {
                    // Make sure this is an absolute index rather than relative
                    // as we slice the range
                    let l2idx = l2idx + qword_l2;

                    // Skip ranges with no dirty bits
                    if *l2ent == 0 { continue; }

                    // Find the set bits in this entry
                    for l2bit in 0..64 {
                        if *l2ent & (1 << l2bit) != 0 {
                            // Compute 4 KiB dirty address
                            let dirty_addr = l2idx * 4096 * 64 + l2bit * 4096;
                            //print!("\tDirty L2 entry {:x}\n", dirty_addr);

                            if dirty_addr < orig_memory.len() {
                                // Actually restore the memory
                                memory[dirty_addr..dirty_addr + 4096]
                                    .copy_from_slice(&orig_memory
                                        [dirty_addr..dirty_addr + 4096]);
                            }
                        }
                    }

                    // Clear dirty bits
                    *l2ent = 0;
                }
            }
        }

        // Clear dirty bits
        *l1ent = 0;
    }
}

/// Fully restores all CPU, device, and memory states
/// 
/// This _may_ leave some devices in an undefined state if things like their
/// ISR or MMIO BARs are changed. `after_restore` needs to be called to correct
/// these changes, but we first need to reset all CPU state and memory/IRQ
/// handlers. This operation is so expensive it's just not feasible.
/// Hopefully I can find a better way in the future but for now as long as
/// a PCI device isn't massively reprogrammed during a fuzz case this should be
/// fine. Further reprogramming of MMIO bases and ISRs usually only happens
/// during boot of an OS.
/// 
/// We also do not restore the VGA buffer at all. It's 16 MiB and causes a huge
/// slowdown. We also don't really care about the screen state when fuzzing
fn restore(orig_memory: &[u8], memory: &mut [u8],
        dirty_bits_l1: &mut [u64], dirty_bits_l2: &mut [u64]) {
    PERSIST.with(|persist| {
    DEVICE_STATE.with(|devices| {
        // Borrow the thread local
        let mut persist = persist.borrow_mut();

        // Restore all device states
        for devstate in devices.borrow_mut().iter() {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    devstate.original.as_ref().unwrap().as_ptr(),
                    devstate.addr as *mut u8, devstate.len);
            }
        }

        // Update the dirty bitmap from the hypervisor dirty list
        // Without this we can do about 65,000 restores/second
        // With this we can only do about 50. This is a huge performance
        // bottleneck right now and I'm already in an email chain with the
        // WHVP dev to optimize their dirty list API. I requested to get 10k
        // per second for querying an empty dirty list. Hopefully we get that :D
        // I've got some workaround ideas for this
        persist.hypervisor.as_mut().unwrap().get_dirty_list(
            dirty_bits_l1, dirty_bits_l2);

        // Restore memory
        reset_dirty_pages(orig_memory, memory, dirty_bits_l1, dirty_bits_l2);

        // Restore disk
        disk::vdisk_discard_changes();

        // Update devices. This currently doesn't work as the IRQs get
        // remapped twice, causing a Bochs panic
        //(routines.after_restore)();
    });
    });
}

/// Simple benchmark used to see how fast we can reset the VM in a loop
fn _benchmark_restore(orig_memory: &[u8], memory: &mut [u8],
        dirty_bits_l1: &mut [u64], dirty_bits_l2: &mut [u64]) {
    let start = std::time::Instant::now();
    for iters in 0u64.. {
        restore(orig_memory, memory, dirty_bits_l1, dirty_bits_l2);

        if (iters & 0xff) == 0 {
            let delta = time::elapsed_from(&start);
            if delta >= 5.0 {
                print!("Iters {:10} in {:10.6} seconds | {:10.2} iters/second\n",
                    iters,
                    delta,
                    (iters as f64) / delta);
                break;
            }
        }
    }
}

/// Rust CPU loop for Bochs which uses both emulation and hypervisor for running
/// a guest
#[no_mangle]
pub extern "C" fn bochs_cpu_loop(routines: &BochsRoutines, pmem_size: u64,
        dirty_bits_l1: usize, dirty_bits_l2: usize, bochs_memory_base: usize,
        original_memory_base: usize) {
    /// Make sure the physical memory size reported by Bochs is sane
    assert!(pmem_size & 0xfff == 0,
        "Physical memory size was not 4 KiB aligned");

    // Lock further device state registration
    let first_run = !DEVICE_STATE_LOCKED.with(|locked| locked.replace(true));

    // Get slice into raw physical memory of Bochs
    let memory = unsafe {
        std::slice::from_raw_parts_mut(bochs_memory_base as *mut u8,
            pmem_size as usize)
    };

    // Get slice of original memory state if we're restoring from a snapshot.
    // If we're not in snapshot mode this will be `None`.
    let orig_memory = if original_memory_base > 0 { unsafe {
        Some(std::slice::from_raw_parts(original_memory_base as *const u8,
            pmem_size as usize))
    }} else {
        None
    };

    // Get slice to L1 dirty bits
    let dirty_bits_l1 = unsafe {
        std::slice::from_raw_parts_mut(dirty_bits_l1 as *mut u64,
            (4 * 1024 * 1024 * 1024) / (1024 * 1024 * 64))
    };

    // Get slice to L2 dirty bits
    let dirty_bits_l2 = unsafe {
        std::slice::from_raw_parts_mut(dirty_bits_l2 as *mut u64,
            (4 * 1024 * 1024 * 1024) / (4096 * 64))
    };

    // If this is the first run and we're in snapshot mode, save all device
    // state
    if first_run && orig_memory.is_some() {
        DEVICE_STATE.with(|x| {
            let mut x = x.borrow_mut();

            // Print the stats of the device state prior to merging contiguous
            // regions of device state
            print!("Device state: {:6} device states totalling {:10} bytes\n",
                x.len(), x.iter().fold(0usize, |acc, x| acc + x.len));

            // Sort by pointer
            x.sort_by_key(|x| x.addr);
            
            // This should never happen
            assert!(x.len() > 0, "Whoa, no devices registered!?");

            // Merge contiguous memory regions together
            let mut ii = 0;
            while ii < (x.len() - 1) {
                // If this region directly connects to the next then merge them
                // to make one larger region
                if (x[ii].addr + x[ii].len) == x[ii + 1].addr {
                    // Grow the current region
                    x[ii].len += x[ii + 1].len;

                    // Remove the region above this
                    x.remove(ii + 1);
                    continue;
                }

                ii += 1;
            }

            // Validate that there is no overlap in any of the regions
            for (fi, first) in x.iter().enumerate() {
                for (si, second) in x.iter().enumerate() {
                    // Skip direct comparisons
                    if fi == si { continue; }

                    // Compute overlap, -1 is safe as we never have a 0 sized
                    // entry
                    let overlaps = std::cmp::max(first.addr, second.addr) <=
                        std::cmp::min(first.addr + first.len - 1,
                            second.addr + second.len - 1);

                    assert!(!overlaps, "Overlap detected in device states");
                }
            }

            // Create copy of all the regions and save it off. This is the state
            // we reset to!
            for devstate in x.iter_mut() {
                // Create Rust slice representing the device state
                let sliced = unsafe {
                    std::slice::from_raw_parts(
                        devstate.addr as *const u8, devstate.len)
                };

                // Create a copy of this device state
                devstate.original = Some(Vec::from(sliced));
            }

            // Print the new statistics of the device state after we merged
            // contiguous device states
            print!("Reduced to:   {:6} device states totalling {:10} bytes\n",
                x.len(), x.iter().fold(0usize, |acc, x| acc + x.len));
        });
    }

    if orig_memory.is_none() {
        // If we're not in snapshot mode then the disk is non-volatile and writes
        // modify the disk
        disk::vdisk_set_non_volatile();
    }

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

            if DEVNULL_FRAMEBUFFERS {
                // Map in the framebuffers to the guest such that reads/writes
                // just are treated as normal RAM. This cuts down on vmexits
                // for framebuffer updates.
                // This is only usable if you use RDP or SSH or something to
                // access your guest, otherwise you get a corrupt/partially
                // updated screen

                // Normal framebuffer is at 0xa0000 for 128 KiB
                // linear framebuffer is at 0xe0000000 for 16 MiB

                // Allocate pages for framebuffers
                persist.normal_fb = vec![Page([0u8; 4096]);     128*1024 / 4096];
                persist.linear_fb = vec![Page([0u8; 4096]); 16*1024*1024 / 4096];

                // Map in normal framebuffer
                assert!(std::mem::size_of_val(
                    persist.normal_fb.as_slice()) == 128 * 1024);
                let sliced = unsafe {
                    std::slice::from_raw_parts_mut(
                        persist.normal_fb.as_mut_ptr() as *mut u8,
                        std::mem::size_of_val(persist.normal_fb.as_slice()))
                };
                new_hyp.map_memory(0xa0000, sliced, PERM_READ | PERM_WRITE);

                // Map in linear framebuffer
                assert!(std::mem::size_of_val(
                    persist.linear_fb.as_slice()) == 16 * 1024 * 1024);
                let sliced = unsafe {
                    std::slice::from_raw_parts_mut(
                        persist.linear_fb.as_mut_ptr() as *mut u8,
                        std::mem::size_of_val(persist.linear_fb.as_slice()))
                };
                new_hyp.map_memory(0xe0000000, sliced, PERM_READ | PERM_WRITE);
            }

            // Get the raw handle for this partition and create the kicker
            // thread which is responsible for on an interval causing VMEXITS
            // which gives us a chance to deliver interrupts
            let handle = new_hyp.handle() as usize;
            std::thread::spawn(move || kicker(handle));

            // Save the hypervisor into the persitent storage
            persist.hypervisor = Some(new_hyp);

            // Create a memory accessor
            persist.memory = MemReader::new(mem_regions);

            // Compute first report time
            persist.future_report =
                time::rdtsc() + (persist.tickrate.unwrap() as u64) * 5;

            // Save the time of hypervisor creation
            persist.start = time::rdtsc();

            // Initialize VM cycle count
            persist.vm_elapsed = 0;

            // Record the TSC value for the last time Bochs device state was
            // synced with the wall clock
            persist.last_sync_cycles = time::rdtsc();
        }

        // We expect on reentry that we try the hypervisor first
        persist.emulating = 0;

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

                dump_coverage(&persist.coverage);

                // Print vmexit reason frequencies
                print!("{:#?}\n", persist.vmexits);

                // Print statistics
                print!("{:#?}\n", persist.stats);

                // Attempt to find the nt!PsLoadedModuleList
                if persist.kernel_module_list.is_none() {
                    // Get information about the guest state
                    let cr3 = context.cr3() as usize;
                    let lma =
                        (unsafe { context.efer.Reg64 } & (1 << 10)) != 0;
                    let kernel_gs = unsafe { context.gs.Segment.Base as usize };
                    let cs        = unsafe { context.cs.Segment.Selector };

                    persist.kernel_module_list =
                        find_kernel_modlist(cr3, lma, kernel_gs, cs,
                        &mut persist.memory).ok();
                }

                // Update the next report time
                persist.future_report = time::rdtsc() +
                    (persist.tickrate.unwrap() as u64) * 5;
            }

            // If we're requesting emulation, step using Bochs
            persist.emulating = std::cmp::min(MAX_EMULATE, persist.emulating);
            if persist.emulating > 0 {
                let emu = persist.emulating;

                // Emulate instructions with Bochs!
                std::mem::drop(persist);
                (routines.step_cpu)(emu);
                persist = x.borrow_mut();

                // Subtract the amount we just emulated from the emulating
                // number.
                // We don't zero it because coverage could cause this to update
                persist.emulating = persist.emulating.checked_sub(emu)
                    .expect("Underflow on emulating");
                continue;
            }

            // Sync bochs register state to hypervisor register state
            (routines.get_context)(&mut context);
            persist.hypervisor.as_mut().unwrap().set_context(&context);

            // Get the current TSC
            let vmstart_cycles = time::rdtsc();

            // Run the hypervisor until exit!
            KICKER_ACTIVE.store(1, Ordering::SeqCst);
            let vmexit = persist.hypervisor.as_mut().unwrap().run();
            KICKER_ACTIVE.store(0, Ordering::SeqCst);

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

            if !COVERAGE_DISABLE {
                std::mem::drop(persist);

                let cr3 = context.cr3() as usize;
                let lma = (unsafe { context.efer.Reg64 } & (1 << 10)) != 0;
                let gs_base = unsafe { context.gs.Segment.Base } as usize;
                let cs = unsafe { context.cs.Segment.Selector };
                let rip = context.rip() as usize;
                let rsp = unsafe { context.rsp.Reg64 as usize };

                report_coverage(cr3, lma, gs_base, cs, rip, rsp);
                persist = x.borrow_mut();
            }

            // Record the exit reason frequencies
            let vmer = VmExitReason::from_whvp(vmexit.ExitReason);

            // Insert the reason if it's not already tracked
            if !persist.vmexits.contains_key(&vmer) {
                persist.vmexits.insert(vmer, 0);
            }

            // Update frequency
            *persist.vmexits.get_mut(&vmer).unwrap() += 1;

            if false {
                std::mem::drop(persist);
                // Benchmark the restore performance, used for testing while we
                // improve WHVP dirty performance.
                // This should be false for all git commits for now
                _benchmark_restore(orig_memory.as_ref().unwrap(),
                    memory, dirty_bits_l1, dirty_bits_l2);
                std::process::exit(-5);
            }

            // Determine the reason the hypervisor exited
            match vmexit.ExitReason {
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonMemoryAccess => {
                    /*let ma = unsafe { &vmexit.__bindgen_anon_1.MemoryAccess };
                    print!("Mem access GVA {:x} GPA {:x} RIP {:x}\n",
                        ma.Gva, ma.Gpa, context.rip());*/

                    // Emulate MMIO by emulating using Bochs for a bit
                    // Note this is tunable but 100 seems to by far be the best
                    // mix between performance and latency. <10 is unusable.
                    // >1000 introduces latency
                    // (cursor stutters when moving, etc)
                    persist.emulating += EMULATE_STEPS;
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonException => {
                    // Only take snapshots when running live
                    if orig_memory.is_some() {
                        persist.hypervisor.as_mut().unwrap().clear_pending_exception();
                        context.dr6.Reg64 = 1 << 16;
                        unsafe { context.rflags.Reg64 |= 1 << 16; }
                        (routines.set_context)(&context);

                        //persist.hypervisor.as_mut().unwrap().deliver_exception(1, None);
                        continue;
                    }

                    const MAGIC_BREAKPOINT_VALUE: u64 = 0x7b3c3638;

                    let exception =
                        unsafe { &vmexit.__bindgen_anon_1.VpException };

                    if exception.ExceptionType == 1 {
                        // a #DB debug exception occured
                        let dr0 = unsafe { context.dr0.Reg64 };
                        let dr1 = unsafe { context.dr1.Reg64 };
                        let dr2 = unsafe { context.dr2.Reg64 };
                        let dr3 = unsafe { context.dr3.Reg64 };
                        let dr6 = unsafe { context.dr6.Reg64 };
                        let dr7 = unsafe { context.dr7.Reg64 };

                        // Combine local and global breakpoints into a 4-bit
                        // vector of enabled breakpoint state
                        let enabled_breakpoints =
                            if (dr7 & (3 << 0)) != 0 { 1 << 0 } else { 0 } |
                            if (dr7 & (3 << 2)) != 0 { 1 << 1 } else { 0 } |
                            if (dr7 & (3 << 4)) != 0 { 1 << 2 } else { 0 } |
                            if (dr7 & (3 << 6)) != 0 { 1 << 3 } else { 0 };

                        // Figure out which hardware breakpoints triggered.
                        // This could be none as this #DB could have been from
                        // a single step event
                        let caused_breakpoint =
                            (dr6 & 0xf) & enabled_breakpoints;

                        // Clear that the debug exception occured
                        // You're supposed to set DR6.RTM when clearing DR6
                        context.dr6.Reg64 = 1 << 16;

                        if caused_breakpoint != 0 && (
                                dr0 == MAGIC_BREAKPOINT_VALUE ||
                                dr1 == MAGIC_BREAKPOINT_VALUE ||
                                dr2 == MAGIC_BREAKPOINT_VALUE ||
                                dr3 == MAGIC_BREAKPOINT_VALUE) {
                            let uptime_since_epoch =
                                SystemTime::now().duration_since(
                                    SystemTime::UNIX_EPOCH).unwrap();

                            // Construct a filename for the folder
                            let snapshot_folder_name =
                                format!("snapshot_{:?}", uptime_since_epoch);

                            print!("Taking snapshot to: {}\n",
                                snapshot_folder_name);

                            // Make snapshot folder
                            std::fs::create_dir(&snapshot_folder_name)
                                .expect("Snapshot already exists");

                            // Cause a Bochs snapshot!
                            let folder_name_cstr =
                                CString::new(snapshot_folder_name)
                                .expect("Couldn't convert to cstring");
                            (routines.take_snapshot)(folder_name_cstr.as_ptr());
                        } else {
                            // We should handle re-injecting the #DB exception
                            continue;
                        }

                        /*
                        print!("Debug exception happened\n");

                        // Inject the exception that was supposed to happen
                        unsafe {
                            if exception.ExceptionInfo
                                    .__bindgen_anon_1.ErrorCodeValid() != 0 {
                                persist.hypervisor.as_mut().unwrap()
                                    .deliver_exception(exception.ExceptionType,
                                        Some(exception.ErrorCode));
                            } else {
                                persist.hypervisor.as_mut().unwrap()
                                    .deliver_exception(
                                        exception.ExceptionType, None);
                            }

                            print!("{}\n", context);
                        }

                        persist.hypervisor.as_mut().unwrap().test_exception();

                        assert!(persist.emulating == 0,
                            "Shouldn't be emulating during exception");
                        continue;*/
                    } else {
                        panic!("Unhandled exception vmexit");
                    }
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64IoPortAccess => {
                    // Emulate I/O by emulating using Bochs for a bit
                    // Note this is tunable but 100 seems to by far be the best
                    // mix between performance and latency. <10 is unusable.
                    // >1000 introduces latency
                    // (cursor stutters when moving, etc)
                    persist.emulating += EMULATE_STEPS;
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64Halt => {
                    // Emulate halts by emulating using Bochs for a bit
                    persist.emulating += 1;
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
                    //print!("Warning: Invalid VP state, emulating for a bit\n");
                    persist.emulating += EMULATE_STEPS;
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64Cpuid => {
                    // Manually handle CPUIDs
                    let cpuid = unsafe { &vmexit.__bindgen_anon_1.CpuidAccess };

                    // Get the leaf and subleaf the MSR is trying to access
                    let leaf    = cpuid.Rax;
                    let subleaf = cpuid.Rcx;

                    // Get the ones that Hyper-V would have returned inside the
                    // VM
                    let rax = cpuid.DefaultResultRax;
                    let rbx = cpuid.DefaultResultRbx;
                    let mut rcx = cpuid.DefaultResultRcx;
                    let rdx = cpuid.DefaultResultRdx;

                    // Modify cpuid info
                    match (leaf, subleaf) {
                        (1, _) => {
                            // Disable xsave/xrstor support
                            // We don't know if we can safely sync these between
                            // bochs yet so we disable AVX.
                            rcx &= !(1 << 26);
                        }
                        _ => {}
                    }

                    // Update context
                    context.rax.Reg64  = rax;
                    context.rbx.Reg64  = rbx;
                    context.rcx.Reg64  = rcx;
                    context.rdx.Reg64  = rdx;

                    // Advance RIP past the cpuid instruction
                    unsafe {
                        context.rip.Reg64 +=
                            vmexit.VpContext.InstructionLength() as u64;
                    }
                    
                    // Write out the context and reenter the VM
                    (routines.set_context)(&context);
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64InterruptWindow => {
                    // We got an interrupt window! Well we don't have to do
                    // anything as now Bochs knows it can deliver async events
                    // and will when we `step_device()`
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonUnrecoverableException => {
                    persist.emulating += EMULATE_STEPS;
                    continue;
                }
                WHV_RUN_VP_EXIT_REASON_WHvRunVpExitReasonX64MsrAccess => {
                    // Handle MSR read/writes
                    persist.emulating += EMULATE_STEPS;
                    continue;
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
