//! The derives of the generated types: `PartialEq` on every one, and `Eq`
//! and `Hash` where no float is inside (`build/derives.rs`, one case each),
//! and on the real types the default features build.

#[path = "../build/derives.rs"]
mod derives;

use std::collections::{BTreeMap, BTreeSet};

use derives::{Part, builtin_hashable, hashable};
use pretty_assertions::assert_eq;
use rstest::rstest;

fn ty(name: &str) -> Part {
    Part::Type(name.to_string())
}

fn builtin(path: &str) -> Part {
    Part::Builtin(path.to_string())
}

fn graph(types: &[(&str, Vec<Part>)]) -> BTreeMap<String, Vec<Part>> {
    types
        .iter()
        .map(|(name, parts)| (name.to_string(), parts.clone()))
        .collect()
}

fn names(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

#[rstest]
#[case::nothing_inside(&[("A", vec![])], &["A"])]
#[case::strings_and_json(&[("A", vec![builtin("::serde_json::Value"), builtin("i64"), builtin("bool")])], &["A"])]
#[case::a_float(&[("A", vec![builtin("f64")])], &[])]
#[case::an_f32(&[("A", vec![builtin("f32")])], &[])]
#[case::a_float_two_levels_down(
    &[("A", vec![ty("B")]), ("B", vec![ty("C")]), ("C", vec![builtin("f64")])],
    &[]
)]
#[case::a_sibling_keeps_it(
    &[("A", vec![ty("B")]), ("B", vec![builtin("i64")]), ("C", vec![builtin("f64")])],
    &["A", "B"]
)]
#[case::recursive_without_a_float(&[("Node", vec![ty("Node"), builtin("i64")])], &["Node"])]
#[case::a_cycle_with_a_float(
    &[("A", vec![ty("B")]), ("B", vec![ty("A"), builtin("f64")])],
    &[]
)]
#[case::a_cycle_and_a_float_beside_it(
    &[("A", vec![ty("B")]), ("B", vec![ty("A")]), ("C", vec![ty("A"), builtin("f64")])],
    &["A", "B"]
)]
// a map of floats: build.rs lists a map's key and value as its parts
#[case::a_map_of_floats(&[("A", vec![builtin("::std::string::String"), builtin("f64")])], &[])]
// an enum's tuple and struct variants are its parts
#[case::an_enum_with_a_number(&[("Version", vec![builtin("::std::string::String"), builtin("f64")])], &[])]
#[case::a_type_no_schema_defines(&[("A", vec![ty("Elsewhere")])], &[])]
fn a_type_is_hashable_when_no_float_is_inside(
    #[case] types: &[(&str, Vec<Part>)],
    #[case] expected: &[&str],
) {
    assert_eq!(hashable(&graph(types)), names(expected));
}

#[rstest]
#[case::f64("f64", false)]
#[case::f32("f32", false)]
#[case::path_and_spaces(" :: f64 ", false)]
#[case::primitive_path(":: std :: primitive :: f32", false)]
#[case::i64("i64", true)]
#[case::nonzero("::std::num::NonZeroU64", true)]
#[case::json("::serde_json::Value", true)]
fn floats_are_the_builtins_without_eq_or_hash(#[case] path: &str, #[case] expected: bool) {
    assert_eq!(builtin_hashable(path), expected);
}

#[cfg(all(feature = "run-results-v6", feature = "manifest-projection-v12"))]
mod generated {
    use std::collections::BTreeMap;
    use std::hash::Hash;

    use dbt_schemas::artifacts::{manifest, run_results};

    fn eq_hash<T: Eq + Hash>() {}
    fn partial_eq<T: PartialEq>() {}

    #[test]
    fn what_holds_no_float_derives_eq_and_hash() {
        eq_hash::<run_results::TimingInfo>();
        eq_hash::<run_results::BaseArtifactMetadata>();
        eq_hash::<run_results::RunResultOutput>();
        eq_hash::<manifest::ManifestProjection>();
        eq_hash::<manifest::ManifestProjectionNode>();
        eq_hash::<manifest::ManifestProjectionMacro>();
    }

    #[test]
    fn what_holds_a_float_derives_partial_eq() {
        // elapsed_time is a number
        partial_eq::<run_results::RunResultsArtifact>();
    }

    #[test]
    fn documents_compare_and_maps_are_ordered() {
        let text = r#"{"nodes": {"model.p.b": {"name": "b"}, "model.p.a": {"name": "a"}},
                       "parent_map": {"model.p.b": ["model.p.a"]}}"#;
        let one: manifest::ManifestProjection = serde_json::from_str(text).unwrap();
        let two: manifest::ManifestProjection = serde_json::from_str(text).unwrap();
        assert_eq!(one, two);
        let ids: Vec<&String> = one.nodes.keys().collect();
        assert_eq!(ids, ["model.p.a", "model.p.b"], "a BTreeMap, in key order");
        let parents: &BTreeMap<String, Vec<String>> = &one.parent_map;
        assert_eq!(parents["model.p.b"], ["model.p.a"]);
        let set: std::collections::HashSet<_> = [one.clone(), two].into_iter().collect();
        assert_eq!(set.len(), 1, "a projection is a key");
    }
}
