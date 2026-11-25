//! Preprocess the upstream dbt schemas and generate their Rust types.
//!
//! The schemas are the two upstream repositories, checked out as submodules
//! under `schemas/upstream/`; `build/catalog.rs` lists every schema they hold
//! as a module and the feature that builds it. For each module whose feature
//! is on, this writes to `OUT_DIR`:
//!
//! - `<stem>.json`: the schema after `build/preprocess.rs`'s rules;
//! - `<stem>.rs`: the types typify generates from it — the same calls
//!   `typify::import_types!` makes, with `unknown_crates = Allow`;
//! - with the `avro` feature, `<stem>.avsc`: its Avro schema (`build/avro.rs`);
//!
//! and once `index.rs`: the module tree, each artifact family's newest
//! version re-exported at its root, and the version dispatch (`Any<Family>`),
//! which `src/generated.rs` includes.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use typify::{TypeSpace, TypeSpaceSettings, UnknownPolicy};

#[path = "build/avro.rs"]
mod avro;
#[path = "build/catalog.rs"]
mod catalog;
#[path = "build/dbt_releases.rs"]
mod dbt_releases;
#[path = "build/preprocess.rs"]
mod preprocess;

use catalog::{Entry, Kind};

/// Schemas typify cannot read, by stem, with why. None today: every variant
/// and version upstream holds builds. A schema listed here gets no module
/// and the build says so; a new schema that fails is never skipped silently,
/// it fails the build until it is fixed or listed.
const UNSUPPORTED: &[(&str, &str)] = &[];

/// The artifact families whose newest version is re-exported at the
/// family's root (`artifacts::run_results::RunResultsArtifact`).
const NEWEST_AT_ROOT: &[&str] = &["run-results", "sources", "catalog", "freshness"];

fn main() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=build");
    println!("cargo::rerun-if-changed={}", catalog::YAML_ROOT);
    println!("cargo::rerun-if-changed={}", catalog::ARTIFACT_ROOT);
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let entries = catalog::discover(crate_dir);
    let mut built = Vec::new();
    for entry in &entries {
        if !enabled(&entry.feature) {
            continue;
        }
        if let Some((_, why)) = UNSUPPORTED.iter().find(|(stem, _)| *stem == entry.stem()) {
            println!(
                "cargo::warning={} is not built: {why}",
                entry.path.display()
            );
            continue;
        }
        let text = std::fs::read_to_string(crate_dir.join(&entry.path))
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", entry.path.display()));
        let processed = match entry.kind {
            Kind::ManifestProjection => {
                let version = entry.version.expect("a manifest has a version");
                let strict = Some(version) == catalog::newest(&entries, "manifest");
                preprocess::manifest_projection(&text, version, strict)
            }
            kind => {
                let mut value: serde_json::Value = serde_json::from_str(&text)
                    .unwrap_or_else(|e| panic!("{} is not JSON: {e}", entry.path.display()));
                let mode = match kind {
                    Kind::Yaml => preprocess::Mode::Yaml,
                    _ => preprocess::Mode::Artifact,
                };
                preprocess::preprocess(&mut value, mode);
                name_the_root(&mut value, entry);
                value
            }
        };
        let json = serde_json::to_string_pretty(&processed).expect("a value serializes");
        let (code, root) = generate(entry, &json);
        write(&out, &format!("{}.json", entry.stem()), &json);
        write(&out, &format!("{}.rs", entry.stem()), &code);
        if enabled("avro") {
            write(
                &out,
                &format!("{}.avsc", entry.stem()),
                &avsc(&processed, entry),
            );
        }
        built.push((entry.clone(), root));
    }
    write(&out, "index.rs", &index(&entries, &built));
}

/// A root without a `title` (the v0 artifacts, some v1s) gets no named type
/// from typify, so nothing could name it: it is called `<Family>Artifact`
/// (`CatalogArtifact`) or, for a YAML file, after the file (`DbtProject`).
/// A titled root is left as it is.
fn name_the_root(value: &mut serde_json::Value, entry: &Entry) {
    if let Some(root) = value.as_object_mut()
        && !root.contains_key("title")
    {
        let title = match entry.kind {
            Kind::Yaml => pascal(&entry.name),
            _ => format!("{}Artifact", pascal(&entry.family)),
        };
        root.insert("title".into(), title.into());
    }
}

/// The Avro schema of a processed schema, its named types in the module's
/// namespace (`dbt_schemas.run_results.v6`).
fn avsc(processed: &serde_json::Value, entry: &Entry) -> String {
    let namespace = std::iter::once("dbt_schemas")
        .chain(entry.group.split('/'))
        .chain(std::iter::once(entry.name.as_str()))
        .map(avro::avro_name)
        .collect::<Vec<_>>()
        .join(".");
    serde_json::to_string_pretty(&avro::avro_schema(processed, &namespace))
        .expect("a value serializes")
}

/// Whether cargo enabled `feature` for this build.
fn enabled(feature: &str) -> bool {
    let var = format!("CARGO_FEATURE_{}", feature.to_uppercase().replace('-', "_"));
    std::env::var_os(var).is_some()
}

fn write(out: &Path, name: &str, text: &str) {
    std::fs::write(out.join(name), text).expect("OUT_DIR is writable");
}

/// typify's types of one processed schema, and the name of its root type.
fn generate(entry: &Entry, processed: &str) -> (String, Option<String>) {
    let root: schemars::schema::RootSchema = serde_json::from_str(processed).unwrap_or_else(|e| {
        panic!(
            "the processed {} is no JSON Schema: {e}",
            entry.path.display()
        )
    });
    let mut settings = TypeSpaceSettings::default();
    settings.with_unknown_crates(UnknownPolicy::Allow);
    let mut types = TypeSpace::new(&settings);
    let id = types.add_root_schema(root).unwrap_or_else(|e| {
        panic!(
            "typify cannot read {}: {e} — fix build/preprocess.rs, or list it in \
             UNSUPPORTED of build.rs with why",
            entry.path.display()
        )
    });
    let name = id.and_then(|id| types.get_type(&id).ok().map(|t| t.name()));
    (types.to_stream().to_string(), name)
}

/// A module tree: path segments to their children, leaves to their files.
#[derive(Default)]
struct Tree {
    children: BTreeMap<String, Tree>,
    leaves: Vec<(String, String, Entry)>,
    glob: Option<String>,
}

fn index(entries: &[Entry], built: &[(Entry, Option<String>)]) -> String {
    let mut tree = Tree::default();
    for (entry, _) in built {
        let mut node = &mut tree;
        for segment in entry.group.split('/') {
            node = node.children.entry(segment.to_string()).or_default();
        }
        node.leaves
            .push((entry.name.clone(), entry.stem(), entry.clone()));
    }
    // each family's newest version this build has, at its root: a version
    // upstream adds that the features leave off moves no root away
    for family in NEWEST_AT_ROOT {
        let module = catalog::module_of(family);
        if let Some(node) = tree.children.get_mut(&module)
            && let Some(version) = newest_leaf(node)
        {
            node.glob = Some(format!("v{version}"));
        }
    }
    // a manifest's root is its newest projection built, as it has always been
    if let Some(manifest) = tree.children.get_mut("manifest")
        && let Some(version) = manifest.children.get("projection").and_then(newest_leaf)
    {
        manifest.glob = Some(format!("projection::v{version}"));
    }

    let mut out = String::from("// @generated by build.rs from build/catalog.rs\n\n");
    render(&tree, &mut out, 0);
    out.push_str(&dispatch(entries, built));
    out.push_str(&artifact_reexports(&tree, built));
    out.push_str(&processed_table(built));
    if enabled("avro") {
        out.push_str(&avro_table(built));
    }
    out.push_str(&schema_ids(entries));
    out.push_str(&releases());
    out
}

/// The newest `v<N>` module of a family's node.
fn newest_leaf(node: &Tree) -> Option<u32> {
    node.leaves
        .iter()
        .filter_map(|(name, ..)| name.strip_prefix('v')?.parse().ok())
        .max()
}

fn render(tree: &Tree, out: &mut String, depth: usize) {
    let pad = "    ".repeat(depth);
    if let Some(glob) = &tree.glob {
        let _ = writeln!(out, "{pad}pub use self::{glob}::*;");
    }
    for (name, stem, entry) in &tree.leaves {
        let _ = writeln!(
            out,
            "{pad}/// Generated from `{}` (feature `{}`).\n\
             {pad}pub mod {name} {{\n\
             {pad}    include!(concat!(env!(\"OUT_DIR\"), \"/{stem}.rs\"));\n\
             {pad}}}",
            entry.path.display(),
            entry.feature,
        );
    }
    for (name, child) in &tree.children {
        let _ = writeln!(out, "{pad}pub mod {name} {{");
        render(child, out, depth + 1);
        let _ = writeln!(out, "{pad}}}");
    }
}

/// `Any<Family>`: one variant per version built, chosen by the document's
/// `metadata.dbt_schema_version`.
fn dispatch(entries: &[Entry], built: &[(Entry, Option<String>)]) -> String {
    // (family, projection) → (version, root type's path, feature)
    type Versions = Vec<(u32, String, String)>;
    let mut families: BTreeMap<(String, bool), Versions> = BTreeMap::new();
    for (entry, root) in built {
        let (Some(version), Some(root)) = (entry.version, root) else {
            continue;
        };
        let projection = entry.kind == Kind::ManifestProjection;
        let path = format!("{}::{}::{root}", entry.group.replace('/', "::"), entry.name);
        families
            .entry((entry.family.clone(), projection))
            .or_default()
            .push((version, path, entry.feature.clone()));
    }
    let mut out = String::new();
    for ((family, projection), mut versions) in families {
        versions.sort();
        let name = format!(
            "Any{}{}",
            pascal(&family),
            if projection { "Projection" } else { "" }
        );
        let known: Vec<String> = entries
            .iter()
            .filter(|e| e.family == family)
            .filter_map(|e| e.version)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|v| v.to_string())
            .collect();
        let list = versions
            .iter()
            .map(|(v, ..)| v.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            out,
            "\n/// A `{family}` artifact of any version this build reads ({what}), chosen by\n\
             /// its `metadata.dbt_schema_version`. Upstream has versions {known}.\n\
             #[derive(Debug, Clone)]\n#[non_exhaustive]\npub enum {name} {{",
            what = if projection {
                "as projections"
            } else {
                "complete"
            },
            known = known.join(", "),
        );
        for (version, path, feature) in &versions {
            let _ = writeln!(
                out,
                "    /// Version {version} (feature `{feature}`).\n    V{version}(Box<{path}>),"
            );
        }
        let _ = writeln!(out, "}}\n\nimpl {name} {{");
        let _ = writeln!(
            out,
            "    /// The family as upstream names it.\n    pub const FAMILY: &'static str = \"{family}\";"
        );
        let _ = writeln!(
            out,
            "    /// The versions this build reads.\n    pub const VERSIONS: &'static [u32] = &[{list}];\n"
        );
        let _ = writeln!(
            out,
            "    /// The version the document was read as.\n    pub fn version(&self) -> u32 {{\n        match self {{"
        );
        for (version, ..) in &versions {
            let _ = writeln!(out, "            Self::V{version}(_) => {version},");
        }
        let _ = writeln!(out, "        }}\n    }}\n");
        let _ = writeln!(
            out,
            "    /// Read a document as the version its `metadata.dbt_schema_version` names.\n    \
             pub fn from_value(value: ::serde_json::Value) -> ::std::result::Result<Self, crate::artifacts::DispatchError> {{\n        \
             let version = crate::artifacts::sniff_family(&value, Self::FAMILY)?;\n        match version {{"
        );
        for (version, ..) in &versions {
            let _ = writeln!(
                out,
                "            {version} => ::serde_json::from_value(value)\n                \
                 .map(|d| Self::V{version}(Box::new(d)))\n                \
                 .map_err(crate::artifacts::DispatchError::Shape),"
            );
        }
        let _ = writeln!(
            out,
            "            other => Err(crate::artifacts::DispatchError::NotBuilt {{\n                \
             family: Self::FAMILY,\n                version: other,\n            }}),\n        \
             }}\n    }}\n}}"
        );
    }
    out
}

/// `__artifacts`: the artifact families and dispatch enums this build has,
/// which `crate::artifacts` re-exports whatever the features are.
fn artifact_reexports(tree: &Tree, built: &[(Entry, Option<String>)]) -> String {
    let mut names: Vec<String> = tree
        .children
        .keys()
        .filter(|name| *name != "yaml")
        .cloned()
        .collect();
    let mut families: Vec<(String, bool)> = built
        .iter()
        .filter(|(e, root)| e.version.is_some() && root.is_some())
        .map(|(e, _)| (e.family.clone(), e.kind == Kind::ManifestProjection))
        .collect();
    families.sort();
    families.dedup();
    for (family, projection) in families {
        names.push(format!(
            "Any{}{}",
            pascal(&family),
            if projection { "Projection" } else { "" }
        ));
    }
    let mut out = String::from("\n#[doc(hidden)]\npub mod __artifacts {\n");
    for name in names {
        let _ = writeln!(out, "    pub use super::{name};");
    }
    out.push_str("}\n");
    out
}

/// Every processed schema this build has, for what reads schemas themselves.
fn processed_table(built: &[(Entry, Option<String>)]) -> String {
    let mut out = String::from(
        "\n/// Every processed schema this build has: its module path (`yaml/latest/dbt_project`,\n\
         /// `manifest/projection/v12`) and its JSON text.\n\
         pub const PROCESSED: &[(&str, &str)] = &[\n",
    );
    for (entry, _) in built {
        let _ = writeln!(
            out,
            "    (\"{}/{}\", include_str!(concat!(env!(\"OUT_DIR\"), \"/{}.json\"))),",
            entry.group,
            entry.name,
            entry.stem()
        );
    }
    out.push_str("];\n");
    out
}

/// Every Avro schema this build has, by module path, as `processed_table`.
fn avro_table(built: &[(Entry, Option<String>)]) -> String {
    let rows: String = built
        .iter()
        .map(|(entry, _)| {
            format!(
                "    (\"{}/{}\", include_str!(concat!(env!(\"OUT_DIR\"), \"/{}.avsc\"))),\n",
                entry.group,
                entry.name,
                entry.stem()
            )
        })
        .collect();
    format!(
        "\n/// Every Avro schema this build has: its module path and its `.avsc` text.\n\
         pub const AVRO: &[(&str, &str)] = &[\n{rows}];\n"
    )
}

/// The `$id` of every artifact schema upstream holds, built or not.
fn schema_ids(entries: &[Entry]) -> String {
    let mut ids: Vec<String> = entries
        .iter()
        .filter(|e| e.kind == Kind::Artifact)
        .map(|e| {
            format!(
                "https://schemas.getdbt.com/dbt/{}/v{}.json",
                e.family,
                e.version.unwrap()
            )
        })
        .collect();
    ids.sort();
    ids.dedup();
    let mut out = String::from(
        "\n/// The `$id` of every artifact schema upstream holds, whether this build has it or not.\n\
         pub const SCHEMA_IDS: &[&str] = &[\n",
    );
    for id in ids {
        let _ = writeln!(out, "    \"{id}\",");
    }
    out.push_str("];\n");
    out
}

/// `__dbt::RELEASES`: the dbt releases of `build/dbt_releases.rs`, each with
/// its features and whether this build has all of them.
fn releases() -> String {
    let mut out = String::from(
        "\n#[doc(hidden)]\npub mod __dbt {\n    pub const RELEASES: &[crate::dbt::DbtRelease] = &[\n",
    );
    for release in dbt_releases::RELEASES {
        let features = dbt_releases::schema_features(release);
        let built = features.iter().all(|f| enabled(f));
        let artifacts: Vec<String> = release
            .artifacts
            .iter()
            .map(|(family, version)| format!("(\"{family}\", {version})"))
            .collect();
        let quoted: Vec<String> = features.iter().map(|f| format!("\"{f}\"")).collect();
        let yaml = match release.yaml {
            Some(y) => format!("Some(\"{y}\")"),
            None => "None".into(),
        };
        let _ = writeln!(
            out,
            "        crate::dbt::DbtRelease {{ dbt: \"{}\", feature: \"{}\", artifacts: &[{}], yaml: {yaml}, features: &[{}], built: {built}, source: \"{}\" }},",
            release.dbt,
            dbt_releases::feature_of(release.dbt),
            artifacts.join(", "),
            quoted.join(", "),
            release.source,
        );
    }
    out.push_str("    ];\n    pub const ALIASES: &[(&str, &str)] = &[");
    for (alias, dbt) in dbt_releases::ALIASES {
        let _ = write!(out, "(\"{alias}\", \"{dbt}\"), ");
    }
    out.push_str("];\n}\n");
    out
}

fn pascal(name: &str) -> String {
    name.split(['-', '_'])
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}
