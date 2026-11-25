//! Which schemas a dbt release writes, and whether this build has them.
//!
//!     cargo run --example dbt_release                      # 1.7.14, 2.0 and latest
//!     cargo run --example dbt_release -- 1.5.0 fusion
//!
//! [`dbt::release`] reads a dbt version as a project names it (`1.7.14`,
//! `v2.0.0-beta.1`, `latest`); its `feature` is the Cargo feature that
//! builds everything that release writes.

use dbt_schemas::dbt::{self, DbtRelease};

fn describe(release: &DbtRelease) -> String {
    let artifacts = release
        .artifacts
        .iter()
        .map(|(family, version)| format!("{family} v{version}"))
        .collect::<Vec<_>>()
        .join(", ");
    let yaml = release
        .yaml
        .map(|variant| format!("; YAML schemas {variant}"))
        .unwrap_or_default();
    let built = if release.built {
        "built"
    } else {
        "not in this build"
    };
    format!(
        "dbt {}: {artifacts}{yaml} (feature `{}`, {built})",
        release.dbt, release.feature
    )
}

fn answer(asked: &str) -> String {
    dbt::release(asked)
        .map(describe)
        .unwrap_or_else(|| format!("{asked}: a dbt release this crate does not know"))
}

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let asked = if asked.is_empty() {
        ["1.7.14", "2.0", "latest"].map(String::from).to_vec()
    } else {
        asked
    };
    asked
        .iter()
        .map(|v| answer(v))
        .for_each(|line| println!("{line}"));

    // The other way round: which releases write manifest v12?
    let writers = dbt::releases_of("manifest", 12)
        .map(|release| release.dbt)
        .collect::<Vec<_>>()
        .join(", ");
    println!("manifest v12 is written by dbt {writers}");
}
