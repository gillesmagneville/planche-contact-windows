#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# release-notes.sh - prints the notes of a release: its section of
# CHANGELOG.md (English, first), then its section of CHANGELOG.fr.md.
#
# Usage: tools/release-notes.sh VERSION        (e.g. tools/release-notes.sh 5.3.0)
#
# A section starts at the "## " heading that ends with "vVERSION" (as in
# "## 2026-11-27 — v5.3.0") and stops at the next "## " heading.

set -euo pipefail

[ $# -eq 1 ] || { echo "usage: $0 VERSION" >&2; exit 2; }
VERSION="$1"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

section() {
    awk -v version="v$1" '
        /^## / {
            if (printing) exit
            n = split($0, words, " ")
            if (words[n] == version) { printing = 1; next }
        }
        printing { print }
    ' "$2" | sed -e '/./,$!d' | sed -e ':a' -e '/^\n*$/{$d;N;ba' -e '}'
}

english="$(section "$VERSION" "$ROOT/CHANGELOG.md")"
french="$(section "$VERSION" "$ROOT/CHANGELOG.fr.md")"
[ -n "$english" ] || { echo "error: no section for v$VERSION in CHANGELOG.md" >&2; exit 1; }
[ -n "$french" ] || { echo "error: no section for v$VERSION in CHANGELOG.fr.md" >&2; exit 1; }

printf '%s\n\n---\n\n## 🇫🇷 Français\n\n%s\n' "$english" "$french"
