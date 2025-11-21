#!/usr/bin/env bash

set -eo pipefail
here="$(dirname "$0")"
src_root="$(readlink -f "${here}/..")"
cd "${src_root}"

source "./scripts/read-cargo-variable.sh"

for cargo_toml in $(git ls-files -- '**/Cargo.toml'); do
  # Read the MSRV from the crate
  minimum_version=$(readCargoVariable rust-version "$cargo_toml" 2>/dev/null)
  # If the crate does not specify a rust-version, fall back to
  # the "program" crate MSRV
  if [[ -z "$minimum_version" ]]; then
    minimum_version=$(readCargoVariable rust-version "program/Cargo.toml")
  fi

  cargo +"$minimum_version" check --manifest-path "$cargo_toml"
done
