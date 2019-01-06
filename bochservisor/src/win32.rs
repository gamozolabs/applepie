use crate::MemReader;
use crate::whvp::WhvpContext;
use std::fmt::Write;
use std::borrow::Cow;

/// All information to uniquely identify a module
#[derive(PartialOrd, Ord, PartialEq, Eq, Hash, Clone, Debug)]
pub struct ModuleInfo<'a> {
    name:          Cow<'a, str>,
    timedatestamp: u32,
    sizeofimage:   u32,
}

impl<'a> ModuleInfo<'a> {
    /// Create a new `ModuleInfo`
    pub fn new(module: Cow<'a, str>, timedatestamp: u32, sizeofimage: u32) -> Self {
        ModuleInfo {
            name: module.into(),
            timedatestamp,
            sizeofimage
        }
    }

    /// Clones a `ModuleInfo` to change the lifetime
    pub fn deepclone<'b>(&self) -> ModuleInfo<'b> {
        ModuleInfo {
            name:          self.name.to_string().into(),
            timedatestamp: self.timedatestamp,
            sizeofimage:   self.sizeofimage
        }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn time(&self) -> u32  { self.timedatestamp }
    pub fn size(&self) -> u32  { self.sizeofimage }
}

/// Module entry
#[derive(Debug)]
pub struct ModuleEntry {
    /// Info to uniquely identify this module
    info: ModuleInfo<'static>,

    /// Base address of the module
    base: usize,

    /// Length (in bytes) of the module
    len: usize,
}

/// Group of modules
#[derive(Debug, Default)]
pub struct ModuleList {
    /// List of all modules
    modules: Vec<ModuleEntry>,
}

impl ModuleList {
    /// Create a new module list
    fn new() -> Self {
        ModuleList { modules: Vec::new() }
    }

    /// Register a new module
    fn add_module(&mut self, module: ModuleEntry) {
        self.modules.push(module);
    }

    /// Get the module offset representation of a virtual address
    pub fn get_modoff(&self, vaddr: usize) -> (Option<&ModuleInfo>, usize) {
        for module in &self.modules {
            if vaddr >= module.base &&
                    vaddr < module.base.checked_add(module.len).unwrap() {
                let offset = vaddr - module.base;
                return (Some(&module.info), offset);
            }
        }

        (None, vaddr)
    }

    /// Get the module offset representation of a virtual address
    pub fn get_modoff_string_int(&self, vaddr: usize, output: &mut String) {
        output.clear();

        let (modinfo, offset) = self.get_modoff(vaddr);
        if let Some(modinfo) = modinfo {
            write!(output, "{}+", modinfo.name()).unwrap();
        }
        write!(output, "0x{:x}", offset).unwrap();
    }

    /// Get the module offset representation of a virtual address
    pub fn get_modoff_string(&self, vaddr: usize) -> String {
        let mut ret = String::new();
        self.get_modoff_string_int(vaddr, &mut ret);
        ret
    }
}

/// Get a list of all modules for the current running process
/// Currently only for user-mode applications
/// On failure may return a 0 sized module list
fn get_modlist_user<'a>(modlist: &mut ModuleList,
        cr3: usize, lma: bool, gs_base: usize, cs: u16,
        memory: &mut MemReader) -> Result<(), ()> {
    // Make sure we have a GS, we're in userspace, and we're also 64-bit
    if !(gs_base != 0 && lma && (cs & 3) == 3) {
        return Err(());
    }

    // Look up the PEB from the TEB
    let peb_ptr = memory.read_virt_usize(cr3, gs_base + 0x60)?;

    // Get the _PEB_LDR_DATA structure pointer 
    let peb_ldr_ptr = memory.read_virt_usize(cr3, peb_ptr + 0x18)?;

    // Get the first pointer to the InLoadOrderModuleList
    // This type is of _LDR_DATA_TABLE_ENTRY
    let mut flink = memory.read_virt_usize(cr3, peb_ldr_ptr + 0x10)?;
    let blink     = memory.read_virt_usize(cr3, peb_ldr_ptr + 0x18)?;

    // This should never happen
    assert!(blink != 0, "No blink");

    // Loop while we have entries in the list
    while flink != 0 {
        // Get base and length
        let base = memory.read_virt_usize(cr3, flink + 0x30)?;
        let len  = memory.read_virt_u32(cr3, flink + 0x40)? as usize;

        // Get the name length and pointer
        let namelen = memory.read_virt_u16(cr3, flink + 0x58)? as usize;
        let nameptr = memory.read_virt_usize(cr3, flink + 0x60)?;

        // Get the module information
        let time_date_stamp = memory.read_virt_u32(cr3, flink + 0x80)?;
        let size_of_image   = memory.read_virt_u32(cr3, flink + 0x40)?;

        // Skip this entry if it doesn't seem sane
        if nameptr == 0 || namelen == 0 || (namelen % 2) != 0 {
            if flink == blink { break; }
            flink = memory.read_virt_usize(cr3, flink)?;
            continue;
        }
        
        // Make room and read the UTF-16 name
        let mut name = vec![0u8; namelen];
        if memory.read_virt(cr3, nameptr, &mut name) != namelen {
            // Name might be paged out, skip entry
            if flink == blink { break; }
            flink = memory.read_virt_usize(cr3, flink)?;
            continue;
        }

        // Convert the module name into a UTF-8 Rust string
        let name_utf8 = String::from_utf16(unsafe {
            std::slice::from_raw_parts(
                name.as_ptr() as *const u16,
                name.len() / 2)
        }).expect("Failed to convert to utf8");

        // Append this to the module list
        modlist.add_module(ModuleEntry {
            info: ModuleInfo::new(name_utf8.into(),
                                  time_date_stamp, size_of_image),
            base,
            len,
        });

        // Go to the next module
        if flink == blink { break; }
        flink = memory.read_virt_usize(cr3, flink)?;
    }

    Ok(())
}

// Find the address of the `nt!PsLoadedModuleList` global
pub fn find_kernel_modlist(context: &WhvpContext,
        memory: &mut MemReader) -> Result<usize, ()> {
    // Get information about the guest state
    let cr3       = context.cr3() as usize;
    let lma       = (unsafe { context.efer.Reg64 } & (1 << 10)) != 0;
    let kernel_gs = unsafe { context.gs.Segment.Base as usize };
    let cs        = unsafe { context.cs.Segment.Selector };

    // Make sure we have a GS, and we're also 64-bit and in kernel mode
    if !(lma && (cs & 3) == 0 && (kernel_gs & (1 << 63)) != 0) {
        return Err(());
    }

    // Search virtual memory in the kernel starting at GS_BASE for 64 MiB.
    // We search for something that looks like nt!PsLoadedModuleList which
    // contains entries of type nt!_KLDR_DATA_TABLE_ENTRY
    //
    // The first entry in the list should always be 'ntoskrnl.exe' so we
    // search for that
    let mut found: Option<usize> = None;

    // Walk through memory
    for offset in (0..64 * 1024 * 1024).step_by(8) {
        let list_addr = kernel_gs + offset;

        // Attempt to read a pointer from this location
        if let Ok(flink) = memory.read_virt_usize(cr3, list_addr) {
            // _KLDR_DATA_TABLE_ENTRY.InLoadOrderLinks.Blink
            let blink = memory.read_virt_usize(cr3, flink + 0x08);

            // If the blink pointer doesn't reference the base of the list this
            // cannot be the module list
            if blink != Ok(list_addr) { continue; }

            // _KLDR_DATA_TABLE_ENTRY.BaseDllName.Length
            let size = memory.read_virt_u16(cr3, flink + 0x58);

            // _KLDR_DATA_TABLE_ENTRY.BaseDllName.Buffer
            let nameptr = memory.read_virt_usize(cr3, flink + 0x60);

            // Make sure the length is 0x18 and all reads succeeded
            if let (Ok(0x18), Ok(nameptr)) = (size, nameptr) {
                // Make room to read the name
                let mut buf = [0u8; 0x18];

                // Read the name
                if memory.read_virt(cr3, nameptr, &mut buf) == 0x18 {
                    // If it's UTF-16 'ntoskrnl.exe' then we found
                    // our list!
                    if &buf == b"n\0t\0o\0s\0k\0r\0n\0l\0.\0e\0x\0e\0" {
                        found = Some(list_addr);
                        break;
                    }
                }
            }
        }
    }

    if let Some(kml) = found {
        print!("Found nt!PsLoadedModuleList at 0x{:x}\n", kml);
    }
    
    found.ok_or(())
}

/// Walk the kernel module list. The `modlist` parameter should be obtained
/// from a successful call to `find_kernel_modlist`
/// 
/// Kernel list is at a global nt!PsLoadedModuleList
/// Dump it with a debugger with:
/// `!list -x "dt" -a "nt!_KLDR_DATA_TABLE_ENTRY" nt!PsLoadedModuleList`
/// The type for this list is `nt!_KLDR_DATA_TABLE_ENTRY`
fn get_modlist_kernel<'a>(modlist: &mut ModuleList,
        cr3: usize, lma: bool, cs: u16,
        memory: &mut MemReader, plml_ptr: usize) -> Result<(), ()> {
    // Make sure we're in long mode and in ring0
    if !(lma && (cs & 3) == 0) {
        return Err(());
    }

    // Get the first pointer to the InLoadOrderModuleList
    // This type is of _KLDR_DATA_TABLE_ENTRY
    let mut flink = memory.read_virt_usize(cr3, plml_ptr)?;
    let blink     = memory.read_virt_usize(cr3, plml_ptr + 0x8)?;

    // This should never happen
    assert!(blink != 0, "No blink");

    // Loop while we have entries in the list
    while flink != 0 {
        // Get base and length
        let base = memory.read_virt_usize(cr3, flink + 0x30)?;
        let len  = memory.read_virt_u32(cr3, flink + 0x40)? as usize;

        // Get the name length and pointer
        let namelen = memory.read_virt_u16(cr3, flink + 0x58)? as usize;
        let nameptr = memory.read_virt_usize(cr3, flink + 0x60)?;

        // Get the module information
        let time_date_stamp = memory.read_virt_u32(cr3, flink + 0x9c)?;
        let size_of_image   = memory.read_virt_u32(cr3, flink + 0x40)?;

        // Skip this entry if it doesn't seem sane
        if nameptr == 0 || namelen == 0 || (namelen % 2) != 0 {
            if flink == blink { break; }
            flink = memory.read_virt_usize(cr3, flink)?;
            continue;
        }

        // Make room and read the UTF-16 name
        let mut name = vec![0u8; namelen];
        if memory.read_virt(cr3, nameptr, &mut name) != namelen {
            // Name might be paged out, skip entry
            if flink == blink { break; }
            flink = memory.read_virt_usize(cr3, flink)?;
            continue;
        }

        // Convert the module name into a UTF-8 Rust string
        let name_utf8 = String::from_utf16(unsafe {
            std::slice::from_raw_parts(
                name.as_ptr() as *const u16,
                name.len() / 2)
        }).expect("Failed to convert to utf8");

        // Append this to the module list
        modlist.add_module(ModuleEntry {
            info: ModuleInfo::new(name_utf8.into(),
                                  time_date_stamp, size_of_image),
            base,
            len,
        });

        // Go to the next module
        if flink == blink { break; }
        flink = memory.read_virt_usize(cr3, flink)?;
    }

    Ok(())
}

/// Walk the module list for the current operating context
pub fn get_modlist<'a>(memory: &mut MemReader,
        cr3: usize, lma: bool, gs_base: usize, cs: u16,
        plml_ptr: Option<usize>) -> Result<ModuleList, ()> {

    // Create the module list we will return
    let mut ret = ModuleList::new();

    // Check which CPL we're at
    if (cs & 3) == 3 {
        // ring3
        get_modlist_user(&mut ret, cr3, lma, gs_base, cs, memory)?;
    } else if plml_ptr.is_some() {
        // kernel
        get_modlist_kernel(&mut ret, cr3, lma, cs, memory, plml_ptr.unwrap())?;
    } else {
        return Err(());
    }

    Ok(ret)
}
