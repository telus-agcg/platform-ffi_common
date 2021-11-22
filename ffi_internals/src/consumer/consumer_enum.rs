//!
//! Generates boilerplate code for using a repr(C) enum in the consumer's language.
//!

use syn::Ident;

mod complex_enum;
mod repr_c_enum;

pub use complex_enum::ComplexConsumerEnum;
pub use repr_c_enum::ReprCConsumerEnum;

trait ConsumerEnumType {
    fn type_name_ident(&self) -> &Ident;
}

trait CommonConsumerNames {
    fn array_name(&self) -> String;
    fn array_init_fn_name(&self) -> String;
    fn array_free_fn_name(&self) -> String;
    fn option_init_fn_name(&self) -> String;
    fn option_free_fn_name(&self) -> String;
}

impl<T: ConsumerEnumType> CommonConsumerNames for T {
    fn array_name(&self) -> String {
        format!("FFIArray{}", self.type_name_ident())
    }

    fn array_init_fn_name(&self) -> String {
        format!("ffi_array_{}_init", self.type_name_ident())
    }

    fn array_free_fn_name(&self) -> String {
        format!("ffi_array_{}_free", self.type_name_ident())
    }

    fn option_init_fn_name(&self) -> String {
        format!("option_{}_init", self.type_name_ident())
    }

    fn option_free_fn_name(&self) -> String {
        format!("option_{}_free", self.type_name_ident())
    }
}
