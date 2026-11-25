#!/bin/sh
# Regenerate schemas/*/processed from the pristine vendored schemas.
# CI verifies the output is committed: git diff --exit-code -- 'schemas/*/processed'
set -eu
cd "$(dirname "$0")"

for f in dbt_project dbt_yml_files packages selectors; do
    python3 -I preprocess.py "yaml/${f}-latest.json" "yaml/processed/${f}.json"
done

for f in run-results-v6 sources-v3 catalog-v1; do
    python3 -I preprocess.py --mode=artifact "artifacts/${f}.json" "artifacts/processed/${f}.json"
done

python3 -I preprocess.py --mode=manifest-projection \
    artifacts/manifest-v12.json \
    artifacts/processed/manifest-v12-projection.json
