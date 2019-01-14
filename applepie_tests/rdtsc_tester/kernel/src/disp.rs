use core;
use core::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};

#[macro_export]
macro_rules! print {
    ( $($arg:tt)* ) => ({
        use core::fmt::Write;
        let _ = write!(&mut ::disp::Writer, $($arg)*);
    })
}

/// Writer implementation used by the `print!` macro
pub struct Writer;

impl core::fmt::Write for Writer
{
    fn write_str(&mut self, s: &str) -> core::fmt::Result
    {
        print_str(s);
        Ok(())
    }
}

fn scroll_screen() {
    // Alias the screen as 2 copies. One readable and one writable
    let screen = unsafe {
        core::slice::from_raw_parts(0xb8000 as *const u16, 80 * 25)
    };
    let screen_mut = unsafe {
        core::slice::from_raw_parts_mut(0xb8000 as *mut u16, 80 * 25)
    };

    // Scroll the screen up
    screen_mut[..80*24].copy_from_slice(&screen[80..]);

    // Clear the final line
    for character in screen_mut[80*24..].iter_mut() {
        *character = 0;
    }
}

fn print_str(string: &str) {
    static CURSOR_POSITION: AtomicUsize = ATOMIC_USIZE_INIT;
    static COLOR:           AtomicUsize = AtomicUsize::new(0x0f00);

    // Pointer to the screen
    let screen_mut = unsafe {
        core::slice::from_raw_parts_mut(0xb8000 as *mut u16, 80 * 25)
    };

    // Load the cursor position
    let mut ii    = CURSOR_POSITION.load(Ordering::SeqCst);
    let mut color = COLOR.load(Ordering::SeqCst) as u16;

    let mut color_latch = false;

    for byte in string.bytes() {
        // Update color if it was latched via the \0 prefix
        if color_latch {
            assert!(byte <= 0xf, "Invalid color");
            color = (byte as u16) << 8;
            color_latch = false;
            continue;
        }

        // Scroll on newlines and don't actually print the character
        if byte == b'\n' {
            scroll_screen();
            ii = 0;
            continue;
        }

        // Reset the cursor on carriage returns and don't actually print the
        // character
        if byte == b'\r' {
            ii = 0;
            continue;
        }

        // \0 is the color prefix
        if byte == b'\0' {
            color_latch = true;
            continue;
        }

        // Scroll on line filling up
        if ii == 80 {
            scroll_screen();
            ii = 0;
        }

        // Write out the character
        screen_mut[80*24 + ii] = color | byte as u16;
        ii += 1;
    }

    // Store off the result cursor position
    CURSOR_POSITION.store(ii, Ordering::SeqCst);
    COLOR.store(color as usize, Ordering::SeqCst);
}

