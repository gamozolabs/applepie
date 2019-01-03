use crate::MemReader;
use crate::whvp::WhvpContext;
use std::fmt::Write;

/// Module entry
pub struct ModuleEntry {
    // Base address of the module
    base: usize,

    // Length (in bytes) of the module
    len: usize,

    // Module name
    name: String,
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
    pub fn get_modoff(&self, vaddr: usize) -> (Option<&str>, usize) {
        for module in &self.modules {
            if vaddr >= module.base &&
                    vaddr < module.base.checked_add(module.len).unwrap() {
                let offset = vaddr - module.base;
                return (Some(&module.name), offset);
            }
        }

        (None, vaddr)
    }

    /// Get the module offset representation of a virtual address
    pub fn get_modoff_string_int(&self, vaddr: usize, output: &mut String) {
        output.clear();

        let (modname, offset) = self.get_modoff(vaddr);
        if let Some(modname) = modname {
            write!(output, "{}+", modname).unwrap();
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
pub fn get_modlist_user(context: &WhvpContext,
        memory: &mut MemReader) -> Result<ModuleList, ()> {
    let mut ret = ModuleList::new();

    // Get information about the guest state
    let cr3 = unsafe { context.cr3.Reg64 } as usize;
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
            base,
            len,
            name: name_utf8
        });

        // Go to the next module
        if flink == blink { break; }
        flink = memory.read_virt_usize(cr3, flink)?;
    }

    Ok(ret)
}
