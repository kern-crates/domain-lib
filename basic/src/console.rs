use core::fmt::{Arguments, Write};
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
pub fn __print(args: Arguments) {
    Stdout.write_fmt(args).unwrap();
}
