use core::fmt::{Arguments, Write};

use ksync::Mutex;
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        let domain_id = rref::domain_id();
        let mut id: usize;
        unsafe {
            core::arch::asm!(
            "mv {},tp", out(reg)id,
            );
        }
        $crate::console::__print(format_args!("[{}][Domain:{}] {}", id,domain_id, format_args!($($arg)*)))
    };
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(
        concat!($fmt, "\n"), $($arg)*));
}

pub struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        corelib::write_console(s);
        Ok(())
    }
}

static STDOUT: Mutex<Stdout> = Mutex::new(Stdout);
pub fn __print(args: Arguments) {
    STDOUT.lock().write_fmt(args).unwrap();
}
