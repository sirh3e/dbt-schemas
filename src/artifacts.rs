//! JSON artifact types. The generated shapes are a consumer's input; map
//! them into your own types immediately.
//!
//! Every field is optional and unknown fields pass, so one artifact type
//! reads the files of many dbt versions: the `$id` constants are for
//! tracing which version wrote a file, never for rejecting it.

pub mod run_results {
    pub use crate::generated::run_results::*;
}
pub mod sources {
    pub use crate::generated::sources::*;
}
pub mod catalog {
    pub use crate::generated::catalog::*;
}
pub mod manifest {
    pub use crate::generated::manifest::*;
}

/// The `$id` of each vendored artifact schema.
pub const MANIFEST_SCHEMA_V12: &str = "https://schemas.getdbt.com/dbt/manifest/v12.json";
pub const RUN_RESULTS_SCHEMA_V6: &str = "https://schemas.getdbt.com/dbt/run-results/v6.json";
pub const SOURCES_SCHEMA_V3: &str = "https://schemas.getdbt.com/dbt/sources/v3.json";
pub const CATALOG_SCHEMA_V1: &str = "https://schemas.getdbt.com/dbt/catalog/v1.json";
