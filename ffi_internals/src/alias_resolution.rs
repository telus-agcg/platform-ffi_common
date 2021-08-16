//!
//! This module provides methods for parsing aliases out of a module and writing their definitions
//! to disk, so that when ffi_* macros encounter a field whose type is an alias defined in a remote
//! crate, the macros can identify the underlying type so they can determine safe FFI behavior. For
//! example, if `CrateA` defines
//! ```ignore
//! type Foo = u16;
//! ```
//! and `CrateB` defines a struct with a field whose type uses that alias
//! ```ignore
//! use CrateA::Foo;
//!
//! struct Bar { foo: Foo }
//! ```
//!
//! In order to `#[derive(ffi_derive::FFI)]` on `Bar`, we need to be able to determine that `Foo` is
//! `u16` and should be exposed following the same rules as `u16`. This requires us to
//! 1. Parse the alias definition into data we can work with in procedural macros.
//! 1. Store that data, in a format that support storage of multiple alias sources.
//! 1. Read that data while deriving the FFI for types in any other crate.
//!

use lazy_static::lazy_static;
use proc_macro_error::abort;
use quote::format_ident;
use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};
use syn::{Attribute, Ident, Item, ItemMod, ItemType, Lit, Meta::NameValue, spanned::Spanned, Type, TypePath};

lazy_static! {
    /// The path to the alias map file, behind a `Mutex` to ensure that multiple operations don't
    /// attempt to write to it at once (which could result in a corrupted data structure).
    ///
    /// This is only an issue for tests since they're executed in parallel; rustc doesn't currently
    /// do any parallel compilation. Still better to be safe and be able to test it, though.
    ///
    static ref ALIAS_MAP_PATH: Mutex<String> = Mutex::new(format!("{}/alias_map.json", env!("OUT_DIR")));
}

/// Describes errors that can occurs during alias resolution.
/// 
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An unsupported `ItemType` was encountered.
    #[error("Unsupported ItemType: `{0:?}`")]
    UnexpectedType(Item),
    /// No path segments were found in a `TypePath`.
    #[error("No path segments in TypePath: `{0:?}`")]
    MissingPath(TypePath),
    /// An error occurred when (de)serializing with `serde_json`. 
    #[error("serde_json error: `{0}`")]
    Serde(serde_json::Error),
    /// An error occurred when reading from or writing to the disk.
    #[error("IO error: `{0}`")]
    Io(std::io::Error),
    /// The attribute macro was invoked on an empty module.
    #[error("No module content found for ItemMod: `{0:?}`")]
    EmptyModule(ItemMod),
    /// A mutex error occurred.
    #[error("Mutex error: `{0}`")]
    Mutex(String),
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Serde(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        Self::Mutex(e.to_string())
    }
}

/// Describes the data for a type alias.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
struct AliasDefinition {
    /// The type that a newtype is defined as. In `type Foo = u16`, this is `u16`.
    definition: String,
    /// `Some` if `definition` is itself an alias, so that we can look at the outer keys again.
    definition_source: Option<String>,
}

/// Parses `module` to create a hashmap of alias definitions so that we can resolve aliases to their
/// underlying types when deriving the FFI.
///
/// # Errors
///
/// This function will return an error if anything goes wrong when parsing data out of `module`,
/// getting a lock on the file path, reading the file, or parsing the file's JSON.
///
pub fn parse_alias_module(resolution_key: String, module: ItemMod) -> Result<ItemMod, Error> {
    #[derive(Default)]
    struct ModuleOutput {
        stripped_items: Vec<Item>,
        new_aliases: HashMap<String, AliasDefinition>,
    }

    // Parse the alias resolution data out of `module`.
    let (brace, items) = match module.clone().content {
        Some((b, i)) => (b, i),
        None => return Err(Error::EmptyModule(module)),
    };
    let module_output: ModuleOutput =
        items
            .iter()
            .try_fold(ModuleOutput::default(), |mut acc, item| {
                if let Item::Type(item_type) = item {
                    let definition_source =
                        item_type.attrs.iter().find_map(parse_nested_alias_meta);
                    let new_item = strip_alias_attribute(item_type);
                    acc.stripped_items.push(new_item);

                    if let Type::Path(t) = &*item_type.ty {
                        let segment = match t.path.segments.first() {
                            Some(s) => s,
                            None => return Err(Error::MissingPath(t.clone())),
                        };
                        *acc.new_aliases
                            .entry(item_type.ident.to_string())
                            .or_default() = AliasDefinition {
                            definition: segment.ident.to_string(),
                            definition_source,
                        };
                    } else {
                        return Err(Error::UnexpectedType(item.clone()));
                    }
                } else {
                    acc.stripped_items.push(item.clone());
                }
                Ok(acc)
            })?;

    update_alias_map(resolution_key, module_output.new_aliases)?;

    Ok(ItemMod {
        attrs: module.attrs,
        vis: module.vis,
        mod_token: module.mod_token,
        ident: module.ident,
        content: Some((brace, module_output.stripped_items)),
        semi: module.semi,
    })
}

/// If `type_name` is an alias in `alias_map`, returns the underlying type (resolving aliases
/// recursively, so if someone is weird and defines typealiases over other typealiases, we'll still
/// find the underlying type, as long as they were all specified in the `alias_paths` helper
/// attribute).
///
/// # Errors
///
/// This function will return an error if anything goes wrong when getting a lock on the file path,
/// reading the file, or parsing the file's JSON.
///
pub(super) fn resolve_type_alias(
    type_name: &Ident,
    relevant_modules: &[String],
    alias_map_path: Option<MutexGuard<'_, String>>,
) -> Result<Ident, Error> {
    // Use the path that was passed in (if we already have it and therefore have a lock on it), or
    // get a lock on the path to the alias map file.
    let alias_map_path = match alias_map_path {
        Some(p) => p,
        None => ALIAS_MAP_PATH.lock()?,
    };
    let aliases: HashMap<String, HashMap<String, AliasDefinition>> =
        match std::fs::File::open(&*alias_map_path) {
            Ok(file) => {
                let reader = std::io::BufReader::new(file);
                serde_json::from_reader(reader)?
            }
            Err(_) => {
                return Ok(type_name.clone());
            }
        };

    let aliases_as_idents: HashMap<String, HashMap<Ident, AliasDefinition>> = aliases
        .iter()
        .map(|x| {
            (
                x.0.clone(),
                x.1.iter()
                    .map(|y| (format_ident!("{}", y.0), y.1.clone()))
                    .collect(),
            )
        })
        .collect();

    let maybe_alias = relevant_modules
        .iter()
        .find_map(|m| aliases_as_idents.get(m).and_then(|a| a.get(type_name)));

    match maybe_alias {
        Some(alias) => {
            let field_type = format_ident!("{}", alias.definition);
            let modules_to_check = match &alias.definition_source {
                Some(source) => vec![source.clone()],
                None => relevant_modules.to_owned(),
            };
            resolve_type_alias(&field_type, &*modules_to_check, Some(alias_map_path))
        }
        None => Ok(type_name.clone()),
    }
}

/// Strips the `nested_alias` attribute off of an `&ItemType`'s attributes and returns an `Item`
/// constructed from its other data. The resulting `Item` can be safely sent back to the
/// `TokenStream` and will no longer have the attribute used by the `alias_resolution` attribute
/// macro.
///
fn strip_alias_attribute(item_type: &ItemType) -> Item {
    let stripped_attrs: Vec<Attribute> = item_type
        .attrs
        .clone()
        .into_iter()
        .filter(|a| parse_nested_alias_meta(a).is_none())
        .collect();
    let new_item_type = ItemType {
        attrs: stripped_attrs,
        vis: item_type.vis.clone(),
        type_token: item_type.type_token,
        ident: item_type.ident.clone(),
        generics: item_type.generics.clone(),
        eq_token: item_type.eq_token,
        ty: item_type.ty.clone(),
        semi_token: item_type.semi_token,
    };
    Item::Type(new_item_type)
}

/// Updates the `alias_map` file on disk with a new map of aliases under the `resolution_key`.
///
/// # Errors
///
/// This function will return an error if anything goes wrong when getting a lock on the file path,
/// reading or writing the file, or parsing the file's JSON.
///
fn update_alias_map(
    resolution_key: String,
    new_aliases: HashMap<String, AliasDefinition>,
) -> Result<(), Error> {
    // Read the existing file so we can add to it, or, if it doesn't exist, initialize an empty
    // `HashMap`.
    let alias_map_path = ALIAS_MAP_PATH.lock()?;
    let mut map: HashMap<String, HashMap<String, AliasDefinition>> =
        match std::fs::OpenOptions::new()
            .read(true)
            .open(&*alias_map_path)
        {
            Ok(file) => {
                let reader = std::io::BufReader::new(file);
                serde_json::from_reader(reader)?
            }
            Err(_) => HashMap::new(),
        };

    *map.entry(resolution_key).or_default() = new_aliases;

    // Write `map`, which now also inclues the new alias resolution data for `module`, back to disk.
    std::fs::write(&*alias_map_path, serde_json::to_string(&map)?)?;
    Ok(())
}

/// Reads the `nested_alias` helper attribute, returning `Some(attribute_value)` if it is found,
/// otherwise `None`.
///
fn parse_nested_alias_meta(attr: &Attribute) -> Option<String> {
    if !attr.path.is_ident("nested_alias") {
        return None;
    }
    match attr.parse_meta() {
        Ok(NameValue(name_value)) => {
            if let Lit::Str(s) = name_value.lit {
                return Some(s.value());
            }
            abort!(name_value.span(), "Unexpected nested_alias value: {:?}", name_value)
        }
        Ok(other) => {
            abort!(attr.span(), "Unexpected meta attribute found: {:?}", other)
        }
        Err(err) => {
            abort!(attr.span(), "Error parsing meta attribute: {:?}", err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESOLUTION_KEY1: &str = "test_module_key1";
    const RESOLUTION_KEY2: &str = "test_module_key2";

    fn setup() -> Result<(), Error> {
        // Configure the alias map file with the alias data we'd pull out of a module.
        let mut aliases1 = HashMap::new();
        *aliases1.entry("alias1".to_string()).or_default() = AliasDefinition {
            definition: "u8".to_string(),
            definition_source: None,
        };
        *aliases1.entry("alias2".to_string()).or_default() = AliasDefinition {
            definition: "String".to_string(),
            definition_source: None,
        };
        update_alias_map(RESOLUTION_KEY1.to_string(), aliases1)?;

        // Configure another module's alias data, including one that references an alias from the
        // first module.
        let mut aliases2 = HashMap::new();
        *aliases2.entry("alias3".to_string()).or_default() = AliasDefinition {
            definition: "u16".to_string(),
            definition_source: None,
        };
        *aliases2.entry("alias4".to_string()).or_default() = AliasDefinition {
            definition: "alias1".to_string(),
            definition_source: Some(RESOLUTION_KEY1.to_string()),
        };
        update_alias_map(RESOLUTION_KEY2.to_string(), aliases2)?;
        Ok(())
    }

    #[test]
    fn test_simple_alias_resolution() -> Result<(), Error> {
        setup()?;

        let field_type = format_ident!("alias1");
        let relevant_modules = [RESOLUTION_KEY1.to_string()];
        let expected = format_ident!("u8");
        assert_eq!(
            expected,
            resolve_type_alias(&field_type, &relevant_modules, None).unwrap()
        );
        Ok(())
    }

    #[test]
    fn test_nested_alias_resolution() -> Result<(), Error> {
        setup()?;

        let field_type = format_ident!("alias4");
        let relevant_modules = [RESOLUTION_KEY2.to_string()];
        let expected = format_ident!("u8");
        assert_eq!(
            expected,
            resolve_type_alias(&field_type, &relevant_modules, None).unwrap()
        );
        Ok(())
    }

    #[test]
    fn test_non_alias_type() -> Result<(), Error> {
        setup()?;

        let field_type = format_ident!("i32");
        let relevant_modules = [RESOLUTION_KEY2.to_string()];
        assert_eq!(
            field_type,
            resolve_type_alias(&field_type, &relevant_modules, None).unwrap()
        );
        Ok(())
    }
}
