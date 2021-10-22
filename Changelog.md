# Change Log

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- Add support for enums with associated values. DEV-17613.
- New helper attributes:
  - `description`: Names an impl so we can make the generated module and consumer file unique even
  when it's not a trait impl.
  - `generic`: Specifies a type to expose something as, instead of a generic.
  - `forbid_memberwise_init`: Prevents us from generating a memberwise initializer (for cases where
  initialization should only happen through specific APIs with extra rules).

### Changed

- Move the consumer type outputs into a trait for consistency/DRYing.

## [0.6.0] - 2021-09-20

### Changed

- Update crate versions to fix dependency management in downstream crates.

## [0.5.1] - 2021-09-20

### Changed

- Fix tests.

## [0.5.0] - 2021-09-22

### Added

- Add support for `Option` and `Result` return types.
- Add support for borrowed parameters.
- Add `ffi_derive::expose_fn` for exposing a single function (as opposed to a whole impl with
  `ffi_derive::expose_impl`).

### Changed

- Replace panics with `proc_macro_error` calls.

### Removed

- Remove `ffi_consumer` crate. This functionality is now provided by `ffi_internals::consumer`.

## [0.4.1] - 2021-08-11

### Changed

- [DEV-17207] Moved tests from `ffi_derive` to `ffi_common`, effectively sorting out the recursive
  dependency problem between the crates.

## [0.4.0] - 2021-08-10

- Note: The release of `ffi_derive` is versioned to `0.3.99` due to a recursive dependency loop on
  it and `ffi_common`.

### Added

- `ffi_core`

### Changed

- `ffi_common` reexports
  - `ffi_derive`
  - `ffi_consumer`
  - `ffi_core`
- `ffi_internals` reexports
  - `heck`
  - `syn`
  - `quote`
- `ffi_core` reexports
  - `paste::paste`

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
