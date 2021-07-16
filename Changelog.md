# Change Log

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2021-07-16

### Added

- [DEV-15924] Support separating client frameworks by crate.

### Changed

- [DEV-13316] Improved alias resolution.
- [DEV-14638] Working FFI generation for `impl` items.
- [DEV-16437] Harden alias resolution.

## [0.2.1] - 2020-12-16

### Fixed

- Changed `ffi_common`'s `build.rs` to use `OUT_DIR`, allowing for `cargo publish`.

## [0.2.0] - 2020-12-15

### Added

- Add `datetime.rs` for representing timestamps across the FFI boundary.
- Add `macros.rs` for generating FFI types and impls for primitive and opaque types.
- Use those macros to generate FFI types and impls for numeric primitives, strings, and `DateTime`s.
- Add `FFIArrayString` struct, related `From` impl and `ffi_array_string_free` for passing
  collections of strings across the boundary.
- Add `ffi_derive::FFI` macro, with support for generating an interface for:
  - `String`
  - `Uuid`
  - `bool`
  - Numeric primitives (excluding: `isize`, `usize`, `i128`, `u128`)
  - Custom `repr(C)` types
  - Custom non-`repr(C)` types
  - Typealiases over any of the above
  - A few specific generics:
    - `Option<T>` where `T` is any supported type (but not nested `Option<Option<T>>`)
    - `Vec<T>` where `T` is any supported type (but not nested `Vec<Vec<T>>`)
    - `Option<Vec<T>>` where `T` is any supported type (but no additional nesting)
- Add `ffi_consumer` crate for generating native consumer code (hardcoded to Swift for now) to wrap
  the FFI produced by `ffi_derive`.

### Changed

- Move `ffi_common` into a workspace (to make room for `ffi_derive`).
- Move the `try_or_set_error` macro into the `error` module with all of the other error handling.
- Rename `ffi.rs` to `string.rs`, since it only contains string-related FFI behaviors now.

### Fixed

- (Internal) The `0.2.0` tag was cut but version numbers hadn't been bumped; the `0.2.0.1` tag
  represents the actual release.

## [0.1.1] - 2020-08-10

### Changed

- Relaxed version of `cbindgen`.

## [0.1.0] - 2020-07-22

- Initial release.
