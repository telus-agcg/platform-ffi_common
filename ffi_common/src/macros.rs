//!
//! Defines macros for generating some common FFI structures and behaviors.
//!

/// Generates the following:
/// 1. A repr(C) struct with a pointer to an array (whose elements are repr(C) value types), its
/// length, and its capacity.
/// 1. `From` impls for converting between `&Vec` of those element types and this new struct.
/// 1. A function for freeing an array of this type.
///
/// Usage looks like:
/// ```
/// declare_value_type_array_struct!(u8);
/// let v: Vec<u8> = vec![1,2,3];
/// let ffi: FFIArrayU8 = (&v).into();
/// ```

///
/// This is intended to be used with numeric primitives, but it may be useful if there are other
/// collections of repr(C) types that we want to pass through the FFI.
///
#[macro_export]
macro_rules! declare_value_type_array_struct {
    ($($t:ident),*) => ($(
        paste! {
            #[doc = """
An FFI-safe representation of a collection of FFI-safe data structures.

This can also express an `Option<Vec<_>>` with a null pointer and a len and capacity of 0. FFI
consumers should therefore make sure that the pointer is not null (although our generated code
should be able to preserve optionality across the FFI boundary, so it will only have to check in
places where null is really possible.)

# Safety

The collection represented by this type needs to be reclaimed by Rust with `Vec::from_raw_parts` so
it can be deallocated safely. Pass this struct to `free_ffi_array_*` when you're done with it (i.e.,
when you've copied it into native memory, displayed it, whatever you're doing on the other side of
the FFI boundary) so we can take care of those steps.
            """]
            #[repr(C)]
            #[derive(Clone, Debug)]
            pub struct [<FFIArray $t:camel>] {
                #[doc = "Pointer to the first element in the array."]
                pub ptr: *const $t,
                #[doc = "The length of (i.e. the number of elements in) this array."]
                pub len: usize,
                #[doc = "The capacity with which this array was allocated."]
                pub cap: usize,
            }

            #[no_mangle]
            #[doc = """
Initialize an `FFIArray*` from across the FFI boundary. This will copy the provided data into Rust
memory.

# Safety

The pointer you send must point to the first element of an array whose elements match the type of
`FFIArray*`.

If `ptr` is a null pointer, this will create an array wrapper with a length and capacity of `0`,
and a null pointer; this expresses the `None` variant of an `Option<Vec<T>>`.
**Important: do not pass a null pointer if the field that this array will be used with is not an
`Option`.**

This is the only way to safely construct an `FFIArray*` from the non-Rust side of the FFI boundary.
We assume that all instances of `FFIArray*` are allocated by Rust, as this allows us to greatly
simplify memory management.
            """]
            pub unsafe extern "C" fn [<ffi_array_ $t:snake _init>](
                ptr: *const $t,
                len: isize,
            ) -> [<FFIArray $t:camel>] {
                let mut v = vec![];
                for i in 0..len {
                    let e = *ptr.offset(i);
                    v.push(e);
                }
                (&v).into()
            }

            impl From<&Vec<$t>> for [<FFIArray $t:camel>] {
                fn from(vec: &Vec<$t>) -> Self {
                    let v: std::mem::ManuallyDrop<Vec<$t>> = std::mem::ManuallyDrop::new(vec.clone());
                    let len = v.len();
                    let ptr = v.as_ptr();
                    let cap = v.capacity();

                    Self { ptr, len, cap }
                }
            }

            impl From<&Option<Vec<$t>>> for [<FFIArray $t:camel>] {
                fn from(opt: &Option<Vec<$t>>) -> Self {
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
            impl From<[<FFIArray $t:camel>]> for Vec<$t> {
                fn from(array: [<FFIArray $t:camel>]) -> Self {
                    unsafe {
                        let v = Vec::from_raw_parts(array.ptr as *mut $t, array.len, array.cap);
                        v
                    }
                }
            }

            impl From<[<FFIArray $t:camel>]> for Option<Vec<$t>> {
                fn from(array: [<FFIArray $t:camel>]) -> Self {
                    if array.ptr.is_null() {
                        None
                    } else {
                        unsafe {
                            let v = Vec::from_raw_parts(array.ptr as *mut $t, array.len, array.cap);
                            Some(v)
                        }
                    }
                }
            }

            impl Drop for [<FFIArray $t:camel>] {
                fn drop(&mut self) {
                    println!("> Dropping value type array: {:?}", self);
                }
            }

            #[doc = """
Pass an FFI array to this method to allow Rust to reclaim ownership of the object so that it can be safely deallocated.

# Safety

We're assuming that the memory in the `array` you give us was allocated by Rust. Don't call this with an object created on the other side of the FFI boundary; that is undefined behavior.

You **must not** access `array` after passing it to this method.

Null pointers will be a no-op.
            """]
            #[no_mangle]
            pub extern "C" fn [<free_ffi_array_ $t:snake>](array: [<FFIArray $t:camel>]) {
                error::clear_last_err_msg();
                if array.ptr.is_null() {
                    return;
                }
                unsafe {
                    let _ = Vec::from_raw_parts(array.ptr as *mut $t, array.len, array.cap);
                }
            }

            #[doc = "An FFI-safe representation of an optional type."]
            #[repr(C)]
            #[derive(Clone, Copy, Debug)]
            pub struct [<Option $t:camel>] {
                #[doc = "True if there's a valid `value`, otherwise false. In the case of false, `value` must be ignored."]
                pub has_value: bool,
                #[doc = "The wrapped value when `has_value` is `true`. This should be considered garbage (i.e., not read) by callers when `has_value` is false."]
                pub value: $t,
            }

            impl From<&Option<$t>> for [<Option $t:camel>] {
                fn from(opt: &Option<$t>) -> Self {
                    match opt {
                        Some(s) => Self {
                            has_value: true,
                            value: s.clone(),
                        },
                        None => Self {
                            has_value: false,
                            value: $t::default(),
                        }
                    }
                }
            }

            impl From<[<Option $t:camel>]> for Option<$t> {
                fn from(opt: [<Option $t:camel>]) -> Self {
                    if opt.has_value {
                        Some(opt.value)
                    } else {
                        None
                    }
                }
            }
        }
    )*);
}

/// Generates the following:
/// 1. A repr(C) struct with a pointer to an array (whose elements are raw `Box<T>`), its
/// length, and its capacity. These elements will be visible across the FFI boundary as opaque
/// pointers, and they will not be deallocated until the struct is passed back to the matching free
/// function (3).
/// 1. `From` impls for converting between `&Vec` of those element types and this new struct.
/// 1. A function for freeing an array of this type.
///
/// Usage looks like:
/// ```
/// #[derive(Debug, Clone)]
/// pub struct Foo {
///     pub bar: i32,
/// }
/// 
/// declare_opaque_type_array_struct!(Foo);
/// 
/// let v: Vec<Foo> = vec![Foo { bar: 1 }, Foo { bar: 2 }, Foo { bar: 3 }];
/// let ffi: FFIArrayFoo = (&v).into();
/// ```
///
/// This is intended to be used with numeric primitives, but it may be useful if there are other
/// collections of repr(C) types that we want to pass through the FFI.
///
#[macro_export]
macro_rules! declare_opaque_type_array_struct {
    ($($t:ident),*) => ($(
        paste! {
            #[doc = """
An FFI-safe representation of a collection of opaque data structures.

This can also express an `Option<Vec<_>>` with a null pointer and a len and capacity of 0. FFI
consumers should therefore make sure that the pointer is not null (although our generated code
should be able to preserve optionality across the FFI boundary, so it will only have to check in
places where null is really possible.)

# Safety

This will need to be brought back into rust ownership in two ways; first, the vec needs to
be reclaimed with `Vec::from_raw_parts`; second, each element of the vec will need
to be reclaimed with `Box::from_raw`. Pass this struct to `free_ffi_array_*` when you're done with
it (i.e., when you've copied it into native memory, displayed it, whatever you're doing on the other
side of the FFI boundary) so we can take care of those steps.
            """]
            #[repr(C)]
            #[derive(Clone, Debug)]
            pub struct [<FFIArray $t:camel>] {
                #[doc = "Pointer to the first element in the array."]
                ptr: *const *const $t,
                #[doc = "The length of (i.e. the number of elements in) this array."]
                len: usize,
                #[doc = "The capacity with which this array was allocated."]
                cap: usize,
            }

            #[doc = """
Initialize an `FFIArray*` from across the FFI boundary. This will copy the provided data into Rust
memory.

# Safety

The pointer you send must point to the first element of an array whose elements are themselves
pointers to Rust-owned instances of opaque types.

If `ptr` is a null pointer, this will create an array wrapper with a length and capacity of `0`,
and a null pointer; this expresses the `None` variant of an `Option<Vec<T>>`.
**Important: do not pass a null pointer if the field that this array will be used with is not an
`Option`.**

This is the only way to safely construct an `FFIArray*` from the non-Rust side of the FFI boundary.
We assume that all instances of `FFIArray*` are allocated by Rust, as this allows us to greatly
simplify memory management.
            """]
            #[no_mangle]
            pub unsafe extern "C" fn [<ffi_array_ $t:snake _init>](
                ptr: *const *const $t,
                len: isize,
            ) -> [<FFIArray $t:camel>] {
                let mut v = vec![];
                for i in 0..len {
                    let e = *ptr.offset(i);
                    v.push((&*e).clone());
                }
                (&v).into()
            }

            impl Drop for [<FFIArray $t:camel>] {
                fn drop(&mut self) {
                    println!("> Dropping reference type array: {:?}", self);
                }
            }

            impl From<&Vec<$t>> for [<FFIArray $t:camel>] {
                fn from(vec: &Vec<$t>) -> Self {
                    let v: std::mem::ManuallyDrop<Vec<*const $t>> = std::mem::ManuallyDrop::new(
                        vec.iter()
                            .map(|e| {
                                let boxed: *const $t = Box::into_raw(Box::new(e.clone()));
                                boxed
                            })
                            .collect()
                    );
                    let len = v.len();
                    let ptr = v.as_ptr();
                    let cap = v.capacity();

                    Self { ptr, len, cap }
                }
            }

            impl From<&Option<Vec<$t>>> for [<FFIArray $t:camel>] {
                fn from(opt: &Option<Vec<$t>>) -> Self {
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

            impl From<[<FFIArray $t:camel>]> for Vec<$t> {
                fn from(array: [<FFIArray $t:camel>]) -> Self {
                    unsafe {
                        Vec::from_raw_parts(array.ptr as *mut *const $t, array.len, array.cap)
                            .into_iter()
                            .map(|e| *Box::from_raw(e as *mut $t))
                            .collect()
                    }
                }
            }

            impl From<[<FFIArray $t:camel>]> for Option<Vec<$t>> {
                fn from(array: [<FFIArray $t:camel>]) -> Self {
                    if array.ptr.is_null() {
                        None
                    } else {
                        unsafe {
                            Some(Vec::from_raw_parts(array.ptr as *mut *const $t, array.len, array.cap)
                                .into_iter()
                                .map(|e| *Box::from_raw(e as *mut $t))
                                .collect())
                        }
                    }
                }
            }

            #[doc = """
Pass an FFI array to this method to allow Rust to reclaim ownership of the object so that it can be safely deallocated.

# Safety

We're assuming that the memory in the `array` you give us was allocated by Rust. Don't call this with an object created on the other side of the FFI boundary; that is undefined behavior.

You **must not** access `array` after passing it to this method.

Null pointers will be a no-op.
            """]
            #[no_mangle]
            pub extern "C" fn [<free_ffi_array_ $t:snake>](array: [<FFIArray $t:camel>]) {
                error::clear_last_err_msg();
                if array.ptr.is_null() {
                    return;
                }
                unsafe {
                    let v = Vec::from_raw_parts(array.ptr as *mut *const $t, array.len, array.cap);
                    for e in v {
                        let _ = std::boxed::Box::from_raw(e as *mut $t);
                    }
                }
            }
        }
    )*);
}
