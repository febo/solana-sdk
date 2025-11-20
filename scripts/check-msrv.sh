#!/usr/bin/env bash

set -eo pipefail
here="$(dirname "$0")"
src_root="$(readlink -f "${here}/..")"
cd "${src_root}"

source "./scripts/read-cargo-variable.sh"

find "$src_root" -type d -print0 | while read -r -d '' dir; do
  crate="${dir##*/}"
  # Skip directories that do not contain a Cargo.toml
  if [[ -e "$crate/Cargo.toml" ]]; then
    # Read the MSRV from the crate
    minimum_version=$(readCargoVariable rust-version "$crate/Cargo.toml" 2>/dev/null)

    # If the crate does not specify a rust-version, fall back to
    # the "program" crate MSRV
    if [[ -z "$minimum_version" ]]; then
      minimum_version=$(readCargoVariable rust-version "program/Cargo.toml")
    fi

    cargo +$minimum_version check
  fi
done
