# ffi_common

This repository provides libraries for generating a Foreign Function Interface (FFI) for Rust, and
the consumer code for safely interacting with that FFI.

## ffi_common

The main interface. This re-exports the other crates in this workspace and is the only thing that 
consumers should need to depend on.
_Directory:_ [`ffi_common/`](ffi_common)

## ffi_core

Low-level FFI functionality, including error handling, safe string conversion, `Option` and
collection types for primitives, and macros for managing `Option` and collection types for custom 
types.
_Directory:_ [`ffi_core/`](ffi_core)

## ffi_derive

A procedural macro crate with macros for generating an FFI for structs, enums, impls, and fns.
_Directory:_ [`ffi_derive/`](ffi_derive)

## ffi_internals

Internal details, including syntax parsing, Rust code generation for `ffi_derive`, and a `consumer`
module for generating consumer code (currently Swift only).
_Directory:_ [`ffi_internals/`](ffi_internals)
