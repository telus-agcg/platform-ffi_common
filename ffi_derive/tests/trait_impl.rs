
mod animals {
    pub mod cats {
        #[derive(Debug, Clone, ffi_derive::FFI)]
        pub struct Cat {
            pub color: String,
            pub age: u8,
        }

        #[derive(Debug, Clone, ffi_derive::FFI)]
        pub struct Meow {
            pub demands_food: bool,
        }
    }
}

mod utilities {
    pub mod sound {
        #[derive(Debug, Clone, ffi_derive::FFI)]
        pub struct Volume { 
            pub value: f64,
        }
    }
}

use animals::cats::{Cat, Meow};
use utilities::sound::Volume;
trait Meows { 
    fn meow(&self, volume: Option<Volume>, count: u8) -> Vec<Meow>;
}

#[ffi_derive::expose_items("utilities::sound::volume_ffi", "animals::cats::cat_ffi", "animals::cats::meow_ffi")]
impl Meows for Cat {
    fn meow(&self, volume: Option<Volume>, count: u8) -> Vec<Meow> {
        let demands_food = if let Some(volume) = volume {
            volume.value >= 50.0
        } else {
            false
        };
        vec![Meow { demands_food }; count.into()]
    }
}

// TODO: Write a test
// TODO: Update the docs in lib.rs with the expanded sig
