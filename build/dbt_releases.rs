//! Which schemas each dbt release writes and reads: the table behind the
//! `dbt-<major>-<minor>` features and `dbt::RELEASES`.
//!
//! Upstream publishes no such table for every family, so it is kept here by
//! hand, and every row says where it was read:
//!
//! - dbt-core 0.19 to 1.11: the `schema_version("<family>", N)` declarations
//!   of dbt-core's own source at the release's tag (`core/dbt/contracts/` up
//!   to 1.7, `core/dbt/artifacts/schemas/` from 1.8). The manifest column
//!   agrees with dbt's documentation, whose table starts at 1.3.
//! - dbt 2.0, the Fusion engine, which dbt-core does not contain: dbt's
//!   documentation (manifest v12; run-results v6, sources v3 and catalog v1
//!   "apply to dbt v2.0 and later"; freshness.json is "Available in v2").
//!
//! The YAML schemas are dbt-jsonschema's directories: none before 1.5,
//! `latest` for 1.8 and later, `latest_fusion` for 2.0. The v0 artifact
//! schemas predate `dbt_schema_version` and belong to no release.
//!
//! A new dbt release is a new row; a new schema version upstream that no row
//! names is still built and released (its `<family>-v<N>` feature), and the
//! upstream bot's pull request says so, for a person to add the row.

/// One dbt release: the schema versions it writes.
pub struct DbtRelease {
    /// `major.minor`: `1.7`, `2.0`.
    pub dbt: &'static str,
    /// The artifact families and the schema version of each.
    pub artifacts: &'static [(&'static str, u32)],
    /// dbt-jsonschema's directory of its YAML schemas, if it has one.
    pub yaml: Option<&'static str>,
    /// Where the row was read.
    pub source: &'static str,
}

const fn core(
    dbt: &'static str,
    artifacts: &'static [(&'static str, u32)],
    yaml: Option<&'static str>,
    source: &'static str,
) -> DbtRelease {
    DbtRelease {
        dbt,
        artifacts,
        yaml,
        source,
    }
}

/// Every dbt release, oldest first.
pub const RELEASES: &[DbtRelease] = &[
    core(
        "0.19",
        &[
            ("manifest", 1),
            ("run-results", 1),
            ("sources", 1),
            ("catalog", 1),
        ],
        None,
        "dbt-core v0.19.0",
    ),
    core(
        "0.20",
        &[
            ("manifest", 2),
            ("run-results", 2),
            ("sources", 1),
            ("catalog", 1),
        ],
        None,
        "dbt-core v0.20.0",
    ),
    core(
        "0.21",
        &[
            ("manifest", 3),
            ("run-results", 3),
            ("sources", 2),
            ("catalog", 1),
        ],
        None,
        "dbt-core v0.21.0",
    ),
    core(
        "1.0",
        &[
            ("manifest", 4),
            ("run-results", 4),
            ("sources", 3),
            ("catalog", 1),
        ],
        None,
        "dbt-core v1.0.0",
    ),
    core(
        "1.1",
        &[
            ("manifest", 5),
            ("run-results", 4),
            ("sources", 3),
            ("catalog", 1),
        ],
        None,
        "dbt-core v1.1.0",
    ),
    core(
        "1.2",
        &[
            ("manifest", 6),
            ("run-results", 4),
            ("sources", 3),
            ("catalog", 1),
        ],
        None,
        "dbt-core v1.2.0",
    ),
    core(
        "1.3",
        &[
            ("manifest", 7),
            ("run-results", 4),
            ("sources", 3),
            ("catalog", 1),
        ],
        None,
        "dbt-core v1.3.0",
    ),
    core(
        "1.4",
        &[
            ("manifest", 8),
            ("run-results", 4),
            ("sources", 3),
            ("catalog", 1),
        ],
        None,
        "dbt-core v1.4.0",
    ),
    core(
        "1.5",
        &[
            ("manifest", 9),
            ("run-results", 4),
            ("sources", 3),
            ("catalog", 1),
        ],
        Some("1.5"),
        "dbt-core v1.5.0",
    ),
    core(
        "1.6",
        &[
            ("manifest", 10),
            ("run-results", 4),
            ("sources", 3),
            ("catalog", 1),
        ],
        Some("1.6"),
        "dbt-core v1.6.0",
    ),
    core(
        "1.7",
        &[
            ("manifest", 11),
            ("run-results", 5),
            ("sources", 3),
            ("catalog", 1),
        ],
        Some("1.7"),
        "dbt-core v1.7.0",
    ),
    core(
        "1.8",
        &[
            ("manifest", 12),
            ("run-results", 6),
            ("sources", 3),
            ("catalog", 1),
        ],
        Some("latest"),
        "dbt-core v1.8.0",
    ),
    core(
        "1.9",
        &[
            ("manifest", 12),
            ("run-results", 6),
            ("sources", 3),
            ("catalog", 1),
        ],
        Some("latest"),
        "dbt-core v1.9.0",
    ),
    core(
        "1.10",
        &[
            ("manifest", 12),
            ("run-results", 6),
            ("sources", 3),
            ("catalog", 1),
        ],
        Some("latest"),
        "dbt-core v1.10.0",
    ),
    core(
        "1.11",
        &[
            ("manifest", 12),
            ("run-results", 6),
            ("sources", 3),
            ("catalog", 1),
        ],
        Some("latest"),
        "dbt-core v1.11.0",
    ),
    core(
        "2.0",
        &[
            ("manifest", 12),
            ("run-results", 6),
            ("sources", 3),
            ("catalog", 1),
            ("freshness", 0),
        ],
        Some("latest_fusion"),
        "docs.getdbt.com (dbt v2, the Fusion engine)",
    ),
];

/// Names for releases, as features: `dbt-latest`, `dbt-fusion`.
pub const ALIASES: &[(&str, &str)] = &[("latest", "1.11"), ("fusion", "2.0")];

/// `1.7` → `dbt-1-7`.
pub fn feature_of(dbt: &str) -> String {
    format!("dbt-{}", dbt.replace('.', "-"))
}

/// The per-schema features a release turns on: each artifact's (a manifest as
/// its projection) and its YAML variant's.
pub fn schema_features(release: &DbtRelease) -> Vec<String> {
    let mut out: Vec<String> = release
        .artifacts
        .iter()
        .map(|(family, version)| match *family {
            "manifest" => format!("manifest-projection-v{version}"),
            _ => format!("{family}-v{version}"),
        })
        .collect();
    if let Some(yaml) = release.yaml {
        out.push(format!("yaml-{}", yaml.replace(['.', '_'], "-")));
    }
    out
}
