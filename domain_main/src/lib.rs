use proc_macro2::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn domain_main(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = TokenStream::from(item);
    let panic = panic_impl();
    quote! (
        #[no_mangle]
        #item
        #panic
    )
    .into()
}

fn panic_impl() -> TokenStream {
    quote!(
        #[lang = "eh_personality"]
        #[no_mangle]
        pub extern "C" fn rust_eh_personality() {
            basic::println_color!(31, "rust_eh_personality called");
            loop {}
        }

        #[allow(non_snake_case)]
        #[no_mangle]
        extern "C" fn _Unwind_Resume(arg: usize) -> ! {
            basic::println_color!(31, "Unwind resume arg {:#x}", arg);
            unwind::unwind_resume(arg);
        }

        #[panic_handler]
        fn panic(info: &PanicInfo) -> ! {
            if let Some(p) = info.location() {
                basic::println_color!(
                    31,
                    "line {}, file {}: {}",
                    p.line(),
                    p.file(),
                    info.message().unwrap()
                );
            } else {
                basic::println_color!(31, "no location information available");
            }
            basic::backtrace(domain_id());
            static FAKE_LOCK: basic::sync::Mutex<()> = basic::sync::Mutex::new(());
            #[cfg(feature = "rust-unwind")]
            {
                let _guard = FAKE_LOCK.lock();
                unwind::unwind_from_panic(3);
            }
        }
    )
    .into()
}
