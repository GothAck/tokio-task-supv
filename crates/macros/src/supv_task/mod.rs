mod attrs;

use indexmap::IndexMap;
use proc_macro2::Ident;
use syn::ItemStruct;

use self::attrs::*;

pub struct SupvTask {
    item: ItemStruct,
    attr: SupvTaskAttr,

    field_attrs: IndexMap<Ident, FieldAttr>,
}

mod impls {
    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::{
        parse::{Parse, ParseStream},
        spanned::Spanned,
        Error, Fields, FieldsNamed, Result,
    };

    use crate::common::FromAttrs;

    use super::*;

    impl SupvTask {
        pub fn try_to_tokens(&self) -> Result<TokenStream> {
            let ident = &self.item.ident;
            let name = self.attr.name(ident);
            let arg = self.attr.arg();
            let output = self.attr.output();
            let spawns = self.attr.spawns();

            let field_spawns = self
                .field_attrs
                .iter()
                .filter_map(|(ident, attr)| attr.spawn(ident))
                .collect::<TokenStream>();

            let spawn_call = self.spawn_call();

            let spawn_name_extra = self.attr.spawn_name_extra().map(|expr| {
                quote! {
                    fn spawn_name_extra(&self) -> Option<String> {
                        #expr
                    }
                }
            });

            let (impl_gen, ty_gen, where_cl) = self.item.generics.split_for_impl();

            Ok(quote! {
                #[::async_trait::async_trait]
                impl #impl_gen SupvTask for #ident #ty_gen #where_cl {
                    const NAME: &'static str = #name;
                    type Arg = #arg;
                    type Output = ::anyhow::Result<#output>;

                    async fn spawn(
                        self: &::std::sync::Arc<Self>,
                        arg: Self::Arg,
                        supv_ref: ::tokio_task_supv::supv::TaskSupvRef,
                        span: ::tracing::Span,
                    ) -> ::anyhow::Result<::tokio::task::JoinHandle<Self::Output>> {
                        use ::tracing::Instrument;
                        #spawns
                        #field_spawns
                        Ok(#spawn_call)
                    }

                    #spawn_name_extra
                }
            })
        }

        fn spawn_call(&self) -> TokenStream {
            let run_args = self.attr.run_args();
            let awaitable = if self.attr.dummy_run() {
                quote!(::tokio_task_supv::dummy::run(supv_ref))
            } else {
                quote!(self.clone().run(#run_args))
            };
            quote!(::tokio::spawn(#awaitable.instrument(span)))
        }
    }

    impl Parse for SupvTask {
        fn parse(input: ParseStream) -> Result<Self> {
            let item: ItemStruct = input.parse()?;
            let attr = SupvTaskAttr::from_attrs(&item.attrs)?.unwrap_or_default();

            let field_attrs = if let Fields::Named(FieldsNamed { named, .. }) = &item.fields {
                named
                    .iter()
                    .filter_map(|f| {
                        Some(
                            FieldAttr::from_attrs(&f.attrs)
                                .transpose()?
                                .map(|a| (f.ident.clone().unwrap(), a)),
                        )
                    })
                    .collect::<Result<_>>()?
            } else {
                return Err(Error::new(
                    item.fields.span(),
                    "Only support named field struct",
                ));
            };

            Ok(Self {
                item,
                attr,
                field_attrs,
            })
        }
    }
}
