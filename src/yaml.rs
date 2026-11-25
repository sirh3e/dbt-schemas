//! Marked-YAML helpers.
//!
//! Each dbt YAML file is parsed exactly once with saphyr; the marked tree
//! yields (a) a [`serde_json::Value`] fed into the generated types, as saphyr
//! does not deserialize into Serde types itself, and (b) the position of any
//! node, for what needs a range in the file.

use std::path::Path;

pub use saphyr::MarkedYaml;
use saphyr::{LoadableYamlNode, Scalar, YamlData};
use serde::de::DeserializeOwned;

use crate::{Error, Result};

/// A 0-based place in a text: the line, and the column in characters.
///
/// The same numbering as the Language Server Protocol's `Position`, so a
/// language server converts it field for field.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// A node's extent: from its start to its end, the end exclusive.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

/// Parse the first YAML document of `text`.
pub fn load_marked<'a>(text: &'a str, path: &Path) -> Result<MarkedYaml<'a>> {
    let docs = MarkedYaml::load_from_str(text).map_err(|e| Error::Yaml {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    docs.into_iter().next().ok_or_else(|| Error::Yaml {
        path: path.to_path_buf(),
        message: "empty yaml document".into(),
    })
}

/// Deserialize a value made by [`to_json`] into a generated type.
pub fn from_json<T: DeserializeOwned>(value: serde_json::Value, path: &Path) -> Result<T> {
    serde_json::from_value(value).map_err(|source| Error::Shape {
        path: path.to_path_buf(),
        source,
    })
}

/// 0-based position of a node's start (saphyr's lines are 1-based, its
/// columns 0-based).
pub fn position_of(node: &MarkedYaml<'_>) -> Position {
    Position {
        line: node.span.start.line().saturating_sub(1) as u32,
        character: node.span.start.col() as u32,
    }
}

/// 0-based extent of a node, its end exclusive.
pub fn span_of(node: &MarkedYaml<'_>) -> Span {
    Span {
        start: position_of(node),
        end: Position {
            line: node.span.end.line().saturating_sub(1) as u32,
            character: node.span.end.col() as u32,
        },
    }
}

/// Look up a mapping entry by string key.
pub fn get<'a, 'input>(node: &'a MarkedYaml<'input>, key: &str) -> Option<&'a MarkedYaml<'input>> {
    match &node.data {
        YamlData::Mapping(map) => map
            .iter()
            .find(|(k, _)| as_str(k).is_some_and(|s| s == key))
            .map(|(_, v)| v),
        _ => None,
    }
}

/// The text of a string scalar.
pub fn as_str<'a>(node: &'a MarkedYaml<'_>) -> Option<&'a str> {
    match &node.data {
        YamlData::Value(Scalar::String(s)) => Some(s.as_ref()),
        YamlData::Representation(s, _, _) => Some(s.as_ref()),
        _ => None,
    }
}

/// The entries of a mapping, in file order; none for anything else.
pub fn entries<'a, 'input>(
    node: &'a MarkedYaml<'input>,
) -> impl Iterator<Item = (&'a MarkedYaml<'input>, &'a MarkedYaml<'input>)> {
    let map = match &node.data {
        YamlData::Mapping(map) => Some(map),
        _ => None,
    };
    map.into_iter().flat_map(|m| m.iter())
}

/// The items of a sequence, in file order; none for anything else.
pub fn items<'a, 'input>(
    node: &'a MarkedYaml<'input>,
) -> impl Iterator<Item = &'a MarkedYaml<'input>> {
    let seq = match &node.data {
        YamlData::Sequence(seq) => Some(seq),
        _ => None,
    };
    seq.into_iter().flatten()
}

/// Convert a marked node into a [`serde_json::Value`] for the generated types.
pub fn to_json(node: &MarkedYaml<'_>) -> serde_json::Value {
    use serde_json::Value;
    match &node.data {
        YamlData::Value(scalar) => scalar_to_json(scalar),
        YamlData::Representation(s, _, _) => Value::String(s.to_string()),
        YamlData::Sequence(seq) => Value::Array(seq.iter().map(to_json).collect()),
        YamlData::Mapping(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map.iter() {
                let key = match as_str(k) {
                    Some(s) => s.to_string(),
                    None => scalar_key_to_string(k),
                };
                out.insert(key, to_json(v));
            }
            Value::Object(out)
        }
        YamlData::Tagged(_, inner) => to_json(inner),
        _ => Value::Null,
    }
}

fn scalar_to_json(scalar: &Scalar<'_>) -> serde_json::Value {
    use serde_json::Value;
    match scalar {
        Scalar::Null => Value::Null,
        Scalar::Boolean(b) => Value::Bool(*b),
        Scalar::Integer(i) => Value::Number((*i).into()),
        Scalar::FloatingPoint(f) => serde_json::Number::from_f64(f.into_inner())
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Scalar::String(s) => Value::String(s.to_string()),
    }
}

fn scalar_key_to_string(node: &MarkedYaml<'_>) -> String {
    match &node.data {
        YamlData::Value(Scalar::Boolean(b)) => b.to_string(),
        YamlData::Value(Scalar::Integer(i)) => i.to_string(),
        YamlData::Value(Scalar::FloatingPoint(f)) => f.to_string(),
        _ => String::new(),
    }
}

/// Where a text stops being YAML: its 0-based position and the message,
/// none if it parses.
pub fn syntax_error(text: &str) -> Option<(Position, String)> {
    match MarkedYaml::load_from_str(text) {
        Ok(_) => None,
        Err(err) => {
            let marker = err.marker();
            Some((
                Position {
                    line: marker.line().saturating_sub(1) as u32,
                    character: marker.col() as u32,
                },
                err.info().to_string(),
            ))
        }
    }
}
