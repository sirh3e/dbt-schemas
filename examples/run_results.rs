//! Read a `run_results.json` of whichever version wrote it, and count its
//! results by status.
//!
//!     cargo run --example run_results                       # a built-in sample
//!     cargo run --example run_results -- target/run_results.json
//!
//! [`AnyRunResults::from_value`] reads the document as the schema its
//! `metadata.dbt_schema_version` names, so one program reads the files of
//! many dbt releases; [`artifacts::dbt_releases`] says which releases those are.

use std::collections::BTreeMap;

use dbt_schemas::artifacts::{self, AnyRunResults, DispatchError};
use serde_json::Value;

const SAMPLE: &str = r#"{
  "metadata": {
    "dbt_schema_version": "https://schemas.getdbt.com/dbt/run-results/v6.json",
    "dbt_version": "1.8.7"
  },
  "results": [
    {"unique_id": "model.shop.orders", "status": "success", "execution_time": 1.2},
    {"unique_id": "model.shop.customers", "status": "success", "execution_time": 0.4},
    {"unique_id": "test.shop.not_null_orders_id", "status": "fail", "execution_time": 0.1}
  ]
}"#;

/// The statuses of a run's results, as `(unique id, status)`.
fn statuses(document: AnyRunResults) -> Result<Vec<(String, String)>, String> {
    let named = |id: Option<String>, status: Option<String>| {
        (
            id.unwrap_or_default(),
            status.unwrap_or_else(|| "unknown".into()),
        )
    };
    match document {
        AnyRunResults::V6(run) => Ok(run
            .results
            .into_iter()
            .map(|r| named(r.unique_id, r.status))
            .collect()),
        // The enum is non_exhaustive and has a variant per version the build
        // has: turn on `run-results-v5` (or `dbt-1-7`) and add its arm.
        other => Err(format!(
            "run results v{} are not summarized here",
            other.version()
        )),
    }
}

/// How many results have each status, in name order.
fn count_by_status(results: &[(String, String)]) -> BTreeMap<&str, usize> {
    results
        .iter()
        .fold(BTreeMap::new(), |mut counts, (_, status)| {
            *counts.entry(status.as_str()).or_default() += 1;
            counts
        })
}

fn summarize(value: Value) -> Result<String, Box<dyn std::error::Error>> {
    let releases = artifacts::dbt_releases(&value)
        .into_iter()
        .map(|release| release.dbt)
        .collect::<Vec<_>>()
        .join(", ");
    let document = AnyRunResults::from_value(value).map_err(|e: DispatchError| e.to_string())?;
    let version = document.version();
    let results = statuses(document)?;
    let counts = count_by_status(&results)
        .into_iter()
        .map(|(status, n)| format!("{n} {status}"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "run results v{version} (dbt {releases}): {} results, {counts}",
        results.len()
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = match std::env::args().nth(1) {
        Some(path) => std::fs::read_to_string(path)?,
        None => SAMPLE.to_string(),
    };
    println!("{}", summarize(serde_json::from_str(&text)?)?);
    Ok(())
}
