//! # `ffi_derive`
//!
//! A library for deriving a C foreign function interface (FFI) from type definitions in Rust.
//!
//! ## Design:
//!
//! We want to be able to share common resource type definitions across all platforms, which will
//! provide a foundation for sharing more complex code. We can do that in Rust, but it requires
//! duplicating type definitions and mananging memory in a wrapper for each Rust FFI library, which
//! is extremely time consuming and can be tricky to get right. This library aims to address those
//! problems by making it trivial to derive a safe native interface for other languages from a
//! native rust interface.
//!
//! We do this by:
//! 1. Generating an FFI module for every `ffi_derive` type (that's the main job of this crate).
//! 1. Generating a C header with [cbindgen](https://github.com/eqrion/cbindgen).
//!     * The cbindgen configuration needs to be defined in the client library. We may eventually be
//! able to produce one as a convenience, but for now you'll need to be able to tell cbindgen what
//! you want exposed.
//!     * `cbindgen` requires a feature from nightly Rust in order to see the derived types and
//! functions, so you'll need to invoke it like
//!         ```bash
//!         rustup run nightly cbindgen \
//!             --config /path/to/cbindgen.toml \
//!             --crate crate_name \
//!             --output path/to/header.h
//!         ```
//! 1. Generating a native interface in one of the supported languages with
//! [`ffi_internals::consumer`]. This wraps the C headers generated by the previous step in native
//! types for the specified language.
//!
//! ### Additional design considerations:
//! * Using or defining a type that happens to have a derived FFI must not be any different from
//! using or defining a non-FFI type. We want it to be easy to make any Rust type provide an FFI
//! module, with minimal design considerations for that use case.
//! * The C interface generated by this library should **not** be used directly. The generated code
//! in the FFI modules relies on invariants that we uphold in our generated native wrappers, which
//! lets us simplify and optimize (for example, we don't have to worry about non-optional types
//! coming in as `std::ptr::null`, or who initialized some piece of memory, because we control both
//! sides of the C interface). We can't enforce that if you use the C interface directly, so you
//! may run into `panic`s, UB, etc.
//!
//! ### Alternatives
//! There are many ways to provide a Rust FFI, some of which may be more appropriate in certain
//! contexts.
//! 1. A simple C FFI can use JSON (de)serialization to exchange data, but this requires
//! implementing any necessary types and (de)serialization in every foreign client, and the overhead
//! for converting all data to/from JSON isn't trivial if you want to use Rust in a front-end
//! application.
//! 1. [Manually wrapping native types](https://github.com/agrian-inc/wise_units/tree/develop/ffi)
//! is another option, with a somewhat cleaner interface, but this ends up with _even more_
//! duplication, and has more complex memory management to worry about.
//! 1. There are [other options](https://docs.rs/ffi-support/0.4.2/ffi_support/) that provide
//! utilities for making an FFI safer, but definitions still need to be duplicated and memory still
//! needs to be managed individually in wrapping libraries.
//!
//! ## Supported types:
//! 1. `String`.
//! 1. `Uuid`.
//! 1. Numeric primitives (`u8` through `f64`).
//! 1. Custom `repr(C)` types.
//! 1. Custom non-`repr(C)` types.
//! 1. Typealiases over any of the above.
//! 1. Typealiases defined in remote crates (see `Remote types` section).
//! 1. Remote types with custom FFI implementations (see `Remote types` section).
//! 1. Enums whose variants have one or zero fields (additional fields need more work in
//! [this GitHub issue](https://github.com/agrian-inc/ffi_common/issues/22)).
//! 1. A few specific generics:
//!   1. `Option<T>` where `T` is any supported type (but not nested `Option<Option<T>>`).
//!   1. `Vec<T>` where `T` is any supported type (but not nested `Vec<Vec<T>>`).
//!   1. `Option<Vec<T>>` where `T` is any supported type (but no additional nesting).
//!
//! ## Using `ffi_derive`
//!
//! With simple enums or structs that can be marked `repr(C)`, you can do something like this, where
//! `cffi` is a feature that determines whether you're building for a C FFI (so that compiling for
//! other purposes isn't constrained to C's memory layout rules):
//! ```
//! #[cfg_attr(feature = "cffi", repr(C), no_mangle, derive(ffi_derive::FFI))]
//! pub enum NativeEnum {
//!     V1,
//!     V2,
//! }
//!
//! impl Default for NativeEnum {
//!     fn default() -> Self {
//!         Self::V1
//!     }
//! }
//! ```
//!
//! With more complicated structs (the primary focus of this library), you can similarly use a
//! feature to control when the type is built with `ffi_derive`.
//!
//! Typealiases are supported, with the caveat that you have to provide the resolution key for the
//! module(s) in which any typealiases used in the declaration of this type are defined (see
//! `ffi(alias_paths(some_module_ids))` in the example below; if `NativeStructId` was an alias over
//! `Uuid` defined in a file at that path (relative to the root of the crate), we'd figure out what
//! type to treat that field as for the purposes of FFI.)
//!
//! Custom types that are safe to use directly in FFI can be marked `ffi(raw)` (see `enum_variant`
//! in the example below).
//! ```ignore
//! #[cfg_attr(
//!     feature = "cffi",
//!     derive(ffi_derive::FFI),
//!     ffi(alias_paths(some_module_ids))
//! )]
//! #[derive(Clone, Debug)]
//! pub struct NativeStruct {
//!     pub a_native_struct_id: NativeStructId,
//!     pub a_string: String,
//!     pub an_f32: f32,
//!     pub a_datetime: NaiveDateTime,
//!     pub collection_of_ids: Vec<Uuid>,
//!     #[cfg_attr(feature = "cffi", ffi(raw))]
//!     pub enum_variant: NativeEnum,
//! }
//! ```
//!
//! ## Custom implementations
//!
//! Some types (like `wise_units::Unit`) don't fit the pattern of deriving an FFI for their visible
//! fields; their internal structure isn't FFI-safe, or isn't a useful interface (for example, we
//! care about a `wise_units::Unit` as a thing that can be initialized from a UCUM expression, and
//! we need to be able to read a UCUM expression out of it, but we don't care about its `terms`
//! field, or the fields of the `Term` type, etc).
//!
//! In those cases, we want to let a type be `ffi_derive`d so it can take advantage of all the
//! boilerplate stuff + get a consumer generated for itself, but provide its own implementation of
//! an initializer and getter functions. The `custom` helper attribute lets us point to to a file
//! that describes the base interface for the type, as in `ffi(custom = "src/unit/custom_ffi.rs")`.
//!
//! If the custom initializer takes a pointer type (like a `* const c_char` for an FFI string), we
//! can't automatically detect whether the parameter is required. If you want to specify which
//! parameters are required, you can do so by prefixing parameter names with `required_` (as in
//! `required_param1`), and we will enforce that in the generated consumer types.
//!
//! See `../../ffi_common/tests/custom_ffi` for an example.
//!
//! ## Remote types
//!
//! Sometimes we'll want to expose a field whose type is defined in a crate we don't control (like
//! boundaries in `agrian_types`, which are usually `geo_types::MultiPolygon<T>`). Since we can't
//! derive an FFI for remote types, we need to be able to point at another type that the remote type
//! can be converted into. This wrapping type (which can either have a derived or a custom FFI
//! implementation) can be specified with the `expose_as` helper attribute, as in
//! `ffi(expose_as = "crate::multi_polygon_ffi::MultiPolygonWrapper")`.
//!
//! See `../tests/remote_types` for an example.
//!
//! Similarly, sometimes a type we want to expose will use a typealias defined in a remote crate.
//! We support that, but because the type information that backs the alias isn't available at the
//! time procedural macros run, we require some additional configuration in both the module that
//! defines the alias, and on the type whose fields are defined with the alias type.
//!
//! ### Remote alias definitions
//!
//! When a module defines aliases that may be used on a type that derives an FFI, the
//! `alias_resolution` attribute macro needs to be run on it in order to populate the definitions of
//! those aliases somewhere so that we can look them up when resolving the underlying types of
//! fields whose type is an alias. The macro invocation also needs to define a unique string for the
//! module (which we refer to internally as the `resolution_key`). This will be used with a helper
//! attribute on types that derive an FFI so that we can identify the source where their aliases are
//! defined.
//!
//! Invoking the alias resolution macro on a module looks like this:
//! ```
//! #[ffi_derive::alias_resolution(some_unique_string)]
//! mod aliases_here {
//!     pub type Foo = u8;
//! }
//! ```
//!
//! Finally, an alias may be defined over another alias (which is odd but happens). We support those
//! cases, but require an additional helper attribute on the alias declaration to tell us where
//! *that* alias is defined. For example:
//! ```ignore
//! #[ffi_derive::alias_resolution(crate1_aliases)]
//! mod aliases_in_crate1 {
//!     pub type Foo = u8;
//! }
//!
//! #[ffi_derive::alias_resolution(crate2_aliases)]
//! mod aliases_in_crate2 {
//!     #[nested_alias="crate1_aliases"]
//!     pub type Bar = Foo;
//! }
//! ```
//!
//! ### Remote aliases in type definitions
//!
//! When an `ffi_derive` type includes a field whose type is an alias defined in a remote crate,
//! the `ffi_derive` macro invocation just needs to include the helper attribute
//! `ffi(alias_modules(a_key))` to tell us the resolution keys of the modules in which those aliases
//! are defined. For example:
//! ```ignore
//! #[ffi_derive::alias_resolution(crate1_aliases)]
//! mod aliases_in_crate1 {
//!     pub type Foo = u8;
//! }
//!
//! #[derive(ffi_derive::FFI), ffi(alias_modules(agrian_types_ids))]
//! pub struct SomeTypeInCrate2 {
//!     pub field: Foo
//! }
//! ```
//!
//! It's worth noting that there's potential for a couple different issues here. First, if a type
//! provides multiple keys in `alias_modules`, and an identical alias is defined in each of those
//! modules, we may interpret the type incorrectly. If that scenario comes up, we can work around it
//! by moving the helper attribute from the struct to the individual fields (since there we only
//! need to point to one `alias_module` at a time), but it gets awfully tedious, so we're not doing
//! that yet. Second, if a type is renamed when it's imported (as in
//! `use crate1::aliases::Foo as Meow`), or uses a fully qualified path instead of an import (as in
//! `pub field foo: crate1::aliases::Foo`), we won't be able to figure out how to go from that
//! definition to `Foo` to `u8`.
//!
//! ### Remote types and multiple consumer frameworks
//!
//! It's generally useful for consumers to separate the generated code produced by these `ffi_*`
//! crates into multiple consumer frameworks. This lets them mirror the crate structure instead of
//! having a single monolithic framework interface. To support that, `ffi_derive` needs to know
//! which remote types need to be imported for the consumer code. This can be expressed with the
//! `consumer_imports` attribute. For example:
//! ```ignore
//! use other_crate::module::OtherType;
//!
//! #[derive(ffi_derive::FFI), ffi(consumer_imports(other_crate::module::OtherType))]
//! pub struct SomeType {
//!     pub field: OtherType
//! }
//! ```
//!
//! This allows us to include an import statement like `import OtherCrate.OtherType` at the top of
//! the generated consumer file.
//!
//! ## Deriving on an impl
//!
//! We also support generating an FFI for trait implementations with the `expose_impl` attribute
//! macro.
//!
//! Couple of limitations here:
//! 1. As mentioned above, we currently only support trait implementations (because we use the trait
//! name + type name to generate a unique module name as a container for the FFI functions).
//! Inherent implementations (like `impl Foo { ... }`) won't work (yet).
//! 1. The invocation site needs to provide the paths to the FFI modules that we'll need to import.
//! For example, the FFI form of a function like
//! `fn meow(&self, volume: Option<Volume>) -> Vec<Meow> { ... }` will need to know the paths to the
//! FFI types of `Meow` and `Volume`. Fortunately, since those FFI types probably come from
//! `ffi_derive`, it's easy to figure out what they would be based on your normal imports. If you do
//! something like `use crate::animals::cats::Meow;` and `use utilities::sound::Volume;`, you'll
//! just need to provide `"crate::animals::cats::meow_ffi", "utilities::sound::volume_ffi"` to the
//! attribute macro.
//!
//! Invoking the `expose_impl` macro looks like this:
//! ```ignore
//! use crate::animals::cats::Meow;
//! use utilities::sound::Volume;
//! #[ffi_derive::expose_impl(animals::cats::meow_ffi::FFIArrayMeow)]
//! impl Meows for Cat {
//!     pub fn meow(&self, volume: Option<Volume>, count: u8) -> Vec<Meow> { ... }
//! }
//! ```
//! and generates a module like this:
//! ```ignore
//! pub mod meows_cat_ffi {
//!     pub unsafe extern "C" fn meow(
//!         cat: *const Cat,
//!         volume: *mut Volume,
//!         count: u8
//!     ) -> FFIArrayMeow { ... }
//! }
//! ```
//!

#![deny(unused_extern_crates, missing_docs)]
#![warn(
    clippy::all,
    clippy::correctness,
    clippy::nursery,
    clippy::pedantic,
    future_incompatible,
    missing_copy_implementations,
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unused_qualifications,
    unused_results,
    variant_size_differences
)]

use ffi_internals::{
    alias_resolution,
    consumer::{consumer_enum, consumer_struct::ConsumerStruct, ConsumerOutput},
    heck::SnakeCase,
    items::{
        enum_ffi,
        fn_ffi::FnFFI,
        impl_ffi::{ImplFFI, ImplInputs},
        struct_ffi::{custom, standard},
    },
    parsing,
    quote::{format_ident, ToTokens},
    syn::{
        parse_macro_input, spanned::Spanned, AttributeArgs, Data, DeriveInput, ItemFn, ItemImpl,
        ItemMod, Type,
    },
};
use proc_macro::TokenStream;
use proc_macro_error::{abort, proc_macro_error};

/// Derive an FFI for a native type definition.
///
/// # Supported Attributes
///
/// The following attributes can be specified with the `ffi` helper attribute when using this derive
/// macro, as in `#[derive(ffi_derive::FFI), ffi(attribute1(args), attribute2)]`.
///
/// ## Struct
///
/// - *alias_modules*: A list of modules that contain aliases referenced by this type, as in
/// `ffi(alias_modules(some_module_name, some_other_module_name))`. Note that this must be the name
/// used when the `alias_resolution` macro was invoked on the module containing aliases, as in
/// `ffi_common::derive::alias_resolution(some_module_name)`.
/// - *forbid_memberwise_init*: This attribute takes no arguments; instead, its presence indicates
/// that we should not generate a memberwise initializer for this type. Usage looks like
/// `ffi(forbid_memberwise_init)`.
/// - *consumer_imports*: A list of paths to be imported into the consumer type definition. These
/// should be absolute paths to remote crates; the goal here is to let the consumer set up
/// frameworks that mirror the crate structure, which means they'll sometimes need to specify that a
/// type needs to be imported from some other framework. This looks like
/// `ffi(consumer_imports(remote_crate::module::Type))`.
/// - *ffi_mod_imports*: A list of absolute paths to be imported in the FFI module, as in
/// `ffi(ffi_mod_imports(crate::module::nested_module::Type))`. This does not need to include paths
/// that are already in scope at the level where this type is defined; those will be imported into
/// the FFI module automatically.
///
/// # Fields
///
/// ## Custom Struct
///
/// The following additional attributes are valid for structs with a manually implemented FFI:
///
/// - *custom*: The filepath (relative to the crate's root) to the file containing the FFI
/// implementation for the struct, as in `ffi(custom = "src/directory/file.rs")`.
/// - *failable_fns*: A collection of paths to functions that can fail (i.e., return a `Result`), as
/// in `ffi(failable_fns(module1::fn1, module2::fn2))`.
/// - *failable_init*: This attribute takes no arguments; instead, its presence indicates that the
/// initializer for this struct is failable (i.e., returns a `Result`). Usage looks like
/// `ffi(failable_init)`.
///
/// # Proc Macro Errors
///
/// Fails if invoked for an unsupported type (such as a union or non-repr(C) enum), or if any
/// unsupported types are encountered when processing `input`.
///
#[proc_macro_derive(FFI, attributes(ffi))]
#[proc_macro_error]
pub fn ffi_derive(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = ffi_internals::syn::parse(input).unwrap();

    // Build the trait implementation
    impl_ffi_macro(&ast)
}

fn impl_ffi_macro(ast: &DeriveInput) -> TokenStream {
    // Get the relative file paths from the attribute args, prefix them with the cargo
    // manifest dir, then build a hash map for resolving type aliases.
    let crate_root = std::env::var("CARGO_MANIFEST_DIR").expect(
        "Could not find `CARGO_MANIFEST_DIR` to look up aliases in `ffi_derive::impl_ffi_macro`.",
    );
    let out_dir = out_dir();
    let type_name = ast.ident.clone();
    let module_name = format_ident!("{}_ffi", &type_name.to_string().to_snake_case());
    let struct_attributes = parsing::StructAttributes::from(&*ast.attrs);
    let doc_comments = ffi_internals::parsing::clone_doc_comments(&*ast.attrs);
    match &ast.data {
        Data::Struct(data) => struct_attributes.custom_attributes.as_ref().map_or_else(
            || {
                let ffi = standard::StructFFI::from(&standard::StructInputs {
                    module_name: &module_name,
                    type_name: &type_name,
                    data,
                    alias_modules: &*struct_attributes.alias_modules,
                    consumer_imports: &struct_attributes.consumer_imports,
                    ffi_mod_imports: &struct_attributes.ffi_mod_imports,
                    forbid_memberwise_init: struct_attributes.forbid_memberwise_init,
                    doc_comments: &doc_comments,
                });
                (&ConsumerStruct::from(&ffi)).write_output(&out_dir);
                proc_macro2::TokenStream::from(ffi)
            },
            |custom_attributes| {
                let ffi = custom::StructFFI::new(
                    &type_name,
                    &module_name,
                    &crate_root,
                    custom_attributes,
                    &*struct_attributes.consumer_imports,
                    &*struct_attributes.ffi_mod_imports,
                    struct_attributes.forbid_memberwise_init,
                    &doc_comments,
                );
                (&ConsumerStruct::from(&ffi)).write_output(&out_dir);
                proc_macro2::TokenStream::from(ffi)
            },
        ),
        Data::Enum(data) => {
            if parsing::is_repr_c(&ast.attrs) {
                let ffi = enum_ffi::reprc::EnumFFI::new(&module_name, &type_name);
                (&consumer_enum::ReprCConsumerEnum::from(&ffi)).write_output(&out_dir);
                ffi.into()
            } else {
                let ffi: enum_ffi::complex::EnumFFI<'_> = enum_ffi::complex::EnumFFI::new(
                    &module_name,
                    &type_name,
                    data,
                    &*struct_attributes.alias_modules,
                    &*struct_attributes.consumer_imports,
                    &*struct_attributes.ffi_mod_imports,
                    &doc_comments,
                );
                (&consumer_enum::ComplexConsumerEnum::from(&ffi)).write_output(&out_dir);
                proc_macro2::TokenStream::from(ffi)
            }
        }
        Data::Union(_) => abort!(type_name.span(), "Unions are not supported"),
    }
    .into()
}

fn out_dir() -> String {
    let root_output_dir = option_env!("FFI_CONSUMER_ROOT_DIR").unwrap_or_else(|| env!("OUT_DIR"));
    let package_name = std::env::var("CARGO_PKG_NAME").unwrap();
    format!("{}/{}", root_output_dir, package_name)
}

/// Parses a module that contains typealiases and stores that information for other `ffi_derive` calls
/// to use later in resolving aliases.
///
/// # Proc Macro Errors
///
/// Fails if this is not invoked on a module, or if the resolution JSON file cannot be read or
/// written to.
///
#[proc_macro_attribute]
#[proc_macro_error]
pub fn alias_resolution(attr: TokenStream, item: TokenStream) -> TokenStream {
    let resolution_key = attr.to_string();
    let module = parse_macro_input!(item as ItemMod);
    let err_span = module.span();
    alias_resolution::parse_alias_module(resolution_key, module)
        .unwrap_or_else(|err| abort!(err_span, "Error parsing alias module: {}", err))
        .into_token_stream()
        .into()
}

/// Parses an impl and produces a module exposing that impl's functions over FFI.
///
/// # Attributes
///
/// The following attributes can be specified when using this attribute macro, as in
/// `#[ffi_derive::expose_impl(attribute1(args), attribute2)]`.
///
/// - *description*: A descriptive name for this impl, which will be combined with the name of the
/// type to uniquely identify the generated module/consumer file. If you're using this macro with an
/// impl for a type like `impl SomeType { ... }`, this attribute is *required*. If you're using this
/// macro when  implementing a trait for a type like `impl SomeTrait for SomeType`, this attribute
/// is optional  (we'll fall back to the trait name for unique naming). This looks like
/// `description("some_description")`.
/// - *ffi_imports*: A list of absolute paths to be imported in the FFI module, as in
/// `ffi_imports(crate::module::nested_module::Type)`.
/// - *consumer_imports*: A list of absolute paths of types that need to be imported in the consumer
/// module, as in `consumer_imports(crate::module::Type)`.
/// - *generic*: A list of generic parameters used in this impl and the concrete types to use for the
/// generated FFI. This looks like `generic(T="ConcreteType")`.
/// - *raw_types*: A list of types that should be exposed directly through the FFI when referenced in
/// this impl. Generally this should just be types that are `repr(C)`. This looks like
/// `raw_types(Type)`.
///
/// # Proc Macro Errors
///
/// Fails if invoked on an unsupported impl, such as: one that doesn't implement a trait, one
/// that doesn't have a `Self` type, or one whose types use aliases that have not been marked
/// `derive(ffi_derive::alias_resolution)`.
///
#[proc_macro_attribute]
#[proc_macro_error]
pub fn expose_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as AttributeArgs);
    let impl_attributes = parsing::ImplAttributes::from(args);
    let item_impl = parse_macro_input!(item as ItemImpl);

    let impl_description = impl_attributes.description.unwrap_or_else(|| {
        item_impl.trait_.as_ref().map_or_else(
            || abort!(item_impl.span(), "No trait info found"),
            |t| t.1.segments.last().unwrap().ident.clone(),
        )
    });
    let type_name = if let Type::Path(ty) = &*item_impl.self_ty {
        ty.path.segments.last().unwrap().ident.clone()
    } else if let Type::Reference(r) = &*item_impl.self_ty {
        if let Type::Path(ty) = &*r.elem {
            ty.path.segments.last().unwrap().ident.clone()
        } else {
            abort!(
                r.span(),
                "Could not find self type for impl in Type::Reference"
            );
        }
    } else if let Type::Group(g) = &*item_impl.self_ty {
        if let Type::Path(ty) = &*g.elem {
            ty.path.segments.last().unwrap().ident.clone()
        } else {
            abort!(g.span(), "Could not find self type for impl in Type::Group");
        }
    } else {
        abort!(
            item_impl.self_ty.span(),
            "Could not find self type for impl"
        );
    };

    let impl_ffi = ImplFFI::from(ImplInputs {
        items: item_impl.items.clone(),
        ffi_imports: impl_attributes.ffi_imports,
        consumer_imports: impl_attributes.consumer_imports,
        raw_types: impl_attributes.raw_types,
        generics: impl_attributes.generics,
        impl_description,
        type_name,
        doc_comments: parsing::clone_doc_comments(&*item_impl.attrs),
    });
    let out_dir = out_dir();
    let file_name = impl_ffi.consumer_file_name();
    ffi_internals::write_consumer_file(&file_name, String::from(&impl_ffi), &out_dir)
        .unwrap_or_else(|err| abort!(item_impl.span(), "Error writing consumer file: {}", err));
    let ffi = impl_ffi.generate_ffi();

    let output = ffi_internals::quote::quote! {
        #item_impl

        #ffi
    };

    output.into()
}

/// Parses a fn and produces a module exposing that function over FFI.
///
/// # Attributes
///
/// The following attributes can be specified when using this attribute macro, as in
/// `#[ffi_derive::expose_fn(attribute1(args), attribute2)]`.
///
/// - *generic*: A list of generic parameters used in this impl and the concrete types to use for the
/// generated FFI. This looks like `generic(T="ConcreteType")`.
/// - *extend_type*: The type to extend on the consumer with this function. We don't currently support
/// generating global consumer functions, so `extend_type` is used to associate this behavior with
/// that type. This is *also* used as the type of `Self` when necessary. This looks like
/// `extend_type(Type)`.
/// - *raw_types*: A list of types that should be exposed directly through the FFI when referenced in
/// this impl. Generally this should just be types that are `repr(C)`. This looks like
/// `raw_types(Type)`.
///
#[proc_macro_attribute]
#[proc_macro_error]
pub fn expose_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = ffi_internals::syn::parse_macro_input!(attr as AttributeArgs);
    let fn_attributes = parsing::FnAttributes::from(args);
    let item_fn = ffi_internals::syn::parse_macro_input!(item as ItemFn);

    let fn_ffi = FnFFI::from((&item_fn, &fn_attributes));
    let module_name = format_ident!("{}_ffi", item_fn.sig.ident);
    let file_name = [&module_name.to_string(), ".swift"].join("");
    let out_dir = out_dir();

    ffi_internals::write_consumer_file(
        &file_name,
        fn_ffi.generate_consumer_extension(&fn_attributes.extend_type.to_string(), &module_name),
        &out_dir,
    )
    .unwrap_or_else(|err| abort!(item_fn.span(), "Error writing consumer file: {}", err));

    let ffi = fn_ffi.generate_ffi(&module_name, None, None);

    let output = ffi_internals::quote::quote! {
        #item_fn

        #ffi
    };

    output.into()
}
