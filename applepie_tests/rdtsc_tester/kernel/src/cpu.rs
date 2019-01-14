/// Performs a rdtsc instruction, returns 64-bit TSC value
#[inline]
pub fn rdtsc() -> u64 {
	let (high, low): (u32, u32);

    unsafe {
        asm!("rdtsc" :
             "={edx}"(high), "={eax}"(low) :::
             "volatile", "intel");
    }

	((high as u64) << 32) | (low as u64)
}

#[inline]
pub unsafe fn invlpg(addr: usize)
{
    asm!("invlpg [$0]" :: "r"(addr) : "memory" : "volatile", "intel");
}

/// Output the byte `val` to `port`
#[inline]
pub unsafe fn out8(port: u16, val: u8) {
    asm!("out dx, al" :: "{al}"(val), "{dx}"(port) :: "intel", "volatile");
}

/// Input a byte from `port`
#[inline]
pub unsafe fn in8(port: u16) -> u8 {
    let ret: u8;
    asm!("in al, dx" : "={al}"(ret) : "{dx}"(port) :: "intel", "volatile");
    ret
}

/// Output the dword `val` to `port`
#[inline]
pub unsafe fn out32(port: u16, val: u32) {
    asm!("out dx, eax" :: "{eax}"(val), "{dx}"(port) :: "intel", "volatile");
}

/// Input a dword from `port`
#[inline]
pub unsafe fn in32(port: u16) -> u32 {
    let ret: u32;
    asm!("in eax, dx" : "={eax}"(ret) : "{dx}"(port) :: "intel", "volatile");
    ret
}

/// Disable interrupts and halt forever
pub fn halt() -> ! {
    loop {
        unsafe {
            asm!("cli ; hlt" :::: "volatile");
        }
    }
}

/// Load the interrupt table specified by vaddr
#[inline]
pub unsafe fn lidt(vaddr: *const u8)
{
	asm!("lidt [$0]" ::
		 "r"(vaddr) :
		 "memory" :
		 "volatile", "intel");
}

/// Reads the contents of CR2
#[inline]
pub fn read_cr2() -> u64 {
    unsafe {
        let cr2: u64;
        asm!("mov $0, cr2" : "=r"(cr2) ::: "intel", "volatile");
        cr2
    }
}

/// Reads the contents of CR3
#[inline]
pub fn read_cr3() -> u64 {
    unsafe {
        let cr3: u64;
        asm!("mov $0, cr3" : "=r"(cr3) ::: "intel", "volatile");
        cr3 & 0xffff_ffff_ffff_f000
    }
}

/// Write back all memory and invalidate caches
#[inline]
pub fn wbinvd() {
    unsafe {
    	asm!("wbinvd" ::: "memory" : "volatile", "intel");
    }
}

/// Memory fence for both reads and writes
#[inline]
pub fn mfence() {
    unsafe {
    	asm!("mfence" ::: "memory" : "volatile", "intel");
    }
}

#[inline]
pub fn lfence() {
    unsafe {
    	asm!("lfence" ::: "memory" : "volatile", "intel");
    }
}

/// Flushes cache line associted with the byte pointed to by `ptr`
#[inline]
pub unsafe fn clflush(ptr: *const u8) {
    asm!("clflush [$0]" :: "r"(ptr as usize) : "memory" : "volatile", "intel");
}

/// Instruction fence (via write cr2) which serializes execution
#[inline]
pub fn ifence() {
    unsafe {
    	write_cr2(0);
    }
}

/// Writes to CR2
#[inline]
pub unsafe fn write_cr2(val: u64)
{
    asm!("mov cr2, $0" :: "r"(val) : "memory" : "intel", "volatile");
}
