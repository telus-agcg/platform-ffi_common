An enum and struct marked with `ffi_derive::FFI`

```rust
#[repr(C)]
#[no_mangle]
#[derive(Clone, Debug, Copy, ffi_derive::FFI)]
pub enum NativeEnum {
    V1,
    V2,
}

impl Default for NativeEnum {
    fn default() -> Self {
        Self::V1
    }
}

use native_enum_ffi::FFIArrayNativeEnum;

#[derive(Clone, Debug, ffi_derive::FFI)]
pub struct NativeStruct {
    pub an_id: Uuid,
    pub a_string: String,
    pub an_f32: f32,
    pub a_datetime: NaiveDateTime,
    pub collection_of_ids: Vec<Uuid>,
    #[ffi(raw)]
    pub collection_of_enum_variants: Vec<NativeEnum>,
}
```

will expand to

```rust
#[repr(C)]
#[no_mangle]
pub enum NativeEnum {
    V1,
    V2,
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::clone::Clone for NativeEnum {
    #[inline]
    fn clone(&self) -> NativeEnum {
        {
            *self
        }
    }
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::fmt::Debug for NativeEnum {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        match (&*self,) {
            (&NativeEnum::V1,) => {
                let mut debug_trait_builder = f.debug_tuple("V1");
                debug_trait_builder.finish()
            }
            (&NativeEnum::V2,) => {
                let mut debug_trait_builder = f.debug_tuple("V2");
                debug_trait_builder.finish()
            }
        }
    }
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::marker::Copy for NativeEnum {}
#[allow(missing_docs)]
pub mod native_enum_ffi {
    use super::*;
    use ffi_common::declare_value_type_array_struct;
    use paste::paste;
    use ffi_common::error;
    #[no_mangle]
    pub unsafe extern "C" fn free_native_enum(data: NativeEnum) {
        ffi_common::error::clear_last_err_msg();
        let _ = data;
    }
    ///
    ///An FFI-safe representation of a collection of FFI-safe data structures.
    ///
    ///This can also express an `Option<Vec<_>>` with a null pointer and a len and capacity of 0. FFI
    ///consumers should therefore make sure that the pointer is not null (although our generated code
    ///should be able to preserve optionality across the FFI boundary, so it will only have to check in
    ///places where null is really possible.)
    ///
    ///# Safety
    ///
    ///The collection represented by this type needs to be reclaimed by Rust with `Vec::from_raw_parts` so
    ///it can be deallocated safely. Pass this struct to `free_ffi_array_*` when you're done with it (i.e.,
    ///when you've copied it into native memory, displayed it, whatever you're doing on the other side of
    ///the FFI boundary) so we can take care of those steps.
    ///            
    #[repr(C)]
    pub struct FFIArrayNativeEnum {
        ///Pointer to the first element in the array.
        pub ptr: *const NativeEnum,
        ///The length of (i.e. the number of elements in) this array.
        pub len: usize,
        ///The capacity with which this array was allocated.
        pub cap: usize,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for FFIArrayNativeEnum {
        #[inline]
        fn clone(&self) -> FFIArrayNativeEnum {
            match *self {
                FFIArrayNativeEnum {
                    ptr: ref __self_0_0,
                    len: ref __self_0_1,
                    cap: ref __self_0_2,
                } => FFIArrayNativeEnum {
                    ptr: ::core::clone::Clone::clone(&(*__self_0_0)),
                    len: ::core::clone::Clone::clone(&(*__self_0_1)),
                    cap: ::core::clone::Clone::clone(&(*__self_0_2)),
                },
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for FFIArrayNativeEnum {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                FFIArrayNativeEnum {
                    ptr: ref __self_0_0,
                    len: ref __self_0_1,
                    cap: ref __self_0_2,
                } => {
                    let mut debug_trait_builder = f.debug_struct("FFIArrayNativeEnum");
                    let _ = debug_trait_builder.field("ptr", &&(*__self_0_0));
                    let _ = debug_trait_builder.field("len", &&(*__self_0_1));
                    let _ = debug_trait_builder.field("cap", &&(*__self_0_2));
                    debug_trait_builder.finish()
                }
            }
        }
    }
    #[no_mangle]
    ///Initialize an `FFIArray*` from across the FFI boundary. This will copy the provided data into Rust
    ///memory.
    ///
    ///# Safety
    ///
    ///The pointer you send must point to the first element of an array whose elements match the type of
    ///`FFIArray*`.
    ///
    ///If `ptr` is a null pointer, this will create an array wrapper with a length and capacity of `0`,
    ///and a null pointer; this expresses the `None` variant of an `Option<Vec<T>>`.
    ///**Important: do not pass a null pointer if the field that this array will be used with is not an
    ///`Option`.**
    ///
    ///This is the only way to safely construct an `FFIArray*` from the non-Rust side of the FFI boundary.
    ///We assume that all instances of `FFIArray*` are allocated by Rust, as this allows us to greatly
    ///simplify memory management.
    pub unsafe extern "C" fn ffi_array_native_enum_init(
        ptr: *const NativeEnum,
        len: isize,
    ) -> FFIArrayNativeEnum {
        let mut v = ::alloc::vec::Vec::new();
        for i in 0..len {
            let e = *ptr.offset(i);
            v.push(e);
        }
        (&v).into()
    }
    impl From<&Vec<NativeEnum>> for FFIArrayNativeEnum {
        fn from(vec: &Vec<NativeEnum>) -> Self {
            let v: std::mem::ManuallyDrop<Vec<NativeEnum>> =
                std::mem::ManuallyDrop::new(vec.clone());
            let len = v.len();
            let ptr = v.as_ptr();
            let cap = v.capacity();
            Self { ptr, len, cap }
        }
    }
    impl From<&Option<Vec<NativeEnum>>> for FFIArrayNativeEnum {
        fn from(opt: &Option<Vec<NativeEnum>>) -> Self {
            opt.as_ref().map_or(
                Self {
                    ptr: std::ptr::null(),
                    len: 0,
                    cap: 0,
                },
                |v| v.into(),
            )
        }
    }
    #[allow(clippy::use_self)]
    impl From<FFIArrayNativeEnum> for Vec<NativeEnum> {
        fn from(array: FFIArrayNativeEnum) -> Self {
            unsafe {
                let v =
                    Vec::from_raw_parts(array.ptr as *mut NativeEnum, array.len, array.cap);
                v
            }
        }
    }
    impl From<FFIArrayNativeEnum> for Option<Vec<NativeEnum>> {
        fn from(array: FFIArrayNativeEnum) -> Self {
            if array.ptr.is_null() {
                None
            } else {
                unsafe {
                    let v = Vec::from_raw_parts(
                        array.ptr as *mut NativeEnum,
                        array.len,
                        array.cap,
                    );
                    Some(v)
                }
            }
        }
    }
    impl Drop for FFIArrayNativeEnum {
        fn drop(&mut self) {
            {
                ::std::io::_print(::core::fmt::Arguments::new_v1(
                    &["> Dropping value type array: ", "\n"],
                    &match (&self,) {
                        (arg0,) => {
                            [::core::fmt::ArgumentV1::new(arg0, ::core::fmt::Debug::fmt)]
                        }
                    },
                ));
            };
        }
    }
    ///
    ///Pass an FFI array to this method to allow Rust to reclaim ownership of the object so that it can be safely deallocated.
    ///
    ///# Safety
    ///
    ///We're assuming that the memory in the `array` you give us was allocated by Rust. Don't call this with an object created on the other side of the FFI boundary; that is undefined behavior.
    ///
    ///You **must not** access `array` after passing it to this method.
    ///
    ///Null pointers will be a no-op.
    ///            
    #[no_mangle]
    pub extern "C" fn free_ffi_array_native_enum(array: FFIArrayNativeEnum) {
        error::clear_last_err_msg();
        if array.ptr.is_null() {
            return;
        }
        unsafe {
            let _ = Vec::from_raw_parts(array.ptr as *mut NativeEnum, array.len, array.cap);
        }
    }
    ///An FFI-safe representation of an optional type.
    #[repr(C)]
    pub struct OptionNativeEnum {
        ///True if there's a valid `value`, otherwise false. In the case of false, `value` must be ignored.
        pub has_value: bool,
        ///The wrapped value when `has_value` is `true`. This should be considered garbage (i.e., not read) by callers when `has_value` is false.
        pub value: NativeEnum,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for OptionNativeEnum {
        #[inline]
        fn clone(&self) -> OptionNativeEnum {
            {
                let _: ::core::clone::AssertParamIsClone<bool>;
                let _: ::core::clone::AssertParamIsClone<NativeEnum>;
                *self
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::marker::Copy for OptionNativeEnum {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for OptionNativeEnum {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                OptionNativeEnum {
                    has_value: ref __self_0_0,
                    value: ref __self_0_1,
                } => {
                    let mut debug_trait_builder = f.debug_struct("OptionNativeEnum");
                    let _ = debug_trait_builder.field("has_value", &&(*__self_0_0));
                    let _ = debug_trait_builder.field("value", &&(*__self_0_1));
                    debug_trait_builder.finish()
                }
            }
        }
    }
    impl From<&Option<NativeEnum>> for OptionNativeEnum {
        fn from(opt: &Option<NativeEnum>) -> Self {
            match opt {
                Some(s) => Self {
                    has_value: true,
                    value: s.clone(),
                },
                None => Self {
                    has_value: false,
                    value: NativeEnum::default(),
                },
            }
        }
    }
    impl From<OptionNativeEnum> for Option<NativeEnum> {
        fn from(opt: OptionNativeEnum) -> Self {
            if opt.has_value {
                Some(opt.value)
            } else {
                None
            }
        }
    }
}
impl Default for NativeEnum {
    fn default() -> Self {
        Self::V1
    }
}
use native_enum_ffi::FFIArrayNativeEnum;
pub struct NativeStruct {
    pub an_id: Uuid,
    pub a_string: String,
    pub an_f32: f32,
    pub a_datetime: NaiveDateTime,
    pub collection_of_ids: Vec<Uuid>,
    #[ffi(raw)]
    pub collection_of_enum_variants: Vec<NativeEnum>,
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::clone::Clone for NativeStruct {
    #[inline]
    fn clone(&self) -> NativeStruct {
        match *self {
            NativeStruct {
                an_id: ref __self_0_0,
                a_string: ref __self_0_1,
                an_f32: ref __self_0_2,
                a_datetime: ref __self_0_3,
                collection_of_ids: ref __self_0_4,
                collection_of_enum_variants: ref __self_0_5,
            } => NativeStruct {
                an_id: ::core::clone::Clone::clone(&(*__self_0_0)),
                a_string: ::core::clone::Clone::clone(&(*__self_0_1)),
                an_f32: ::core::clone::Clone::clone(&(*__self_0_2)),
                a_datetime: ::core::clone::Clone::clone(&(*__self_0_3)),
                collection_of_ids: ::core::clone::Clone::clone(&(*__self_0_4)),
                collection_of_enum_variants: ::core::clone::Clone::clone(&(*__self_0_5)),
            },
        }
    }
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::fmt::Debug for NativeStruct {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        match *self {
            NativeStruct {
                an_id: ref __self_0_0,
                a_string: ref __self_0_1,
                an_f32: ref __self_0_2,
                a_datetime: ref __self_0_3,
                collection_of_ids: ref __self_0_4,
                collection_of_enum_variants: ref __self_0_5,
            } => {
                let mut debug_trait_builder = f.debug_struct("NativeStruct");
                let _ = debug_trait_builder.field("an_id", &&(*__self_0_0));
                let _ = debug_trait_builder.field("a_string", &&(*__self_0_1));
                let _ = debug_trait_builder.field("an_f32", &&(*__self_0_2));
                let _ = debug_trait_builder.field("a_datetime", &&(*__self_0_3));
                let _ = debug_trait_builder.field("collection_of_ids", &&(*__self_0_4));
                let _ = debug_trait_builder
                    .field("collection_of_enum_variants", &&(*__self_0_5));
                debug_trait_builder.finish()
            }
        }
    }
}
#[allow(box_pointers)]
#[allow(missing_docs)]
pub mod native_struct_ffi {
    use ffi_common::{*, string::FFIArrayString, datetime::*};
    use std::os::raw::c_char;
    use std::{
        ffi::{CStr, CString},
        mem::ManuallyDrop,
        ptr,
    };
    use paste::paste;
    use uuid::Uuid;
    use super::*;
    #[no_mangle]
    pub unsafe extern "C" fn native_struct_free(data: *const NativeStruct) {
        ffi_common::error::clear_last_err_msg();
        let _ = Box::from_raw(data as *mut NativeStruct);
    }
    ///
    ///An FFI-safe representation of a collection of opaque data structures.
    ///
    ///This can also express an `Option<Vec<_>>` with a null pointer and a len and capacity of 0. FFI
    ///consumers should therefore make sure that the pointer is not null (although our generated code
    ///should be able to preserve optionality across the FFI boundary, so it will only have to check in
    ///places where null is really possible.)
    ///
    ///# Safety
    ///
    ///This will need to be brought back into rust ownership in two ways; first, the vec needs to
    ///be reclaimed with `Vec::from_raw_parts`; second, each element of the vec will need
    ///to be reclaimed with `Box::from_raw`. Pass this struct to `free_ffi_array_*` when you're done with
    ///it (i.e., when you've copied it into native memory, displayed it, whatever you're doing on the other
    ///side of the FFI boundary) so we can take care of those steps.
    ///            
    #[repr(C)]
    pub struct FFIArrayNativeStruct {
        ///Pointer to the first element in the array.
        ptr: *const *const NativeStruct,
        ///The length of (i.e. the number of elements in) this array.
        len: usize,
        ///The capacity with which this array was allocated.
        cap: usize,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for FFIArrayNativeStruct {
        #[inline]
        fn clone(&self) -> FFIArrayNativeStruct {
            match *self {
                FFIArrayNativeStruct {
                    ptr: ref __self_0_0,
                    len: ref __self_0_1,
                    cap: ref __self_0_2,
                } => FFIArrayNativeStruct {
                    ptr: ::core::clone::Clone::clone(&(*__self_0_0)),
                    len: ::core::clone::Clone::clone(&(*__self_0_1)),
                    cap: ::core::clone::Clone::clone(&(*__self_0_2)),
                },
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for FFIArrayNativeStruct {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                FFIArrayNativeStruct {
                    ptr: ref __self_0_0,
                    len: ref __self_0_1,
                    cap: ref __self_0_2,
                } => {
                    let mut debug_trait_builder = f.debug_struct("FFIArrayNativeStruct");
                    let _ = debug_trait_builder.field("ptr", &&(*__self_0_0));
                    let _ = debug_trait_builder.field("len", &&(*__self_0_1));
                    let _ = debug_trait_builder.field("cap", &&(*__self_0_2));
                    debug_trait_builder.finish()
                }
            }
        }
    }
    ///Initialize an `FFIArray*` from across the FFI boundary. This will copy the provided data into Rust
    ///memory.
    ///
    ///# Safety
    ///
    ///The pointer you send must point to the first element of an array whose elements match the type of
    ///`FFIArray*`.
    ///
    ///If `ptr` is a null pointer, this will create an array wrapper with a length and capacity of `0`,
    ///and a null pointer; this expresses the `None` variant of an `Option<Vec<T>>`.
    ///**Important: do not pass a null pointer if the field that this array will be used with is not an
    ///`Option`.**
    ///
    ///This is the only way to safely construct an `FFIArray*` from the non-Rust side of the FFI boundary.
    ///We assume that all instances of `FFIArray*` are allocated by Rust, as this allows us to greatly
    ///simplify memory management.
    #[no_mangle]
    pub unsafe extern "C" fn ffi_array_native_struct_init(
        ptr: *const *const NativeStruct,
        len: isize,
    ) -> FFIArrayNativeStruct {
        let mut v = ::alloc::vec::Vec::new();
        for i in 0..len {
            let e = *ptr.offset(i);
            v.push((&*e).clone());
        }
        (&v).into()
    }
    impl Drop for FFIArrayNativeStruct {
        fn drop(&mut self) {
            {
                ::std::io::_print(::core::fmt::Arguments::new_v1(
                    &["> Dropping reference type array: ", "\n"],
                    &match (&self,) {
                        (arg0,) => {
                            [::core::fmt::ArgumentV1::new(arg0, ::core::fmt::Debug::fmt)]
                        }
                    },
                ));
            };
        }
    }
    impl From<&Vec<NativeStruct>> for FFIArrayNativeStruct {
        fn from(vec: &Vec<NativeStruct>) -> Self {
            let v: std::mem::ManuallyDrop<Vec<*const NativeStruct>> =
                std::mem::ManuallyDrop::new(
                    vec.iter()
                        .map(|e| {
                            let boxed: *const NativeStruct =
                                Box::into_raw(Box::new(e.clone()));
                            boxed
                        })
                        .collect(),
                );
            let len = v.len();
            let ptr = v.as_ptr();
            let cap = v.capacity();
            Self { ptr, len, cap }
        }
    }
    impl From<&Option<Vec<NativeStruct>>> for FFIArrayNativeStruct {
        fn from(opt: &Option<Vec<NativeStruct>>) -> Self {
            opt.as_ref().map_or(
                Self {
                    ptr: std::ptr::null(),
                    len: 0,
                    cap: 0,
                },
                |v| v.into(),
            )
        }
    }
    impl From<FFIArrayNativeStruct> for Vec<NativeStruct> {
        fn from(array: FFIArrayNativeStruct) -> Self {
            unsafe {
                Vec::from_raw_parts(
                    array.ptr as *mut *const NativeStruct,
                    array.len,
                    array.cap,
                )
                .into_iter()
                .map(|e| *Box::from_raw(e as *mut NativeStruct))
                .collect()
            }
        }
    }
    impl From<FFIArrayNativeStruct> for Option<Vec<NativeStruct>> {
        fn from(array: FFIArrayNativeStruct) -> Self {
            if array.ptr.is_null() {
                None
            } else {
                unsafe {
                    Some(
                        Vec::from_raw_parts(
                            array.ptr as *mut *const NativeStruct,
                            array.len,
                            array.cap,
                        )
                        .into_iter()
                        .map(|e| *Box::from_raw(e as *mut NativeStruct))
                        .collect(),
                    )
                }
            }
        }
    }
    ///
    ///Pass an FFI array to this method to allow Rust to reclaim ownership of the object so that it can be safely deallocated.
    ///
    ///# Safety
    ///
    ///We're assuming that the memory in the `array` you give us was allocated by Rust. Don't call this with an object created on the other side of the FFI boundary; that is undefined behavior.
    ///
    ///You **must not** access `array` after passing it to this method.
    ///
    ///Null pointers will be a no-op.
    ///            
    #[no_mangle]
    pub extern "C" fn free_ffi_array_native_struct(array: FFIArrayNativeStruct) {
        error::clear_last_err_msg();
        if array.ptr.is_null() {
            return;
        }
        unsafe {
            let v = Vec::from_raw_parts(
                array.ptr as *mut *const NativeStruct,
                array.len,
                array.cap,
            );
            for e in v {
                let _ = std::boxed::Box::from_raw(e as *mut NativeStruct);
            }
        }
    }
    #[no_mangle]
    pub extern "C" fn native_struct_init(
        an_id: *const c_char,
        a_string: *const c_char,
        an_f32: f32,
        a_datetime: TimeStamp,
        collection_of_ids: FFIArrayString,
        collection_of_enum_variants: FFIArrayNativeEnum,
    ) -> *const NativeStruct {
        let data = NativeStruct {
            an_id: unsafe {
                Uuid::parse_str(CStr::from_ptr(an_id as *mut c_char).to_str().unwrap())
                    .unwrap()
            },
            a_string: unsafe {
                CStr::from_ptr(a_string as *mut c_char)
                    .to_str()
                    .unwrap()
                    .to_string()
            },
            an_f32: an_f32,
            a_datetime: a_datetime.into(),
            collection_of_ids: collection_of_ids.into(),
            collection_of_enum_variants: collection_of_enum_variants.into(),
        };
        Box::into_raw(Box::new(data))
    }
    #[no_mangle]
    ///Get `an_id` for this `NativeStruct`.
    pub unsafe extern "C" fn get_native_struct_an_id(
        ptr: *const NativeStruct,
    ) -> *const c_char {
        let data = &*ptr;
        {
            ffi_common::error::clear_last_err_msg();
            let c_string = match CString::new(data.an_id.to_string()) {
                Ok(val) => val,
                Err(error) => {
                    ::ffi_common::error::set_last_err_msg(error.to_string().as_str());
                    return std::ptr::null();
                }
            };
            let c: *const c_char = c_string.into_raw();
            c
        }
    }
    #[no_mangle]
    ///Get `a_string` for this `NativeStruct`.
    pub unsafe extern "C" fn get_native_struct_a_string(
        ptr: *const NativeStruct,
    ) -> *const c_char {
        let data = &*ptr;
        {
            ffi_common::error::clear_last_err_msg();
            let c_string = match CString::new(data.a_string.to_string()) {
                Ok(val) => val,
                Err(error) => {
                    ::ffi_common::error::set_last_err_msg(error.to_string().as_str());
                    return std::ptr::null();
                }
            };
            let c: *const c_char = c_string.into_raw();
            c
        }
    }
    #[no_mangle]
    ///Get `an_f32` for this `NativeStruct`.
    pub unsafe extern "C" fn get_native_struct_an_f32(ptr: *const NativeStruct) -> f32 {
        ffi_common::error::clear_last_err_msg();
        let data = &*ptr;
        data.an_f32.clone()
    }
    #[no_mangle]
    ///Get `a_datetime` for this `NativeStruct`.
    pub unsafe extern "C" fn get_native_struct_a_datetime(
        ptr: *const NativeStruct,
    ) -> TimeStamp {
        ffi_common::error::clear_last_err_msg();
        let data = &*ptr;
        (&data.a_datetime).into()
    }
    #[no_mangle]
    ///Get `collection_of_ids` for this `NativeStruct`.
    pub unsafe extern "C" fn get_native_struct_collection_of_ids(
        ptr: *const NativeStruct,
    ) -> FFIArrayString {
        ffi_common::error::clear_last_err_msg();
        let data = &*ptr;
        let v = &data.collection_of_ids;
        v.into()
    }
    #[no_mangle]
    ///Get the `collection_of_enum_variants` for a `NativeStruct`.
    pub unsafe extern "C" fn get_native_struct_collection_of_enum_variants(
        ptr: *const NativeStruct,
    ) -> FFIArrayNativeEnum {
        ffi_common::error::clear_last_err_msg();
        let data = &*ptr;
        let v = &data.collection_of_enum_variants;
        v.into()
    }
}
```