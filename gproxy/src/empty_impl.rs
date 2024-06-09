use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{ItemTrait, TraitItem, TraitItemFn, TypeParamBound};

pub fn impl_empty_supertrait(ident: Ident, trait_def: ItemTrait) -> TokenStream {
    let supertraits = trait_def.supertraits.clone();
    let mut code = vec![];
    for supertrait in supertraits {
        match supertrait {
            TypeParamBound::Trait(trait_bound) => {
                let path = trait_bound.path.clone();
                let segments = path.segments;
                for segment in segments {
                    let trait_name = segment.ident.clone();
                    match trait_name.to_string().as_str() {
                        "DeviceBase" => {
                            let device_base = quote!(
                                impl DeviceBase for #ident{
                                    fn handle_irq(&self)->AlienResult<()>{
                                        Err(AlienError::ENOSYS)
                                    }
                                }
                            );
                            code.push(device_base)
                        }
                        "Basic" => {
                            let basic = quote!(
                                impl Basic for #ident{
                                    fn is_active(&self)->bool{
                                        false
                                    }
                                }
                            );
                            code.push(basic)
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    quote::quote!(
        #(#code)*
    )
    .into()
}

pub fn impl_empty_func(func_vec: Vec<TraitItem>) -> Vec<TokenStream> {
    let mut func_codes = vec![];
    func_vec.iter().for_each(|item| match item {
        TraitItem::Fn(method) => {
            let func_code = impl_empty_func_code(&method);
            func_codes.push(func_code);
        }
        _ => {
            panic!("item is not a function");
        }
    });
    func_codes
}

fn impl_empty_func_code(func: &TraitItemFn) -> TokenStream {
    let name = func.sig.ident.clone();
    let mut attr = func.attrs.clone();

    attr.retain(|attr| {
        let path = attr.path();
        !path.is_ident("recover") && !path.is_ident("no_check")
    });
    let mut sig = func.sig.clone();
    sig.inputs.iter_mut().skip(1).for_each(|arg| {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                let pat = pat_type.pat.as_mut();
                match pat {
                    syn::Pat::Ident(ident) => {
                        let name = ident.ident.clone();
                        // name
                        ident.ident = Ident::new(&format!("_{}", name), name.span());
                    }
                    _ => {
                        panic!("not a ident");
                    }
                }
            }
            _ => {
                panic!("not a typed");
            }
        }
    }); // reset the input arguments

    match name.to_string().as_str() {
        "init" => {
            let token = quote!(
                #(#attr)*
                #sig{
                    Ok(())
                }
            );
            token
        }
        _ => {
            let token = quote!(
                #(#attr)*
                #sig{
                    Err(AlienError::ENOSYS)
                }
            );
            token
        }
    }
}
