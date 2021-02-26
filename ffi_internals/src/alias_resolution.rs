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
use quote::format_ident;
use std::{collections::HashMap, io::BufRead, sync::Mutex};
use syn::{Attribute, Ident, Item, ItemMod, ItemType, Lit, Meta::NameValue, Type};

lazy_static! {
    /// The path to the alias map file, behind a `Mutex` to ensure that multiple operations don't
    /// attempt to write to it at once (which could result in a corrupted data structure).
    ///
    /// This is only an issue for tests since they're executed in parallel; rustc doesn't currently
    /// do any parallel compilation. Still better to be safe and be able to test it, though.
    ///
    static ref ALIAS_MAP_PATH: Mutex<String> = Mutex::new(format!("{}/alias_map.json", env!("OUT_DIR")));
}

/// Describes the data for a type alias.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct AliasDefinition {
    /// The type that a newtype is defined as. In `type Foo = u16`, this is `u16`.
    definition: String,
    /// Set if `definition` is itself an alias, so that we can look at the outer keys again.
    definition_source: Option<String>,
}

/// Parses `module` to create a hashmap of alias definitions so that we can resolve aliases to their
/// underlying types when deriving the FFI.
///
pub fn parse_alias_module(resolution_key: String, module: ItemMod) -> ItemMod {

    #[derive(Default)]
    struct ModuleOutput {
        stripped_items: Vec<Item>,
        new_aliases: HashMap<String, AliasDefinition>
    }

    // Parse the alias resolution data out of `module`.
    let (brace, items) = module
        .clone()
        .content
        .unwrap_or_else(|| panic!("No module content? {:?}", module));
    let module_output: ModuleOutput = items
        .iter()
        .fold(ModuleOutput::default(), |mut acc, item| {
            if let Item::Type(item_type) = item {
                let definition_source = item_type.attrs.iter().find_map(parse_nested_alias_meta);
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
                let new_item = Item::Type(new_item_type);
                acc.stripped_items.push(new_item);

                if let Type::Path(t) = &*item_type.ty {
                    let segment = t
                        .path
                        .segments
                        .first()
                        .unwrap_or_else(|| panic!("No path segment? {:?}", t));
                    acc.new_aliases.insert(
                        item_type.ident.to_string(),
                        AliasDefinition {
                            definition: segment.ident.to_string(),
                            definition_source,
                        },
                    );
                } else {
                    panic!(
                        "Found type alias that isn't assigned to a type path. What this? {:?}",
                        item
                    );
                }
            } else {
                acc.stripped_items.push(item.clone());
            }
            acc
        });

    update_alias_map(resolution_key, module_output.new_aliases);

    ItemMod {
        attrs: module.attrs,
        vis: module.vis,
        mod_token: module.mod_token,
        ident: module.ident,
        content: Some((brace, module_output.stripped_items)),
        semi: module.semi,
    }
}

/// If `field_type` is an alias in `alias_map`, returns the underlying type (resolving aliases
/// recursively, so if someone is weird and defines typealiases over other typealiases, we'll still
/// find the underlying type, as long as they were all specified in the `alias_paths` helper
/// attribute).
///
pub(super) fn resolve_type_alias(field_type: &Ident, relevant_modules: &[String]) -> Ident {
    let alias_map_path = ALIAS_MAP_PATH.lock().unwrap();
    let aliases: HashMap<String, HashMap<String, AliasDefinition>> =
        match std::fs::File::open(&*alias_map_path) {
            Ok(file) => {
                let reader = std::io::BufReader::new(file);
                match serde_json::from_reader(reader) {
                    Ok(result) => result,
                    Err(e) => panic!("Can't parse the file {}: {}", alias_map_path, e),
                }
            }
            Err(_) => {
                return field_type.clone();
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
        .find_map(|m| aliases_as_idents.get(m).and_then(|a| a.get(field_type)));

    match maybe_alias {
        Some(alias) => {
            let field_type = format_ident!("{}", alias.definition);
            let modules_to_check = match &alias.definition_source {
                Some(source) => vec![source.to_owned()],
                None => relevant_modules.to_owned(),
            };
            // We need to manually drop alias_map_path here because we're calling
            // `resolve_type_alias` recursively, which will cause us to try to get another lock on
            // `ALIAS_MAP_PATH` on the same thread (which will either deadlock or panic).
            drop(alias_map_path);
            resolve_type_alias(&field_type, &*modules_to_check)
        }
        None => field_type.clone(),
    }
}

/// Updates the alias_map file on disk with a new map of aliases under the `resolution_key`.
///
fn update_alias_map(resolution_key: String, new_aliases: HashMap<String, AliasDefinition>) {
    // Read the existing file so we can add to it, or, if it doesn't exist, initialize an empty
    // `HashMap`.
    let alias_map_path = ALIAS_MAP_PATH.lock().unwrap();
    let mut map: HashMap<String, HashMap<String, AliasDefinition>> =
        match std::fs::OpenOptions::new()
            .read(true)
            .open(&*alias_map_path)
        {
            Ok(file) => {
                let mut reader = std::io::BufReader::new(file);
                if reader.fill_buf().ok().unwrap().is_empty() {
                    HashMap::new()
                } else {
                    match serde_json::from_reader(reader) {
                        Ok(result) => result,
                        Err(e) => panic!("Can't parse the file {}: {}", alias_map_path, e),
                    }
                }
            }
            Err(_) => HashMap::new(),
        };
    map.insert(resolution_key, new_aliases);

    // Write `map`, which now also inclues the new alias resolution data for `module`, back to disk.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&*alias_map_path)
        .unwrap_or_else(|e| panic!("Error opening file to write {}: {}", alias_map_path, e));
    
    serde_json::to_writer(file, &map)
        .unwrap_or_else(|e| println!("Error writing file {}: {}", alias_map_path, e));
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
            panic!("Unexpected nested_alias value: {:?}", name_value);
        }
        Ok(other) => {
            panic!("Unexpected meta attribute found: {:?}", other);
        }
        Err(err) => {
            panic!("Error parsing meta attribute: {:?}", err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESOLUTION_KEY1: &str = "test_module_key1";
    const RESOLUTION_KEY2: &str = "test_module_key2";

    fn setup() {
        // Configure the alias map file with the alias data we'd pull out of a module.
        let mut aliases1 = HashMap::new();
        aliases1.insert(
            "alias1".to_string(),
            AliasDefinition {
                definition: "u8".to_string(),
                definition_source: None,
            },
        );
        aliases1.insert(
            "alias2".to_string(),
            AliasDefinition {
                definition: "String".to_string(),
                definition_source: None,
            },
        );
        update_alias_map(RESOLUTION_KEY1.to_string(), aliases1);

        // Configure another module's alias data, including one that references an alias from the
        // first module.
        let mut aliases2 = HashMap::new();
        aliases2.insert(
            "alias3".to_string(),
            AliasDefinition {
                definition: "u16".to_string(),
                definition_source: None,
            },
        );
        aliases2.insert(
            "alias4".to_string(),
            AliasDefinition {
                definition: "alias1".to_string(),
                definition_source: Some(RESOLUTION_KEY1.to_string()),
            },
        );
        update_alias_map(RESOLUTION_KEY2.to_string(), aliases2);
    }

    #[test]
    fn test_simple_alias_resolution() {
        setup();

        let field_type = format_ident!("alias1");
        let relevant_modules = [RESOLUTION_KEY1.to_string()];
        let expected = format_ident!("u8");
        assert_eq!(expected, resolve_type_alias(&field_type, &relevant_modules));
    }

    #[test]
    fn test_nested_alias_resolution() {
        setup();

        let field_type = format_ident!("alias4");
        let relevant_modules = [RESOLUTION_KEY2.to_string()];
        let expected = format_ident!("u8");
        assert_eq!(expected, resolve_type_alias(&field_type, &relevant_modules));
    }

    #[test]
    fn test_non_alias_type() {
        setup();

        let field_type = format_ident!("i32");
        let relevant_modules = [RESOLUTION_KEY2.to_string()];
        assert_eq!(
            field_type,
            resolve_type_alias(&field_type, &relevant_modules)
        );
    }
}
