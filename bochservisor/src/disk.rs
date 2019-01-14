/// applepie disk emulation
/// This is just a flat file disk format but with some diff support to make it
/// easy to open it for read-only use and discard changes

use std::fs::{File, OpenOptions};
use std::cell::RefCell;
use std::io::{Read, Seek, SeekFrom, Write};
use std::collections::HashMap;

thread_local! {
    /// State of a currently active virtual disk
    static DISK_STATE: RefCell<Option<VirtualDisk>> = RefCell::new(None);
}

/// Virtual disk state
struct VirtualDisk {
    /// Name of the file
    _filename: String,

    /// Unique file descriptor given to Bochs and to track this disk
    fd: i32,

    /// Actual Rust file descriptor for this file
    backing: File,

    /// Length of the file
    length: u64,

    /// Storage for disk data if we're in a volatile mode
    /// This HashMap converts a sector number to the corresponding sector data
    /// If this store is `None`, the disk is not in volatile mode and thus
    /// writes go to disk.
    volatile_store: Option<HashMap<u64, [u8; 512]>>,
}

/// Discard all changes made to the vdisk. This is used to reset the disk to
/// its original state during a restore
pub fn vdisk_discard_changes() {
    DISK_STATE.with(|x| {
        let mut x = x.borrow_mut();
        
        // Handle cases where no vdisk is used
        if x.is_none() { return; }

        let vdisk = x.as_mut().unwrap();

        // Drop all entries in the `HashMap`
        vdisk.volatile_store.as_mut()
            .expect("Attempted to discard changes to non-volatile disk")
            .clear();
    });
}

/// Disable volatile mode for the vdisk
pub fn vdisk_set_non_volatile() {
    DISK_STATE.with(|x| {
        let mut x = x.borrow_mut();

        // Handle cases where no vdisk is used
        if x.is_none() { return; }

        let vdisk = x.as_mut().unwrap();

        // If the disk is already non-volatile, just return out
        if vdisk.volatile_store.is_none() {
            return;
        }

        // Make sure there are no temporary stores cached in the volatile_store
        // We could allow this in future if we wrote out the changes
        assert!(vdisk.volatile_store.as_ref().unwrap().len() == 0,
            "Cannot go non-volatile for vdisk with pending stores");

        // Disable the volatile store
        vdisk.volatile_store = None;

        print!("Set vdisk to non-volatile mode\n");
    });
}

#[no_mangle]
pub extern "C" fn vdisk_open(path: *const u8, _flags: i32) -> i32 {
    // Just return out the file descriptor
    DISK_STATE.with(|x| {
        let mut x = x.borrow_mut();

        // Convert the null-terminated C string to a Rust string
        let filename = unsafe { crate::string_from_cstring(path) };
        assert!(filename.is_some(), "Failed to get filename for vdisk_open()");

        // Disk should only ever be opened once at a time
        assert!(x.is_none(), "Attempted to open multiple disks");

        // Open the file for RW
        let backing = OpenOptions::new()
            .read(true)
            .write(true)
            .open(filename.as_ref().unwrap())
            .expect("Failed to open disk backing file for disk");

        // Get the length and validate it
        let length = backing.metadata()
            .expect("Failed to get hard disk metadata").len();
        assert!(length > 0 && (length % 512) == 0,
            "Disk was empty or not a 512-byte aligned size");

        // Create a new disk
        // By default new disks are _always_ in volatile mode and thus will
        // not update the underlying file. This can be disabled with
        // `vdisk_set_non_volatile` prior to any modifications to the disk
        *x = Some(VirtualDisk {
            _filename: filename.unwrap(),
            fd:        1,
            backing:   backing,
            length,
            volatile_store: Some(HashMap::new()),
        });

        // Return the fd
        x.as_ref().unwrap().fd
    })
}

#[no_mangle]
pub extern "C" fn vdisk_read(_vuint: i32, blk: u64, buf: *mut u8) -> bool {
    DISK_STATE.with(|x| {
        let mut x = x.borrow_mut();
        let vdisk = x.as_mut().unwrap();

        // Create Rust slice for output
        let output_buf = unsafe {
            std::slice::from_raw_parts_mut(buf, 512)
        };

        // Compute and bounds check the LBA
        let lba = blk.checked_mul(512).expect("Integer overflow on block");
        assert!(lba < vdisk.length, "Out-of-bounds disk access");

        // Return sectors from the volatile store if we're in volatile mode and
        // the sector is in the database
        if let Some(volatile_store) = &vdisk.volatile_store {
            if let Some(sector) = volatile_store.get(&blk) {
                // Copy the data from the cache and return that we succeeded
                output_buf.copy_from_slice(sector);
                return true;
            }
        }

        // Seek to block offset
        vdisk.backing.seek(SeekFrom::Start(lba)).expect("Failed to seek");

        // Read the entire sector
        vdisk.backing.read_exact(output_buf).expect("Failed to read sector");

        true
    })
}

#[no_mangle]
pub extern "C" fn vdisk_write(_vuint: i32, blk: u64, buf: *const u8) -> bool {
    DISK_STATE.with(|x| {
        let mut x = x.borrow_mut();
        let vdisk = x.as_mut().unwrap();

        // Create Rust slice for output
        let input_buf = unsafe {
            std::slice::from_raw_parts(buf, 512)
        };

        // Compute and bounds check the LBA
        let lba = blk.checked_mul(512).expect("Integer overflow on block");
        assert!(lba < vdisk.length, "Out-of-bounds disk access");

        // If the volatile store is present, all writes go to the store rather
        // than the actual disk
        if let Some(volatile_store) = &mut vdisk.volatile_store {
            if let Some(sector) = volatile_store.get_mut(&blk) {
                // Update the sector in the store
                sector.copy_from_slice(input_buf);
                return true;
            } else {
                // Insert a new sector into the store
                let mut buf = [0u8; 512];
                buf.copy_from_slice(input_buf);
                volatile_store.insert(blk, buf);
                return true;
            }
        }

        // Seek to block offset
        vdisk.backing.seek(SeekFrom::Start(lba)).expect("Failed to seek");

        // Write out the entire sector and flush the write to disk
        vdisk.backing.write_all(input_buf).expect("Failed to write sector");
        vdisk.backing.flush().expect("Failed to flush sector");

        true
    })
}

#[no_mangle]
pub extern "C" fn vdisk_close(_vuint: i32) {
    // Free the vdisk by replacing it with a `None` value
    DISK_STATE.with(|x| {
        let mut x = x.borrow_mut();
        *x = None;
    });
}

#[no_mangle]
pub extern "C" fn vdisk_get_size(_vuint: i32) -> u64 {
    // Get the disk size in sectors
    DISK_STATE.with(|x| {
        x.borrow().as_ref().unwrap().length / 512
    })
}
