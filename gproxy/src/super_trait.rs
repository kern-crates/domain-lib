use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{ItemTrait, TypeParamBound};

use crate::SyncType;

pub fn impl_supertrait(ident: Ident, trait_def: ItemTrait, sync_ty: SyncType) -> TokenStream {
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
                            let (ext_code, inner_code) = match sync_ty {
                                SyncType::SRCU => (quote!(), impl_srcu_code()),
                                SyncType::RWLOCK => {
                                    (impl_lock_code(&ident), impl_rwlock_code(&ident))
                                }
                            };

                            let device_base = quote!(
                                #ext_code
                                impl DeviceBase for #ident{
                                    fn handle_irq(&self)->AlienResult<()>{
                                        #inner_code
                                    }
                                }
                            );
                            code.push(device_base)
                        }
                        "Basic" => {
                            let basic = quote!(
                                impl Basic for #ident{
                                    fn domain_id(&self)->u64{
                                        self.domain.get().domain_id()
                                    }
                                    fn is_active(&self)->bool{
                                        self.domain.get().is_active()
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

fn impl_srcu_code() -> TokenStream {
    quote!(
        let idx = self.srcu_lock.read_lock();
        let domain = self.domain.get();
        if !domain.is_active() {
            return Err(AlienError::DOMAINCRASH);
        }
        let res = domain.handle_irq();
        self.srcu_lock.read_unlock(idx);
        res
    )
}

fn impl_rwlock_code(ident: &Ident) -> TokenStream {
    let upper_ident = Ident::new(
        &format!("{}_KEY", ident.to_string().to_uppercase()),
        ident.span(),
    );
    quote!(
        if static_branch_likely!(#upper_ident) {
            return self.__handle_irq_with_lock();
        }
        self.__handle_irq_no_lock()
    )
}
fn impl_lock_code(ident: &Ident) -> TokenStream {
    quote!(
        impl #ident{
            fn __handle_irq(&self) -> AlienResult<()> {
                let domain = self.domain.get();
                if !domain.is_active() {
                    return Err(AlienError::DOMAINCRASH);
                }
                domain.handle_irq()
            }
            fn __handle_irq_no_lock(&self) -> AlienResult<()> {
                self.counter.inc();
                let res = self.__handle_irq();
                self.counter.dec();
                res
            }
            #[cold]
            fn __handle_irq_with_lock(&self) -> AlienResult<()> {
                let r_lock = self.lock.read();
                let res = self.__handle_irq();
                drop(r_lock);
                res
            }
        }
    )
}
