#!/usr/bin/env bash
# Runs every quality gate for the crate. Add --mutation to also run the (slower) mutation check.
set -euo pipefail
cd "$(dirname "$0")"
echo "== fmt ==";    cargo fmt --check
echo "== clippy =="; cargo clippy --all-targets -- -D warnings
echo "== tests ==";  cargo test --quiet
echo "== docs ==";   RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --quiet
if [ "${1:-}" = "--mutation" ]; then echo "== mutation check =="; python tools/mutation_check.py; fi
echo "ALL CHECKS PASSED"
