use proc_macro2::Ident;
use syn::{punctuated::Punctuated, Expr, LitStr, Token, Type, TypeTuple};

#[derive(Default)]
pub struct SupvTaskAttr {
    name: Option<LitStr>,
    arg: Option<TypeTuple>,
    output: Option<Type>,
    run_args: Option<Punctuated<Expr, Token![,]>>,
    dummy_run: Option<()>,
    spawn: Vec<Expr>,
    spawn_name_extra: Option<Expr>,
}

#[derive(Default)]
pub struct FieldAttr {
    spawn: Option<AttrSpawn>,
}

enum AttrSpawn {
    Task(AttrSpawnTask),
    Let(AttrSpawnLet),
}

struct AttrSpawnTask {
    args: Box<Expr>,
}

struct AttrSpawnLet {
    ident: Ident,
    token_eq: Token![=],
    expr: Box<Expr>,
}

mod impls {
    use heck::ToSnakeCase;
    use proc_macro2::TokenStream;
    use quote::{quote, ToTokens};
    use syn::{
        parenthesized,
        parse::{Parse, ParseStream},
        Error, Result, ext::IdentExt,
    };

    use crate::common::{FromAttrs, Mergeable};

    use super::*;

    impl SupvTaskAttr {
        pub fn name(&self, ident: &Ident) -> LitStr {
            self.name
                .clone()
                .unwrap_or_else(|| LitStr::new(&ident.to_string().to_snake_case(), ident.span()))
        }

        pub fn arg(&self) -> TokenStream {
            self.arg
                .as_ref()
                .map_or_else(|| quote!(()), ToTokens::to_token_stream)
        }

        pub fn output(&self) -> TokenStream {
            self.output
                .as_ref()
                .map_or_else(|| quote!(()), ToTokens::to_token_stream)
        }

        pub fn run_args(&self) -> TokenStream {
            self.run_args
                .as_ref()
                .map_or_else(|| quote!((supv_ref)), ToTokens::to_token_stream)
        }

        pub fn dummy_run(&self) -> bool {
            self.dummy_run.is_some()
        }

        pub fn spawns(&self) -> TokenStream {
            let spawn = &self.spawn;
            quote!( #(#spawn;)* )
        }

        pub fn spawn_name_extra(&self) -> Option<TokenStream> {
            self.spawn_name_extra
                .as_ref()
                .map(ToTokens::to_token_stream)
        }
    }

    impl FromAttrs for SupvTaskAttr {
        const ATTR_IDENT: &'static str = "task";
    }

    impl Mergeable for SupvTaskAttr {
        fn merge(&mut self, other: Self) {
            self.name.merge(other.name);
            self.arg.merge(other.arg);
            self.output.merge(other.output);
            self.run_args.merge(other.run_args);
            self.dummy_run.merge(other.dummy_run);
            self.spawn.extend(other.spawn);
            self.spawn_name_extra.merge(other.spawn_name_extra);
        }
    }

    impl Parse for SupvTaskAttr {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut this = Self::default();

            while !input.is_empty() {
                let ident: Ident = input.parse()?;
                let content;
                match ident.to_string().as_str() {
                    "name" => {
                        parenthesized!(content in input);
                        this.name = Some(content.parse()?);
                    }
                    "arg" => {
                        parenthesized!(content in input);
                        this.arg = Some(content.parse()?);
                    }
                    "output" => {
                        parenthesized!(content in input);
                        this.output = Some(content.parse()?);
                    }
                    "run_args" => {
                        parenthesized!(content in input);
                        this.run_args = Some(content.parse_terminated(Expr::parse)?);
                    }
                    "dummy_run" => {
                        this.dummy_run = Some(());
                    }
                    "spawn" => {
                        parenthesized!(content in input);
                        this.spawn.push(content.parse()?);
                    }
                    "spawn_name_extra" => {
                        parenthesized!(content in input);
                        this.spawn_name_extra = Some(content.parse()?);
                    }
                    _ => return Err(Error::new(ident.span(), "Unknown attr")),
                }
                if !input.is_empty() {
                    input.parse::<Token![,]>()?;
                }
            }
            Ok(this)
        }
    }

    impl FieldAttr {
        pub fn spawn(&self, ident: &Ident) -> Option<TokenStream> {
            self.spawn.as_ref().map(|spawn| spawn.to_token_stream(ident))
        }
    }

    impl FromAttrs for FieldAttr {
        const ATTR_IDENT: &'static str = "task";
    }

    impl Mergeable for FieldAttr {
        fn merge(&mut self, other: Self) {
            self.spawn.merge(other.spawn);
        }
    }

    impl Parse for FieldAttr {
        fn parse(input: ParseStream) -> Result<Self> {
            let mut this = Self::default();

            while !input.is_empty() {
                let ident: Ident = input.parse()?;
                let content;
                match ident.to_string().as_str() {
                    "spawn" => {
                        parenthesized!(content in input);
                        this.spawn = Some(content.parse()?);
                    }
                    _ => return Err(Error::new(ident.span(), "Unknown attr")),
                }
                if !input.is_empty() {
                    input.parse::<Token![,]>()?;
                }
            }

            Ok(this)
        }
    }

    impl AttrSpawn {
        fn to_token_stream(&self, ident: &Ident) -> TokenStream {
            match self {
                Self::Task(task) => task.to_token_stream(ident),
                Self::Let(spawn_let) => spawn_let.to_token_stream(),
            }
        }
    }

    impl Parse for AttrSpawn {
        fn parse(input: ParseStream) -> Result<Self> {
            let ident = input.call(Ident::parse_any)?;

            match ident.to_string().as_str() {
                "task" => {
                    let content;
                    parenthesized!(content in input);
                    content.parse().map(Self::Task)
                }
                "let" => {
                    // let content;
                    // parenthesized!(content in input);
                    input.parse().map(Self::Let)
                }
                _ => Err(Error::new(ident.span(), "Unknown attr")),
            }
        }
    }

    impl AttrSpawnTask {
        fn to_token_stream(&self, ident: &Ident) -> TokenStream {
            let Self { args } = self;

            quote! {
                supv_ref.spawn(&self.#ident, #args).await?;
            }
        }
    }

    impl Parse for AttrSpawnTask {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(Self {
                args: input.parse()?,
            })
        }
    }

    impl AttrSpawnLet {
        fn to_token_stream(&self) -> TokenStream {
            let Self { ident, token_eq, expr } = self;

            quote! {
                let #ident #token_eq #expr;
            }
        }
    }

    impl Parse for AttrSpawnLet {
        fn parse(input: ParseStream) -> Result<Self> {
            Ok(Self {
                ident: input.parse()?,
                token_eq: input.parse()?,
                expr: input.parse()?,
            })
        }
    }
}
