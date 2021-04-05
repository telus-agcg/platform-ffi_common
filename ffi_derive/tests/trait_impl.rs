mod animals {
    pub mod cats {
        #[derive(Debug, Clone, ffi_derive::FFI)]
        pub struct Cat {
            pub color: String,
            pub age: u8,
        }

        #[derive(Debug, Clone, PartialEq, ffi_derive::FFI)]
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

#[ffi_derive::expose_items(animals::cats::meow_ffi::FFIArrayMeow)]
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

#[test]
fn test_meow_ffi() {
    use std::boxed::Box;

    let cat = Cat {
        color: "black".to_string(),
        age: 2,
    };
    let cat_ptr = Box::into_raw(Box::new(cat));
    let volume = Volume { value: 100.0 };
    let volume_ptr = Box::into_raw(Box::new(volume));
    let ffi_meows = unsafe { meows_cat_ffi::meow(cat_ptr, volume_ptr, 3) };
    let rust_meows: Vec<Meow> = ffi_meows.into();
    let expected = vec![
        Meow { demands_food: true },
        Meow { demands_food: true },
        Meow { demands_food: true },
    ];
    assert_eq!(rust_meows, expected);
}
