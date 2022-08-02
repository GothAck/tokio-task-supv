pub use heck::ToSnakeCase;
use proc_macro2::Ident;
use syn::{parse::Parse, Attribute, Result};

pub trait IdentAsSnakeCase {
    fn to_snake_case(&self) -> Ident;
}

pub trait Mergeable: Sized {
    fn merge(&mut self, other: Self);

    fn merge_with(mut self, other: Self) -> Self {
        self.merge(other);
        self
    }
}

pub trait FromAttrs: Default + Mergeable + Parse {
    const ATTR_IDENT: &'static str;

    fn from_attrs(attrs: &[Attribute]) -> Result<Option<Self>> {
        attrs
            .iter()
            .filter(|a| a.path.is_ident(Self::ATTR_IDENT))
            .map(|a| {
                if a.tokens.is_empty() {
                    Ok(Self::default())
                } else {
                    a.parse_args_with(Self::parse)
                }
            })
            .reduce(Mergeable::merge_with)
            .transpose()
    }
}

mod impls {
    use super::*;

    impl IdentAsSnakeCase for Ident {
        fn to_snake_case(&self) -> Ident {
            Ident::new(&self.to_string().to_snake_case(), self.span())
        }
    }

    impl<T> Mergeable for Option<T> {
        fn merge(&mut self, other: Self) {
            if let Some(other) = other {
                self.replace(other);
            }
        }
    }

    impl<T: Mergeable> Mergeable for Result<T> {
        fn merge(&mut self, other: Self) {
            match (self, other) {
                (Err(this), Err(other)) => {
                    this.combine(other);
                }
                (Err(..), Ok(..)) => {}
                (this @ Ok(..), res @ Err(..)) => {
                    *this = res;
                }
                (Ok(this), Ok(other)) => {
                    this.merge(other);
                }
            }
        }
    }
}
