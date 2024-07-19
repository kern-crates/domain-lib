use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{FnArg, ItemTrait, ReturnType, TraitItem, TraitItemFn};

use crate::{
    common::{
        collect_func_info, gen_trampoline_info, resource_code, FuncInfo, ResourceCode,
        TrampolineInfo,
    },
    empty_impl::impl_empty_code,
    super_trait::impl_supertrait,
    Proxy, SyncType,
};

pub fn def_struct_rwlock(proxy: Proxy, trait_def: ItemTrait) -> TokenStream {
    let trait_name = &trait_def.ident;
    let func_vec = trait_def.items.clone();

    let ident = proxy.ident.clone();
    let super_trait_code = impl_supertrait(ident.clone(), trait_def.clone(), SyncType::RWLOCK);

    let (func_code, other) = impl_func(func_vec, &trait_name, &ident, proxy.source.is_some());

    let extern_func_code = other[0].clone();
    let inner_call_code = other[1].clone();

    let macro_ident = Ident::new(&format!("gen_for_{}", trait_name), trait_name.span());
    let impl_ident = Ident::new(&format!("impl_for_{}", trait_name), trait_name.span());

    let (empty_ident, empty_def_code, empty_impl_for_code) =
        impl_empty_code(trait_name, trait_def.clone());
    let ResourceCode {
        resource_field,
        resource_init,
        cast,
        call_once,
        replace_call,
    } = resource_code(&proxy);

    let ident_key = Ident::new(
        &format!("{}_KEY", ident.to_string().to_uppercase()),
        trait_name.span(),
    );

    let prox_ext_impl = impl_prox_ext_trait(&ident, replace_call, trait_name);

    quote::quote!(
        #[macro_export]
        macro_rules! #macro_ident {
            () => {
                define_static_key_false!(#ident_key);
                #[derive(Debug)]
                pub struct #ident{
                    domain: RcuData<Box<dyn #trait_name>>,
                    lock: RwLock<()>,
                    domain_loader: SleepMutex<DomainLoader>,
                    counter: PerCpuCounter,
                    #resource_field
                }
                impl #ident{
                    pub fn new(domain: Box<dyn #trait_name>,domain_loader: DomainLoader)->Self{
                        Self{
                            domain: RcuData::new(Box::new(domain)),
                            lock: RwLock::new(()),
                            domain_loader: SleepMutex::new(domain_loader),
                            counter: PerCpuCounter::new(),
                            #resource_init
                        }
                    }

                    pub fn all_counter(&self) -> usize {
                        self.counter.all()
                    }

                     pub fn domain_loader(&self) -> DomainLoader{
                        self.domain_loader.lock().clone()
                    }
                }

                impl ProxyBuilder for #ident{
                    type T = Box<dyn #trait_name>;
                    fn build(domain: Self::T,domain_loader: DomainLoader)->Self{
                        Self::new(domain,domain_loader)
                    }
                    fn build_empty(domain_loader: DomainLoader)->Self{
                        let domain = Box::new(#empty_ident::new());
                        Self::new(domain,domain_loader)
                    }
                    fn init_by_box(&self, argv: Box<dyn Any+Send+Sync>) -> AlienResult<()>{
                        #cast
                        #call_once
                        Ok(())
                    }
                }

                #super_trait_code


                impl #trait_name for #ident{
                    #(#func_code)*
                }

                impl #ident{
                    #(#inner_call_code)*
                }

                #prox_ext_impl

                #(#extern_func_code)*


                #empty_def_code

            };
        }

        #[macro_export]
        macro_rules! #impl_ident {
            ($name:ident) => {
                impl #trait_name for $name{
                    #(#func_code)*
                }
                impl $name{
                    #(#inner_call_code)*
                }
            }
        }

        #empty_impl_for_code

    )
    .into()
}

fn impl_prox_ext_trait(
    proxy_name: &Ident,
    replace_call: TokenStream,
    trait_name: &Ident,
) -> TokenStream {
    let code = quote!(
        impl #proxy_name{
            pub fn replace(&self,new_domain: Box<dyn #trait_name>,loader:DomainLoader) -> AlienResult<()> {
                // stage1: get the sleep lock and change to updating state
                let mut loader_guard = self.domain_loader.lock();
                k_static_branch_enable!(BLKDOMAINPROXY_KEY);

                // why we need to synchronize_sched here?
                synchronize_sched();

                // stage2: get the write lock and wait for all readers to finish
                let w_lock = self.lock.write();
                // wait if there are readers which are reading the old domain but no read lock
                while self.all_counter() > 0 {
                    println!("Wait for all reader to finish");
                    yield_now();
                }

                let old_id = self.domain_id();

                // stage3: init the new domain before swap
                let new_domain_id = new_domain.domain_id();
                #replace_call

                // stage4: swap the domain and change to normal state
                let old_domain = self.domain.swap(Box::new(new_domain));
                // change to normal state
                k_static_branch_disable!(BLKDOMAINPROXY_KEY);

                // stage5: recycle all resources
                let real_domain = Box::into_inner(old_domain);
                forget(real_domain);
                free_domain_resource(old_id, FreeShared::NotFree(new_domain_id));

                // stage6: release all locks
                *loader_guard = loader;
                drop(w_lock);
                drop(loader_guard);
                Ok(())
            }
        }
    );
    code
}

fn impl_func(
    func_vec: Vec<TraitItem>,
    trait_name: &Ident,
    proxy_name: &Ident,
    has_resource: bool,
) -> (Vec<TokenStream>, Vec<Vec<TokenStream>>) {
    let mut func_codes = vec![];
    let mut extern_func_codes = vec![vec![], vec![]];
    func_vec.iter().for_each(|item| match item {
        TraitItem::Fn(method) => {
            let (func_code, extern_asm_code, inner_call_code) =
                impl_func_code_rwlock(&method, trait_name, proxy_name, has_resource);
            func_codes.push(func_code);
            extern_func_codes[0].push(extern_asm_code);
            extern_func_codes[1].push(inner_call_code);
        }
        _ => {
            panic!("item is not a function");
        }
    });
    (func_codes, extern_func_codes)
}

fn impl_func_code_rwlock(
    func: &TraitItemFn,
    trait_name: &Ident,
    proxy_name: &Ident,
    _has_resource: bool,
) -> (TokenStream, TokenStream, TokenStream) {
    let FuncInfo {
        has_recovery,
        no_check,
        func_name,
        attr,
        sig,
        input_argv,
        output,
        fn_args,
        arg_domain_change,
    } = collect_func_info(func);

    match func_name.to_string().as_str() {
        "init" => {
            if input_argv.len() > 0 {
                assert_eq!(input_argv.len(), 1);
            }
            let token = quote!(
                #(#attr)*
                #sig{
                    self.domain.get().init(#(#input_argv),*)
                }
            );
            (token, quote!(), quote!())
        }
        _ => {
            let (func_inner, trampoline, inner_call) = gen_trampoline_rwlock(
                has_recovery,
                trait_name,
                proxy_name,
                func_name,
                input_argv,
                fn_args,
                arg_domain_change,
                output,
                no_check,
            );

            let token = quote!(
                #(#attr)*
                #sig{
                    #func_inner
                }
            );
            (token, trampoline, inner_call)
        }
    }
}

fn gen_trampoline_rwlock(
    has_recover: bool,
    trait_name: &Ident,
    proxy_name: &Ident,
    func_name: Ident,
    input_argv: Vec<Ident>,
    fn_args: Vec<FnArg>,
    arg_domain_change: Vec<TokenStream>,
    out_put: ReturnType,
    no_check: bool,
) -> (TokenStream, TokenStream, TokenStream) {
    let info = gen_trampoline_info(
        proxy_name,
        &func_name,
        &input_argv,
        &fn_args,
        &arg_domain_change,
        no_check,
    );

    let (asm_code, inner_call_code, __ident_no_lock, __ident_with_lock) = impl_inner_code(
        has_recover,
        proxy_name,
        &func_name,
        &trait_name,
        &fn_args,
        &input_argv,
        out_put,
        &arg_domain_change,
        &info,
    );

    let ident_key = Ident::new(
        &format!("{}_KEY", proxy_name.to_string().to_uppercase()),
        proxy_name.span(),
    );
    let call = quote!(
        if static_branch_likely!(#ident_key) {
            return self.#__ident_with_lock(#(#input_argv),*);
        }
        self.#__ident_no_lock(#(#input_argv),*)
    );
    // println!("{:?}",real_code.to_string());
    (call, asm_code, inner_call_code)
}

fn impl_inner_code(
    has_recover: bool,
    _proxy_name: &Ident,
    func_name: &Ident,
    trait_name: &Ident,
    fn_argv: &Vec<FnArg>,
    input_argv: &Vec<Ident>,
    output: ReturnType,
    arg_domain_change: &Vec<TokenStream>,
    info: &TrampolineInfo,
) -> (TokenStream, TokenStream, Ident, Ident) {
    let __ident = Ident::new(&format!("__{}", func_name), func_name.span());
    let __ident_no_lock = Ident::new(&format!("__{}_no_lock", func_name), func_name.span());
    let __ident_with_lock = Ident::new(&format!("__{}_with_lock", func_name), func_name.span());

    let TrampolineInfo {
        trampoline_ident,
        real_ident,
        error_ident,
        error_ident_ptr,
        get_domain_id,
        call_trampoline_arg,
        check_code,
        trampoline_func_arg,
        call_move_to,
    } = info;

    let (ident_call, asm_code) = if has_recover {
        let ident_call = quote!(
            let r_domain = self.domain.get().as_ref();
            #check_code
            #get_domain_id
            let res = unsafe {
                #trampoline_ident(#call_trampoline_arg)
            };
            res
        );
        let asm_code = quote!(
            #[no_mangle]
            #[naked]
            #[allow(non_snake_case)]
            #[allow(undefined_naked_function_abi)]
            unsafe fn #trampoline_ident(domain:&dyn #trait_name,#trampoline_func_arg) #output{
                core::arch::asm!(
                    "addi sp, sp, -33*8",
                    "sd x1, 1*8(sp)",
                    "sd x2, 2*8(sp)",
                    "sd x3, 3*8(sp)",
                    "sd x4, 4*8(sp)",
                    "sd x5, 5*8(sp)",
                    "sd x6, 6*8(sp)",
                    "sd x7, 7*8(sp)",
                    "sd x8, 8*8(sp)",
                    "sd x9, 9*8(sp)",
                    "sd x10, 10*8(sp)",
                    "sd x11, 11*8(sp)",
                    "sd x12, 12*8(sp)",
                    "sd x13, 13*8(sp)",
                    "sd x14, 14*8(sp)",
                    "sd x15, 15*8(sp)",
                    "sd x16, 16*8(sp)",
                    "sd x17, 17*8(sp)",
                    "sd x18, 18*8(sp)",
                    "sd x19, 19*8(sp)",
                    "sd x20, 20*8(sp)",
                    "sd x21, 21*8(sp)",
                    "sd x22, 22*8(sp)",
                    "sd x23, 23*8(sp)",
                    "sd x24, 24*8(sp)",
                    "sd x25, 25*8(sp)",
                    "sd x26, 26*8(sp)",
                    "sd x27, 27*8(sp)",
                    "sd x28, 28*8(sp)",
                    "sd x29, 29*8(sp)",
                    "sd x30, 30*8(sp)",
                    "sd x31, 31*8(sp)",
                    "call {error_ptr}",
                    "sd a0, 32*8(sp)",
                    "mv a0, sp",
                    "call register_cont",
                    //  recover caller saved registers
                    "ld ra, 1*8(sp)",
                    "ld x5, 5*8(sp)",
                    "ld x6, 6*8(sp)",
                    "ld x7, 7*8(sp)",
                    "ld x10, 10*8(sp)",
                    "ld x11, 11*8(sp)",
                    "ld x12, 12*8(sp)",
                    "ld x13, 13*8(sp)",
                    "ld x14, 14*8(sp)",
                    "ld x15, 15*8(sp)",
                    "ld x16, 16*8(sp)",
                    "ld x17, 17*8(sp)",
                    "ld x28, 28*8(sp)",
                    "ld x29, 29*8(sp)",
                    "ld x30, 30*8(sp)",
                    "ld x31, 31*8(sp)",
                    "addi sp, sp, 33*8",
                    "la gp, {real_func}",
                    "jr gp",
                    error_ptr = sym #error_ident_ptr,
                    real_func = sym #real_ident,
                    options(noreturn)
                )
            }
            #[allow(non_snake_case)]
            fn #real_ident(r_domain:&dyn #trait_name,#trampoline_func_arg) #output{
                #(#arg_domain_change)*
                let res = r_domain.#func_name(#(#input_argv),*).map(|r| {
                    #call_move_to
                    r
                });
                continuation::pop_continuation();
                res
            }
            #[allow(non_snake_case)]
            fn #error_ident() #output{
                Err(AlienError::DOMAINCRASH)
            }
            #[allow(non_snake_case)]
            fn #error_ident_ptr() ->usize{
                #error_ident as usize
            }
        );

        (ident_call, asm_code)
    } else {
        let ident_call = quote!(
            let r_domain = self.domain.get();
            #check_code
            #get_domain_id
            #(#arg_domain_change)*
            let res = r_domain.#func_name(#(#input_argv),*).map(|r| {
                #call_move_to
                r
            });
            res
        );
        (ident_call, quote!())
    };

    let inner_call = quote!(
        fn #__ident(&self, #(#fn_argv),*)#output{
            #ident_call
        }
        fn #__ident_no_lock(&self, #(#fn_argv),*)#output{
            self.counter.inc();
            let res = self.#__ident(#(#input_argv),*);
            self.counter.dec();
            res
        }
        #[cold]
        fn #__ident_with_lock(&self, #(#fn_argv),*)#output{
            let r_lock = self.lock.read();
            let res = self.#__ident(#(#input_argv),*);
            drop(r_lock);
            res
        }
    );

    (asm_code, inner_call, __ident_no_lock, __ident_with_lock)
}
