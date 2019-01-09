use std::collections::HashMap;
use crate::win32::ModuleInfo;
use crate::symdumper::{get_symbols_from_module, SymbolContext};

/// Structure representing all symbols
#[derive(Default)]
pub struct Symbols {
    /// Symbols per module name
    modules: HashMap<ModuleInfo, SymbolContext>,
}

impl Symbols {
    /// Load the symbols for a module `module_name` with TimeDateStamp and
    /// SizeOfImage from their PE header
    fn load_win32(&mut self, module: &ModuleInfo) -> Result<(), std::io::Error>{
        // Already loaded
        if self.modules.contains_key(module) { return Ok(()); }

        if let Ok(symbols) = get_symbols_from_module(module) {
            print!("Loaded symbols for {:x?}\n", module);

            // Update the database
            self.modules.insert(module.clone(), symbols);
        } else {
            // Failed to download the symbols, create an empty entry in the
            // `HashMap` so we don't keep trying to re-download
            self.modules.insert(module.clone(), SymbolContext::default());
        }

        Ok(())
    }

    /// Lookup a symbol based on a module and offset
    pub fn resolve(&mut self, module: &ModuleInfo, offset: usize)
            -> Option<String> {
        // Attempt to load symbols for this module
        if self.load_win32(module).is_err() {
            return None;
        }

        // Look up the module
        if let Some(context) = self.modules.get(module) {
            // Look for nearest symbol
            let search = context.symbols
                .binary_search_by_key(&offset, |x| x.0 as usize);

            match search {
                Ok(ii) => {
                    // Direct match, there should be 0 offset as the offset
                    // matched the symbol offset

                    // Compute offset
                    let offset = offset
                        .checked_sub(context.symbols[ii].0 as usize)
                        .expect("Uh oh, integer underflow");

                    // Create symbol+offset
                    Some(format!("{}!{}+0x{:x}",
                        module.name(), context.symbols[ii].1, offset))
                }
                Err(ii) => {
                    // Could not find direct match, `ii` is the insertion point
                    // to maintain sorted order
                    
                    // If `ii` is zero that means the only insertion point is
                    // prior to all entries. Thus we don't have a symbol
                    if ii == 0 { return None; }

                    // Compute offset based on the symbol prior to the insertion
                    // point, which will be the nearest symbol lower than our
                    // offset
                    let offset = offset
                        .checked_sub(context.symbols[ii-1].0 as usize)
                        .expect("Uh oh, integer underflow");

                    // Create symbol+offset
                    Some(format!("{}!{}+0x{:x}",
                        module.name(), context.symbols[ii-1].1, offset))
                }
            }
        } else {
            // No symbols for this module
            None
        }
    }
}
