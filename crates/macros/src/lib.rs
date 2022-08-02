mod common;
mod supv_task;

use paste::paste;
use proc_macro::TokenStream;
use syn::{parse_macro_input, Error};

use self::supv_task::SupvTask;

macro_rules! macro_try_to_tokens {
    ($input:ident as $ty:ident) => {
        parse_macro_input!($input as $ty)
            .try_to_tokens()
            .unwrap_or_else(Error::into_compile_error)
            .into()
    };
}

macro_rules! impl_proc_macro_derive {
    ($Trait:ident $(, attributes($($attribute:ident),*))?) => {
        paste! {
            #[proc_macro_derive($Trait $(, attributes($($attribute),*))?)]
            pub fn [<$Trait:snake:lower>](input: TokenStream) -> TokenStream {
                macro_try_to_tokens!(input as $Trait)
            }
        }
    };
}

impl_proc_macro_derive!(SupvTask, attributes(task));
