# Change Log

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### [Added]

- `FFI` derive macro, with support for generating an interface for:
    - `String`.
    - `Uuid`.
    - Numeric primitives (`u8` through `f64`).
    - Custom `repr(C)` types.
    - Custom non-`repr(C)` types.
    - Typealiases over any of the above.
    - A few specific generics:
        - `Option<T>` where `T` is any supported type (but not nested `Option<Option<T>>`).
        - `Vec<T>` where `T` is any supported type (but not nested `Vec<Vec<T>>`).
        - `Option<Vec<T>>` where `T` is any supported type (but no additional nesting).
- `example.md` to show the results of `cargo expand` without having to build a separate library. 
