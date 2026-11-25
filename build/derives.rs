//! Which generated types can derive `Eq` and `Hash`.
//!
//! Every generated type derives `PartialEq`: every field type has it. `Eq`
//! and `Hash` need the same of every value a type holds, at any depth, and a
//! float has neither. Maps are generated as `BTreeMap` (which has both), and
//! `serde_json::Value` and `Map` have both, so a type can derive them exactly
//! when no float is anywhere inside it.
//!
//! The decision is plain data, a type's name to the parts it holds, so it is
//! tested without typify; `build.rs` reads the graph out of typify's types.

use std::collections::{BTreeMap, BTreeSet};

/// What a type holds, one level down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    /// Another generated type, by name.
    Type(String),
    /// A type typify names itself (`f64`, `::serde_json::Value`, `i64`, …).
    Builtin(String),
}

/// Whether a builtin has `Eq` and `Hash`: everything typify emits but floats.
/// A path is read without its spaces, as a token stream may write it
/// (`:: std :: primitive :: f64`), and by its last segment.
pub fn builtin_hashable(path: &str) -> bool {
    let path: String = path.split_whitespace().collect();
    let last = path.rsplit("::").next().unwrap_or(&path);
    !matches!(last, "f32" | "f64")
}

/// The types that can derive `Eq` and `Hash`: the largest set closed under
/// "every part is a hashable builtin or a type in the set". It starts from
/// every type and drops what holds a float, or a type already dropped, until
/// nothing more drops; a recursive type stays unless a float is on its cycle.
pub fn hashable(graph: &BTreeMap<String, Vec<Part>>) -> BTreeSet<String> {
    let everything: BTreeSet<String> = graph.keys().cloned().collect();
    std::iter::successors(Some(everything), |kept| {
        let next = keep(graph, kept);
        (next != *kept).then_some(next)
    })
    .last()
    .expect("successors yields its first")
}

/// One round: the types of `kept` whose every part is still fine.
fn keep(graph: &BTreeMap<String, Vec<Part>>, kept: &BTreeSet<String>) -> BTreeSet<String> {
    kept.iter()
        .filter(|name| {
            graph[*name].iter().all(|part| match part {
                Part::Builtin(path) => builtin_hashable(path),
                // a name the graph does not hold is no type of this schema:
                // nothing is known of it, so it is not counted on
                Part::Type(other) => kept.contains(other),
            })
        })
        .cloned()
        .collect()
}
