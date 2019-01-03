use std::collections::HashMap;

/// Structure representing symbol information for a module
#[derive(Serialize, Deserialize)]
#[repr(C)]
struct SymbolContext {
    /// Symbols in format (offset, symbol name, size of symbol)
    symbols: Vec<(u64, String, u64)>,

    /// Source line in format ???
    sourceline: Vec<(u64, String, u64)>,
}

/// Structure representing all symbols
#[derive(Default)]
pub struct Symbols {
    /// Symbols per module name
    modules: HashMap<String, SymbolContext>,
}

impl Symbols {
    /// Load up all symbols from a folder "symbols" in the current directory
    pub fn load() -> Result<Symbols, std::io::Error> {
        let mut ret = Symbols { modules: HashMap::new() };

        // Go through each file in the folder
        for entry in std::fs::read_dir("symbols")? {
            let entry = entry?;
            let path = entry.path();

            // We only care about json files
            if path.is_file() && path.to_str().unwrap().ends_with(".json") {
                // Convert to lowercase
                let filename = path.file_name().unwrap()
                    .to_str().unwrap().to_lowercase();

                // Get the name excluding the extension as the module name
                let modname = &filename[..filename.len()-5];

                print!("Loading symbols for file {:?} | {}\n", path, modname);

                // Read the contents and parse the JSON
                let contents = std::fs::read_to_string(&path)?;
                let deserialized: SymbolContext =
                    serde_json::from_str(&contents)?;

                // Print symbol status
                print!("Loaded {} symbols {} sourcelines\n",
                    deserialized.symbols.len(),
                    deserialized.sourceline.len());

                // Insert this module listing
                ret.modules.insert(modname.into(), deserialized);
            }
        }

        Ok(ret)
    }

    pub fn resolve(&self, module: &str, offset: usize) -> Option<String> {
        // Normalize the name to lowercase so it's case insensitive
        let module = module.to_lowercase();

        // Look up the module
        if let Some(context) = self.modules.get(&module) {
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
                        module, context.symbols[ii].1, offset))
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
                        module, context.symbols[ii-1].1, offset))
                }
            }
        } else {
            // No symbols for this module
            None
        }
    }
}
