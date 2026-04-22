#!/usr/bin/env bash
set -euo pipefail
source "$SHELLOPS_DIR/.envrc"

USAGE="$(cat <<EOF
Updates Cargo.toml and Cargo.lock to match a given version.

Usage: sync_cargo_version.sh <version>

Arguments:
  version     Semver version (e.g., 1.2.3 or 1.0.0-rc.1)

Flags:
  -h, --help  Show this help text
EOF
)"

REGEX_SEMVER='^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z]+(\.[0-9]+)?)?$'
CARGO_TOML="$PROJ/Cargo.toml"
ARG_VERSION=""

function main() {
  parse_args "$@"

  local old_line new_line
  old_line="$(get_package_version)"
  new_line="version = \"$ARG_VERSION\""

  if [[ "$old_line" == "$new_line" ]]; then
    log "Cargo.toml already at version '$ARG_VERSION'"
    exit 0
  fi

  sed -i "s/$old_line/$new_line/" "$CARGO_TOML"
  cargo update --manifest-path "$CARGO_TOML" --package insights --precise "$ARG_VERSION"
  log "Updated Cargo.toml to '$ARG_VERSION'"
}

function get_package_version() {
  local in_package=false

  while IFS= read -r line; do
    if [[ "$line" == "[package]" ]]; then
      in_package=true
    elif [[ "$line" == "["* ]]; then
      in_package=false
    elif $in_package && [[ "$line" == version* ]]; then
      echo "$line"
      return
    fi
  done < "$CARGO_TOML"

  echo 'version = "0.0.0"'
}

function parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -h|--help) log "$USAGE" && exit 0 ;;
      -*)
        log "Unknown option: $1"
        log "$USAGE"
        exit 1
        ;;
      *)
        if [[ ! "$1" =~ $REGEX_SEMVER ]]; then
          log "Invalid semver: $1"
          log "$USAGE"
          exit 1
        fi
        ARG_VERSION="$1"
        ;;
    esac
    shift
  done

  if [[ -z "$ARG_VERSION" ]]; then
    error "version not supplied"
    log "$USAGE"
    exit 1
  fi
}

main "$@"
