//! Preprocess the vendored dbt schemas and generate their Rust types.
//!
//! For each pristine schema under `schemas/` this writes two files to
//! `OUT_DIR`: `<name>.json`, the schema after `build/preprocess.rs`'s rules,
//! and `<name>.rs`, the types typify generates from it — the same calls
//! `typify::import_types!` makes, with `unknown_crates = Allow`.
//! `src/generated.rs` and `src/processed.rs` include them.

use std::path::{Path, PathBuf};

use typify::{TypeSpace, TypeSpaceSettings, UnknownPolicy};

#[path = "build/preprocess.rs"]
mod preprocess;

use preprocess::Mode;

/// The schemas the crate compiles: output name, pristine file, how.
enum Source {
    Schema(&'static str, Mode),
    ManifestProjection(&'static str),
}

const SCHEMAS: &[(&str, Source)] = &[
    (
        "dbt_project",
        Source::Schema("yaml/dbt_project-latest.json", Mode::Yaml),
    ),
    (
        "dbt_yml_files",
        Source::Schema("yaml/dbt_yml_files-latest.json", Mode::Yaml),
    ),
    (
        "packages",
        Source::Schema("yaml/packages-latest.json", Mode::Yaml),
    ),
    (
        "selectors",
        Source::Schema("yaml/selectors-latest.json", Mode::Yaml),
    ),
    (
        "run_results",
        Source::Schema("artifacts/run-results-v6.json", Mode::Artifact),
    ),
    (
        "sources",
        Source::Schema("artifacts/sources-v3.json", Mode::Artifact),
    ),
    (
        "catalog",
        Source::Schema("artifacts/catalog-v1.json", Mode::Artifact),
    ),
    (
        "manifest",
        Source::ManifestProjection("artifacts/manifest-v12.json"),
    ),
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=build/preprocess.rs");
    let schemas = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    for (name, source) in SCHEMAS {
        let (file, processed) = match source {
            Source::Schema(file, mode) => {
                let mut value: serde_json::Value = serde_json::from_str(&read(&schemas, file))
                    .unwrap_or_else(|e| panic!("schemas/{file} is not JSON: {e}"));
                preprocess::preprocess(&mut value, *mode);
                (file, value)
            }
            Source::ManifestProjection(file) => {
                (file, preprocess::manifest_projection(&read(&schemas, file)))
            }
        };
        let text = serde_json::to_string_pretty(&processed).expect("a value serializes");
        std::fs::write(out.join(format!("{name}.json")), &text).expect("OUT_DIR is writable");
        std::fs::write(out.join(format!("{name}.rs")), generate(name, file, &text))
            .expect("OUT_DIR is writable");
    }
}

fn read(schemas: &Path, file: &str) -> String {
    let path = schemas.join(file);
    println!("cargo::rerun-if-changed={}", path.display());
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read schemas/{file}: {e}"))
}

fn generate(name: &str, file: &str, processed: &str) -> String {
    let root: schemars::schema::RootSchema = serde_json::from_str(processed)
        .unwrap_or_else(|e| panic!("the processed {name} schema is no JSON Schema: {e}"));
    let mut settings = TypeSpaceSettings::default();
    settings.with_unknown_crates(UnknownPolicy::Allow);
    let mut types = TypeSpace::new(&settings);
    types
        .add_root_schema(root)
        .unwrap_or_else(|e| panic!("typify cannot read the schema of schemas/{file}: {e}"));
    types.to_stream().to_string()
}
