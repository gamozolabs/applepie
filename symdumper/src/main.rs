#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

use std::env;
use std::fs::File;
use std::io::Write;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::iter::once;

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

extern fn srcline_callback(srcline_info: *const SrcCodeInfoW, context: Context) -> bool {
    let srcline = unsafe { &*srcline_info };
    let context = unsafe { &mut *context };

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

extern fn sym_callback(sym_info: *const SymbolInfoW, size: u32, context: Context) -> bool {
    let symbol = unsafe { &*sym_info };
    let context = unsafe { &mut *context };

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
                      callback: extern fn(sym_info: *const SymbolInfoW, size: u32, context: Context) -> bool, 
                      context: Context) -> bool;

    fn SymEnumSourceLinesW(hProcess: HANDLE, Base: u64, Obj: usize, File: usize, Line: u32,
                           Flags: u32, callback: extern fn(LineInfo: *const SrcCodeInfoW, UserContext: Context) -> bool,
                           UserContext: Context) -> bool;
}

pub fn win16_for_str(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

#[derive(Serialize)]
#[repr(C)]
struct SymbolContext {
    symbols:    Vec<(u64, String, u64)>,
    sourceline: Vec<(u64, String, u64)>,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        print!("Usage: symdumper <PE file to dump> <output json> <symbol path>\n");
        return;
    }

    let mut symdb = SymbolContext {
        symbols: Vec::new(),
        sourceline: Vec::new(),
    };

    let module_base;

    unsafe {
        let cur_process = GetCurrentProcess();

        // Initialize the symbol library for this process
        let sympath = win16_for_str(&args[3]);
        assert!(SymInitializeW(cur_process, sympath.as_ptr(), false),
                "Failed to SymInitializeW()");

        // Load up a module into the current process as the base address
        // the file specified
        let filename = win16_for_str(&args[1]);
        module_base = SymLoadModuleExW(cur_process, 0, filename.as_ptr(), std::ptr::null(), 0, 0, 0, 0);
        assert!(module_base != 0, "Failed to SymLoadModuleExW()");

        print!("Loaded library to 0x{:x}\n", module_base);

        // Get information about the module we just loaded
        let mut module_info = ImagehlpModule64W::default();
        assert!(SymGetModuleInfoW64(cur_process, module_base,
                                   &mut module_info as *mut _),
            "Failed to SymGetModuleInfoW64()");

        // This is pedantic but we might as well check it
        assert!(module_info.BaseOfImage == module_base);

        print!("Module size {:x}\n", module_info.ImageSize);

        assert!(SymEnumSymbolsW(cur_process, module_base, 0, sym_callback, &mut symdb as *mut _));
        print!("Enumerated symbols\n");
        if !SymEnumSourceLinesW(cur_process, module_base, 0, 0, 0, 0, srcline_callback, &mut symdb as *mut _) {
            print!("Warning: Could not enumerate sourcelines\n");
        } else {
            print!("Enumerated lines\n");
        }
    }

    symdb.symbols.sort_by_key(|x| x.0);
    print!("Sorted symbol info\n");

    symdb.sourceline.sort_by_key(|x| x.0);
    print!("Sorted sourceline info\n");

    let serialized = serde_json::to_string(&symdb).unwrap();
    let mut fd = File::create(&args[2]).expect("Failed to create symbol file");
    fd.write_all(serialized.as_bytes()).expect("Failed to write symbol file");
}
