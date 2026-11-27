#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# set-version.sh - sets the version of the project, everywhere it is written:
#   VERSION      the reference (read by the C# projects and by the installer)
#   Cargo.toml   [workspace.package] version, shared by the two Rust programs
#   Cargo.lock   the entries of these two programs
#
# Usage: tools/set-version.sh VERSION          (e.g. tools/set-version.sh 5.4.0)
#
# Nothing is committed or tagged: review the change, add the section of the
# release to CHANGELOG.md and CHANGELOG.fr.md, then commit and tag yourself
# (see CONTRIBUTING.md, "Publishing a release").

set -euo pipefail

[ $# -eq 1 ] || { echo "usage: $0 VERSION" >&2; exit 2; }
VERSION="$1"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "error: VERSION must look like 1.2.3" >&2; exit 1; }
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "$VERSION" > VERSION

# Cargo.toml: the version line that follows [workspace.package].
awk -v version="$VERSION" '
    /^\[/ { in_package = ($0 == "[workspace.package]") }
    in_package && /^version = / { print "version = \"" version "\""; next }
    { print }
' Cargo.toml > Cargo.toml.new && mv Cargo.toml.new Cargo.toml

# Cargo.lock: the version line that follows the name of each of our programs.
awk -v version="$VERSION" '
    /^name = "(planche-contact-engine|planche-contact-verb)"$/ { ours = 1; print; next }
    ours && /^version = / { print "version = \"" version "\""; ours = 0; next }
    { ours = 0; print }
' Cargo.lock > Cargo.lock.new && mv Cargo.lock.new Cargo.lock

grep -q "^version = \"$VERSION\"$" Cargo.toml || { echo "error: Cargo.toml was not updated" >&2; exit 1; }
[ "$(grep -c "^version = \"$VERSION\"$" Cargo.lock)" -ge 2 ] || { echo "error: Cargo.lock was not updated" >&2; exit 1; }

echo "Version set to $VERSION in VERSION, Cargo.toml and Cargo.lock."
