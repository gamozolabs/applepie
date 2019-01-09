use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::iter::once;
use std::process::Command;
use std::io::{Error, ErrorKind};
use crate::win32::ModuleInfo;

type HANDLE = usize;

extern {
    fn GetCurrentProcess() -> HANDLE;
}

#[allow(non_snake_case)]
#[repr(C)]
struct SrcCodeInfoW {
    SizeOfStruct: u32,
    Key: usize,
    ModBase: u64,
    Obj: [u16; 261],
    FileName: [u16; 261],
    LineNumber: u32,
    Address: u64,
}

impl Default for SrcCodeInfoW {
    fn default() -> Self {
        SrcCodeInfoW {
            SizeOfStruct: std::mem::size_of::<SrcCodeInfoW>() as u32,
            Key: 0,
            ModBase: 0,
            Obj: [0; 261],
            FileName: [0; 261],
            LineNumber: 0,
            Address: 0,
        }
    }
}

#[allow(non_snake_case)]
#[repr(C)]
struct SymbolInfoW {
    SizeOfStruct: u32,
    TypeIndex: u32,
    Reserved: [u64; 2],
    Index: u32,
    Size: u32,
    ModBase: u64,
    Flags: u32,
    Value: u64,
    Address: u64,
    Register: u32,
    Scope: u32,
    Tag: u32,
    NameLen: u32,
    MaxNameLen: u32,

    // technically this field is dynamically sized as specified by MaxNameLen
    Name: [u16; 8192],
}

impl Default for SymbolInfoW {
    fn default() -> Self {
        SymbolInfoW {
            // Subtract off the size of the dynamic component, one byte
            // already included in the structure
            SizeOfStruct: std::mem::size_of::<SymbolInfoW>() as u32 - 8192*2,

            TypeIndex: 0,
            Reserved: [0; 2],
            Index: 0,
            Size: 0,
            ModBase: 0,
            Flags: 0,
            Value: 0,
            Address: 0,
            Register: 0,
            Scope: 0,
            Tag: 0,
            NameLen: 0,
            MaxNameLen: 8192,
            Name: [0; 8192],
        }
    }
}

#[allow(non_snake_case)]
#[repr(C)]
struct ImagehlpLineW64 {
    SizeOfStruct: u32,
    Key: usize,
    LineNumber: u32,
    FileName: *const u16,
    Address: u64,
}

impl Default for ImagehlpLineW64 {
    fn default() -> Self {
        ImagehlpLineW64 {
            SizeOfStruct: std::mem::size_of::<ImagehlpLineW64>() as u32,
            Key: 0,
            LineNumber: 0,
            FileName: std::ptr::null(),
            Address: 0,
        }
    }
}

#[allow(non_snake_case)]
#[repr(C)]
struct ImagehlpModule64W {
    SizeOfStruct: u32,
    BaseOfImage: u64,
    ImageSize: u32,
    TimeDateStamp: u32,
    CheckSum: u32,
    NumSyms: u32,
    SymType: u32,
    ModuleName: [u16; 32],
    ImageName: [u16; 256],
    LoadedImageName: [u16; 256],
    LoadedPdbName: [u16; 256],
    CVSig: u32,
    CVData: [u16; 780],
    PdbSig: u32,
    PdbSig70: [u8; 16],
    PdbAge: u32,
    PdbUnmatched: bool,
    DbgUnmatched: bool,
    LineNumbers: bool,
    GlobalSymbols: bool,
    TypeInfo: bool,
    SourceIndexed: bool,
    Publics: bool,
}

impl Default for ImagehlpModule64W {
    fn default() -> Self {
        ImagehlpModule64W {
            SizeOfStruct: std::mem::size_of::<ImagehlpModule64W>() as u32,
            BaseOfImage:  0,
            ImageSize:    0,
            TimeDateStamp: 0,
            CheckSum: 0,
            NumSyms: 0,
            SymType: 0,
            ModuleName: [0; 32],
            ImageName: [0; 256],
            LoadedImageName: [0; 256],
            LoadedPdbName: [0; 256],
            CVSig: 0,
            CVData: [0; 780],
            PdbSig: 0,
            PdbSig70: [0; 16],
            PdbAge: 0,
            PdbUnmatched: false,
            DbgUnmatched: false,
            LineNumbers: false,
            GlobalSymbols: false,
            TypeInfo: false,
            SourceIndexed: false,
            Publics: false,
        }
    }
}

/// Vector of (virtual address, symbol name, symbol size)
type Context = *mut SymbolContext;

extern fn srcline_callback(srcline_info: *const SrcCodeInfoW, context: usize) -> bool {
    let srcline = unsafe { &*srcline_info };
    let context = unsafe { &mut *(context as Context) };

    let mut filename = Vec::with_capacity(srcline.FileName.len());
    for &val in srcline.FileName.iter() {
        if val == 0 { break; }
        filename.push(val);
    }
    
    let source_filename = String::from_utf16(&filename)
        .expect("Failed to decode UTF-16 file name");

    context.sourceline.push((srcline.Address - srcline.ModBase, source_filename, srcline.LineNumber as u64));
    true
}

extern fn sym_callback(sym_info: *const SymbolInfoW, size: u32, context: usize) -> bool {
    let symbol = unsafe { &*sym_info };
    let context = unsafe { &mut *(context as Context) };

    // Technically NameLen isn't supposed to contain the null terminator... but it does.
    // Yay!
    if symbol.NameLen < 1 {
        return true;
    }

    let symbol_name = String::from_utf16(&symbol.Name[..symbol.NameLen as usize - 1])
        .expect("Failed to decode UTF-16 symbol name");

    context.symbols.push((symbol.Address - symbol.ModBase, symbol_name, size as u64));
    true
}

#[link(name = "dbghelp")]
extern {
    fn SymInitializeW(hProcess: HANDLE, UserSearchPath: *const u16,
                      fInvadeProcess: bool) -> bool;

    fn SymLoadModuleExW(hProcess: HANDLE, hFile: HANDLE, ImageName: *const u16,
                        ModuleName: *const u16, BaseOfDll: u64, DllSize: u32,
                        Data: usize, Flags: u32) -> u64;

    fn SymGetModuleInfoW64(hProcess: HANDLE, dwAddr: u64,
                          ModuleInfo: *mut ImagehlpModule64W) -> bool;

    fn SymEnumSymbolsW(hProcess: HANDLE, BaseOfDll: u64, Mask: usize,
                      callback: extern fn(sym_info: *const SymbolInfoW, size: u32, context: usize) -> bool, 
                      context: usize) -> bool;

    fn SymEnumSourceLinesW(hProcess: HANDLE, Base: u64, Obj: usize, File: usize, Line: u32,
                           Flags: u32, callback: extern fn(LineInfo: *const SrcCodeInfoW, UserContext: usize) -> bool,
                           UserContext: usize) -> bool;

    fn SymUnloadModule64(hProcess: HANDLE, BaseOfDll: u64) -> bool;

    fn SymCleanup(hProcess: HANDLE) -> bool;
}

pub fn win16_for_str(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

#[repr(C)]
#[derive(Clone, Default)]
pub struct SymbolContext {
    pub symbols:    Vec<(u64, String, u64)>,
    pub sourceline: Vec<(u64, String, u64)>,
}

/// Get all of the symbols from a PE file `pe_file`
pub fn get_symbols_from_file(pe_file: &str) -> SymbolContext {
    let mut symdb = SymbolContext {
        symbols:    Vec::new(),
        sourceline: Vec::new(),
    };

    let module_base;

    unsafe {
        let cur_process = GetCurrentProcess();

        // Initialize the symbol library for this process
        assert!(SymInitializeW(cur_process, 0 as *const _, false),
                "Failed to SymInitializeW()");

        // Load up a module into the current process as the base address
        // the file specified
        let filename = win16_for_str(pe_file);
        module_base = SymLoadModuleExW(cur_process, 0, filename.as_ptr(), std::ptr::null(), 0, 0, 0, 0);
        assert!(module_base != 0, "Failed to SymLoadModuleExW()");

        // Get information about the module we just loaded
        let mut module_info = ImagehlpModule64W::default();
        assert!(SymGetModuleInfoW64(cur_process, module_base,
                                   &mut module_info as *mut _),
            "Failed to SymGetModuleInfoW64()");

        // This is pedantic but we might as well check it
        assert!(module_info.BaseOfImage == module_base);

        assert!(SymEnumSymbolsW(cur_process, module_base, 0, sym_callback, &mut symdb as *mut _ as usize));
        if !SymEnumSourceLinesW(cur_process, module_base, 0, 0, 0, 0, srcline_callback, &mut symdb as *mut _ as usize) {
            // Eh just silently fail here, most people won't have private
            // symbols so this would just spam
            //print!("Warning: Could not enumerate sourcelines\n");
        }

        assert!(SymUnloadModule64(cur_process, module_base),
            "Failed to SymUnloadModule64()");

        assert!(SymCleanup(cur_process), "Failed to SymCleanup()");
    }

    symdb.symbols.sort_by_key(|x| x.0);
    symdb.sourceline.sort_by_key(|x| x.0);

    symdb
}

/// Get all of the symbols from a module `module_name` with a TimeDateStamp
/// and SizeOfImage from the PE header. This will automatically download the
/// module and PDB from the symbol store using symchk
pub fn get_symbols_from_module(module: &ModuleInfo)
    -> std::io::Result<SymbolContext>
{
    // Use symchk to download the module and symbols
    let module = download_symbol(module.name(), module.time(), module.size())?;
    Ok(get_symbols_from_file(&module))
}

/// Download a module and the corresponding PDB based on module_name,
/// it's TimeDateStamp and SizeOfImage from it's PE header
/// 
/// Returns a string containing a filename of the downloaded module
fn download_symbol(module_name: &str, timedatestamp: u32, sizeofimage: u32)
        -> std::io::Result<String> {
    let mut dir = std::env::temp_dir();
    dir.push("applepie_manifest");

    // Create manifest file for symchk
    std::fs::write(&dir, format!("{},{:x}{:x},1\r\n",
        module_name, timedatestamp, sizeofimage))?;

    // Run symchk to download this module
    let res = Command::new("symchk")
        .arg("/v")
        .arg("/im")
        .arg(dir)
        .output()?;
    if !res.status.success() {
        return Err(Error::new(ErrorKind::Other, "symchk returned with error"));
    }

    // Symchk apparently ran, check output
    let stderr = std::str::from_utf8(&res.stderr)
        .expect("Failed to convert symchk output to utf-8");

    let mut filename = None;
    for line in stderr.lines() {
        const PREFIX:  &'static str = "DBGHELP: ";
        const POSTFIX: &'static str = " - OK";

        // The line that contains the filename looks like:
        // DBGHELP: C:\symbols\calc.exe\8f598a9eb000\calc.exe - OK
        if !line.starts_with(PREFIX) { continue; }
        if !line.ends_with(POSTFIX) { continue; }

        // We only expect one line of output to match the above criteria
        // If there are multiple we'll need to improve this "parser"
        assert!(filename.is_none(), "Multiple filenames in symchk output");

        // Save the filename we found
        filename = Some(&line[PREFIX.len()..line.len() - POSTFIX.len()]);
    }

    // Fail hard if we didn't get the output filename from symchk
    let filename = filename.expect("Did not get expected symchk output");

    // Run symchk to download the pdb for the file
    let res = Command::new("symchk")
        .arg(filename)
        .output()?;
    if !res.status.success() {
        return Err(Error::new(ErrorKind::Other, "symchk returned with error"));
    }

    // Now we have downloaded the PDB for this file :)
    Ok(filename.into())
}

#[test]
fn test_symchk() {
    download_symbol("calc.exe", 0x8F598A9E, 0xB000)
        .expect("Failed to download symbol");
}
