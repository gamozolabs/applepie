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
pub struct ModuleEntry {
    /// Info to uniquely identify this module
    info: ModuleInfo<'static>,

    /// Base address of the module
    base: usize,

    /// Length (in bytes) of the module
    len: usize,
}

/// Group of modules
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
pub fn get_modlist_user<'a>(context: &WhvpContext,
        memory: &mut MemReader) -> Result<ModuleList, ()> {
    let mut ret = ModuleList::new();

    // Get information about the guest state
    let cr3 = context.cr3() as usize;
    let lma = (unsafe { context.efer.Reg64 } & (1 << 10)) != 0;
    let gs_base = unsafe { context.gs.Segment.Base } as usize;

    // Make sure we have a GS, we're in userspace, and we're also 64-bit
    if !(gs_base != 0 && lma &&
            (unsafe { context.cs.Segment.Selector } & 3) == 3) {
        return Ok(ret);
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
        let len  = memory.read_virt_usize(cr3, flink + 0x40)? as u32 as usize;

        // Get the name length and pointer
        let namelen = memory.read_virt_usize(cr3, flink + 0x58)? as u16 as usize;
        let nameptr = memory.read_virt_usize(cr3, flink + 0x60)?;

        // Get the module information
        let time_date_stamp = memory.read_virt_usize(cr3, flink + 0x80)? as u32;
        let size_of_image   = memory.read_virt_usize(cr3, flink + 0x40)? as u32;

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
        ret.add_module(ModuleEntry {
            info: ModuleInfo::new(name_utf8.into(),
                                  time_date_stamp, size_of_image),
            base,
            len,
        });

        // Go to the next module
        if flink == blink { break; }
        flink = memory.read_virt_usize(cr3, flink)?;
    }

    Ok(ret)
}
