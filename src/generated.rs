//! dbt schema types generated at compile time with `typify::import_types!`.
//!
//! The macro reads the **preprocessed** schemas under `schemas/*/processed/`
//! (made by `schemas/preprocess.py` from the pristine vendored files; the
//! typify 0.8 workarounds and the artifacts' leniency rules are documented
//! there). Paths are relative to this crate's `CARGO_MANIFEST_DIR`.

// ---- YAML files (dbt-labs/dbt-jsonschema) ----

pub mod dbt_project {
    typify::import_types!(
        schema = "schemas/yaml/processed/dbt_project.json",
        unknown_crates = Allow,
    );
}

pub mod dbt_yml_files {
    typify::import_types!(
        schema = "schemas/yaml/processed/dbt_yml_files.json",
        unknown_crates = Allow,
    );
}

pub mod packages {
    typify::import_types!(
        schema = "schemas/yaml/processed/packages.json",
        unknown_crates = Allow,
    );
}

pub mod selectors {
    typify::import_types!(
        schema = "schemas/yaml/processed/selectors.json",
        unknown_crates = Allow,
    );
}

// ---- JSON artifacts (dbt-labs/schemas.getdbt.com): read through the
// ---- preprocess --mode=artifact leniency rules; `manifest` compiles the
// ---- documented projection only, never the 946 KB schema.

pub mod run_results {
    typify::import_types!(
        schema = "schemas/artifacts/processed/run-results-v6.json",
        unknown_crates = Allow,
    );
}

pub mod sources {
    typify::import_types!(
        schema = "schemas/artifacts/processed/sources-v3.json",
        unknown_crates = Allow,
    );
}

pub mod catalog {
    typify::import_types!(
        schema = "schemas/artifacts/processed/catalog-v1.json",
        unknown_crates = Allow,
    );
}

pub mod manifest {
    typify::import_types!(
        schema = "schemas/artifacts/processed/manifest-v12-projection.json",
        unknown_crates = Allow,
    );
}
