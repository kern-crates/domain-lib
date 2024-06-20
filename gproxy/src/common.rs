use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::{Attribute, FnArg, ReturnType, Signature, TraitItemFn};

use crate::Proxy;

pub struct ResourceCode {
    pub resource_field: TokenStream,
    pub resource_init: TokenStream,
    pub cast: TokenStream,
    pub call_once: TokenStream,
}

pub fn resource_code(proxy: &Proxy) -> ResourceCode {
    let resource_field = if proxy.source.is_some() {
        quote! (
            resource: Once<Box<dyn Any+Send+Sync>>
        )
    } else {
        quote!()
    };
    let (resource_init, cast, call_once) = if proxy.source.is_some() {
        let s1 = quote! (
            resource: Once::new()
        );
        let s_ty = proxy.source.as_ref().unwrap();
        let s2 = quote! (
            let arg = argv.as_ref().downcast_ref::<#s_ty>().unwrap();
            self.init(arg)?;
        );
        let s3 = quote! (
            self.resource.call_once(|| argv);
        );
        (s1, s2, s3)
    } else {
        let s2 = quote!(
            let _ = argv;
            self.init()?;
        );
        (quote!(), s2, quote!())
    };
    ResourceCode {
        resource_field,
        resource_init,
        cast,
        call_once,
    }
}

pub struct FuncInfo {
    pub has_recovery: bool,
    pub no_check: bool,
    pub func_name: Ident,
    pub attr: Vec<Attribute>,
    pub sig: Signature,
    pub input_argv: Vec<Ident>,
    pub output: ReturnType,
    pub fn_args: Vec<FnArg>,
    pub arg_domain_change: Vec<TokenStream>,
}

pub fn collect_func_info(func: &TraitItemFn) -> FuncInfo {
    let has_recover = func
        .attrs
        .iter()
        .find(|attr| {
            let path = attr.path();
            path.is_ident("recoverable")
        })
        .is_some();

    let no_check = func
        .attrs
        .iter()
        .find(|attr| {
            let path = attr.path();
            path.is_ident("no_check")
        })
        .is_some();

    let name = func.sig.ident.clone();
    let mut attr = func.attrs.clone();

    attr.retain(|attr| {
        let path = attr.path();
        !path.is_ident("recoverable") && !path.is_ident("no_check")
    });

    let sig = func.sig.clone();
    let input = sig.inputs.clone();
    let out_put = sig.output.clone();
    let mut fn_args = vec![];

    let mut arg_domain_change = vec![];

    let input_argv = input
        .iter()
        .skip(1)
        .map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => {
                let ty = pat_type.ty.as_ref().to_token_stream().to_string();
                let pat = pat_type.pat.as_ref();
                match pat {
                    syn::Pat::Ident(ident) => {
                        fn_args.push(arg.clone());
                        let name = ident.ident.clone();
                        if ty.starts_with("RRef") {
                            let change_code = quote! (
                                let old_id = #name.move_to(id);
                            );
                            arg_domain_change.push(change_code);
                        }
                        name
                    }
                    _ => {
                        panic!("not a ident");
                    }
                }
            }
            _ => {
                panic!("not a typed");
            }
        })
        .collect::<Vec<Ident>>();
    FuncInfo {
        has_recovery: has_recover,
        no_check,
        func_name: name,
        attr,
        sig,
        input_argv,
        output: out_put,
        fn_args,
        arg_domain_change,
    }
}

pub struct TrampolineInfo {
    pub trampoline_ident: Ident,
    pub real_ident: Ident,
    pub error_ident: Ident,
    pub error_ident_ptr: Ident,
    pub get_domain_id: TokenStream,
    pub call_trampoline_arg: TokenStream,
    pub check_code: TokenStream,
    pub trampoline_func_arg: TokenStream,
    pub call_move_to: TokenStream,
}

pub fn gen_trampoline_info(
    proxy_name: &Ident,
    func_name: &Ident,
    input_argv: &Vec<Ident>,
    fn_args: &Vec<FnArg>,
    arg_domain_change: &Vec<TokenStream>,
    no_check: bool,
) -> TrampolineInfo {
    let trampoline_ident = Ident::new(
        &format!("{}_{}_trampoline", proxy_name, func_name),
        func_name.span(),
    );
    let real_ident = Ident::new(&format!("{}_{}", proxy_name, func_name), func_name.span());
    let error_ident = Ident::new(&format!("{}_error", real_ident), func_name.span());
    let error_ident_ptr = Ident::new(&format!("{}_error_ptr", real_ident), func_name.span());

    let (get_domain_id, call_trampoline_arg) = if arg_domain_change.is_empty() {
        let x2 = quote!(
            r_domain,#(#input_argv),*
        );
        (quote!(), x2)
    } else {
        let x1 = quote!(
            let id = r_domain.domain_id();
        );
        let x2 = quote!(
            r_domain,id,#(#input_argv),*
        );
        (x1, x2)
    };

    let check_code = if no_check {
        quote!()
    } else {
        quote!(if !r_domain.is_active() {
            return Err(AlienError::DOMAINCRASH);
        })
    };

    let (trampoline_func_arg, call_move_to) = if arg_domain_change.is_empty() {
        let x1 = quote!(#(#fn_args),*);
        let x2 = quote!();
        (x1, x2)
    } else {
        let x1 = quote!(id:u64,#(#fn_args),*);
        let x2 = quote!(
            r.move_to(old_id);
        );
        (x1, x2)
    };

    TrampolineInfo {
        trampoline_ident,
        real_ident,
        error_ident,
        error_ident_ptr,
        get_domain_id,
        call_trampoline_arg,
        check_code,
        trampoline_func_arg,
        call_move_to,
    }
}
