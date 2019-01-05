pub trait PhysMem {
    fn alloc_page(&mut self) -> Option<*mut u8>;
    fn read_phys_int(&mut self, addr: *mut u64) -> Result<u64, &'static str>;
    fn write_phys(&mut self, addr: *mut u64, val: u64) -> Result<(), &'static str>;

    /// Check if a virtual address starting at `addr` for `length` is allowed
    /// for use.
    ///
    /// Returns true if you can safely allocate using `addr` and length,
    /// otherwise false
    fn probe_vaddr(&mut self, addr: usize, length: usize) -> bool;
}

/// Canonicalize a 64-bit address such that bits [63:48] are sign extended
/// from bit 47
pub fn canonicalize_address(addr: u64) -> u64
{
    let mut addr: i64 = addr as i64;

    /* Canon addresses are 48-bits sign extended. Do a shift left by 16 bits
     * to mask off the top bits, then do an arithmetic shift right (note i64
     * type) to sign extend the 47th bit.
     */
    addr <<= 64 - 48;
    addr >>= 64 - 48;

    addr as u64
}

/// Bits for raw page tables and page table entries
#[repr(u64)]
pub enum PTBits {
    Present        = (1 <<  0),
    Writable       = (1 <<  1),
    User           = (1 <<  2),
    WriteThrough   = (1 <<  3),
    CacheDisable   = (1 <<  4),
    Accessed       = (1 <<  5),
    Dirty          = (1 <<  6),
    PageSize       = (1 <<  7), /* Only valid for PDPTEs and PDEs */
    Global         = (1 <<  8),
    ExecuteDisable = (1 << 63),
}

/// Valid page mapping sizes
pub enum MapSize {
    Mapping1GiB,
    Mapping2MiB,
    Mapping4KiB,
}

/// Structure representing a page table
pub struct PageTable<'a, T: 'a + PhysMem> {
    backing: *mut u64,
    physmem: &'a mut T,
}

impl<'a, T: 'a + PhysMem> PageTable<'a, T>
{
    pub unsafe fn from_existing(existing: *mut u64, physmem: &'a mut T) ->
        PageTable<'a, T>
    {
        PageTable {
            physmem,
            backing: existing,
        }
    }

    /// Get a pointer to the root page table for this page table. This value
    /// is what would be put in cr3.
    pub fn get_backing(&self) -> *mut u64
    {
        self.backing
    }

    /// Translate a virtual address to a physical address using this page table
    /// Optionally dirty pages as we walk performing the translation.
    ///
    /// Returns a tuple of (physical address, page size)
    pub fn virt_to_phys_dirty(&mut self, vaddr: u64, dirty: bool) ->
        Result<Option<(u64, u64)>, &'static str>
    {
        unsafe {
            let mut cur = self.backing;

            /* Non-canonical addresses not translatable */
            if canonicalize_address(vaddr) != vaddr {
                return Err("Virtual address to virt_to_phys() not canonical");
            }
            
            /* Calculate the components for each level of the page table from
             * the vaddr.
             */
            let cr_offsets: [u64; 4] = [
                ((vaddr >> 39) & 0x1ff), /* 512 GiB */
                ((vaddr >> 30) & 0x1ff), /*   1 GiB */
                ((vaddr >> 21) & 0x1ff), /*   2 MiB */
                ((vaddr >> 12) & 0x1ff), /*   4 KiB */
            ];

            /* For each level in the page table */
            for (depth, cur_offset) in cr_offsets.iter().enumerate() {
                /* Get the page table entry */
                let entry = self.physmem.read_phys_int(cur.offset(*cur_offset as isize))?;

                /* If the entry is not present return None */
                if (entry & PTBits::Present as u64) == 0 {
                    return Ok(None);
                }

                /* Entry was present, dirty it */
                if dirty {
                    self.physmem.write_phys(cur.offset(*cur_offset as isize),
                        entry | PTBits::Accessed as u64 | PTBits::Dirty as u64)?;
                }

                /* Get the physical address of the next level */
                cur = (entry & 0xFFFFFFFFFF000) as *mut u64;

                /* Check if this is a large page */
                if (entry & PTBits::PageSize as u64) != 0 {
                    match depth {
                        /* PageSize bit set on PML4E (512 GiB page) MBZ */
                        0 => {
                            /* PS bit must be zero on PML4Es */
                            return Err("PageSize bit set on PML4E");
                        },

                        /* PageSize bit set on PDPE (1 GiB page) */
                        1 => {
                            return Ok(Some((cur as u64 + (vaddr & 0x3FFFFFFF),
                                           0x40000000)));
                        },

                        /* PageSize bit set on PDE (2 MiB page) */
                        2 => {
                            return Ok(Some((cur as u64 + (vaddr & 0x1FFFFF),
                                           0x200000)));
                        },
                        
                        /* PageSize bit is the PAT bit at PTE level */
                        _ => {},
                    }
                }
            }

            /* Return out physical address of vaddr and the entry */
            Ok(Some((cur as u64 + (vaddr & 0xfff), 0x1000)))
        }
    }

    /// Translate a virtual address to a physical address
    ///
    /// Return a tuple of (physical address, page size)
    pub fn virt_to_phys(&mut self, vaddr: u64) ->
        Result<Option<(u64, u64)>, &'static str>
    {
        self.virt_to_phys_dirty(vaddr, false)
    }

    /// Invoke a closure on each page present in this page table. Optionally
    /// if `dirty_only` is true, the closure will only be invoked for dirty
    /// pages.
    ///
    /// XXX: This is marked unsafe until it is correct for tables with large
    ///      pages.
    ///
    /// Dirty pages will be set to clean during the walk if `dirty_only` is
    /// true.
    pub unsafe fn for_each_page<F>(&mut self, dirty_only: bool, mut func: F)
        -> Result<(), &'static str>
        where F: FnMut(u64, u64)
    {
        for pml4e in 0..512u64 {
            let ent = self.backing as *mut u64;
            let tmp = self.physmem.read_phys_int(ent.offset(pml4e as isize))?;
            if (tmp & PTBits::Present as u64) == 0 { continue; }
            if dirty_only {
                if (tmp & PTBits::Accessed as u64) == 0 {
                    continue;
                }
                self.physmem.write_phys(ent.offset(pml4e as isize),
                    tmp & !(PTBits::Accessed as u64))?;
            }

            let ent = (tmp & 0xFFFFFFFFFF000) as *mut u64;

            for pdpe in 0..512u64 {
                let tmp = self.physmem.read_phys_int(ent.offset(pdpe as isize))?;
                if (tmp & 1) == 0 { continue; }
                if dirty_only {
                    if (tmp & PTBits::Accessed as u64) == 0 {
                        continue;
                    }
                    self.physmem.write_phys(ent.offset(pdpe as isize),
                                     tmp & !(PTBits::Accessed as u64))?;
                }
                let ent = (tmp & 0xFFFFFFFFFF000) as *mut u64;

                for pde in 0..512u64 {
                    let tmp = self.physmem.read_phys_int(ent.offset(pde as isize))?;
                    if (tmp & 1) == 0 { continue; }
                    if dirty_only {
                        if (tmp & PTBits::Accessed as u64) == 0 {
                            continue;
                        }
                        self.physmem.write_phys(ent.offset(pde as isize),
                                         tmp & !(PTBits::Accessed as u64))?;
                    }
                    let ent = (tmp & 0xFFFFFFFFFF000) as *mut u64;

                    for pte in 0..512u64 {
                        let tmp = self.physmem.read_phys_int(ent.offset(pte as isize))?;
                        if (tmp & 1) == 0 { continue; }
                        if dirty_only {
                            if (tmp & PTBits::Dirty as u64) == 0 {
                                continue;
                            }
                            self.physmem.write_phys(ent.offset(pte as isize),
                                tmp & !(PTBits::Dirty as u64))?;
                        }

                        let vaddr = (pml4e << 39) | (pdpe << 30) |
                            (pde << 21) | (pte << 12);
                        let paddr = tmp & 0xFFFFFFFFFF000;

                        func(vaddr as u64, paddr);
                    }
                }
            }
        }

        Ok(())
    }
}
