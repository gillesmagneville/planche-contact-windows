#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
#
# build-installer.sh - builds the Windows installer of Planche-Contact, on
# Linux.
#
# The project is made of three programs:
#   * PlancheContact.exe                   the window (C#, WinUI 3)
#   * engine\planche-contact-engine.exe    the engine (Rust)
#   * planche-contact-verb.exe             the File Explorer menu handler (Rust)
# plus the manual, in docs\. The two Rust programs are cross-compiled here.
# The window can only be built on Windows: this script takes it already
# built, as a folder (see CONTRIBUTING.md).
#
# Usage:
#   ./build-installer.sh --app DIR [--no-rust-build]
#       DIR: the published window, i.e. the output of "dotnet publish" on a
#       Windows machine. Builds the Rust programs (unless --no-rust-build:
#       they are then taken as already built in target/), gathers
#       everything in dist/planche-contact/, then builds the installer.
#   ./build-installer.sh --dist DIR
#       DIR: a folder that already holds everything, such as the
#       "planche-contact-folder" artifact of the GitHub Actions workflow.
#       Only builds the installer (this is what the CI does).
#
# Result, in dist/:
#   planche-contact-VERSION-setup.exe
#   planche-contact-VERSION-setup.exe.sha256
#
# Requirements (Ubuntu/Debian):
#   sudo apt-get install nsis gcc-mingw-w64-x86-64-posix g++-mingw-w64-x86-64-posix
#   rustup target add x86_64-pc-windows-gnu

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="x86_64-pc-windows-gnu"
APP_EXE="PlancheContact.exe"
# The resource index of the window: it does not start without it (see
# app/PlancheContact/PublishWinUIResources.targets).
APP_PRI="PlancheContact.pri"
ENGINE_EXE="engine/planche-contact-engine.exe"
VERB_EXE="planche-contact-verb.exe"

die() { echo "error: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "'$1' not found — $2"; }

usage() { sed -n '3,34p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

APP_DIR=""
DIST_DIR=""
BUILD_RUST=1
while [ $# -gt 0 ]; do
    case "$1" in
        --app) [ $# -ge 2 ] || die "--app needs a folder"; APP_DIR="$2"; shift 2 ;;
        --dist) [ $# -ge 2 ] || die "--dist needs a folder"; DIST_DIR="$2"; shift 2 ;;
        --no-rust-build) BUILD_RUST=0; shift ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown argument: $1 (see --help)" ;;
    esac
done
if [ -n "$APP_DIR" ] && [ -n "$DIST_DIR" ]; then die "--app and --dist are exclusive"; fi
if [ -z "$APP_DIR" ] && [ -z "$DIST_DIR" ]; then usage; exit 2; fi

VERSION="$(tr -d '[:space:]' < "$ROOT/VERSION")"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "VERSION must look like 1.2.3 (found: '$VERSION')"

need makensis "sudo apt-get install nsis"
need sha256sum "it is part of coreutils"

if [ -n "$APP_DIR" ]; then
    [ -f "$APP_DIR/$APP_EXE" ] || die "$APP_DIR does not hold $APP_EXE: not a published window"
    [ -f "$APP_DIR/$APP_PRI" ] || die "$APP_DIR does not hold $APP_PRI: the window would not start (publish it again with the current sources)"

    if [ "$BUILD_RUST" = 1 ]; then
        need cargo "install Rust (https://rustup.rs)"
        need x86_64-w64-mingw32-g++-posix "sudo apt-get install gcc-mingw-w64-x86-64-posix g++-mingw-w64-x86-64-posix"
        echo "==> Building the Rust programs for Windows"
        (cd "$ROOT" && cargo build --release --locked --target "$TARGET" \
            -p planche-contact-engine -p planche-contact-verb)
    fi
    RUST_OUT="$ROOT/target/$TARGET/release"
    for exe in planche-contact-engine.exe "$VERB_EXE"; do
        [ -f "$RUST_OUT/$exe" ] || die "$RUST_OUT/$exe not found: build the Rust programs first"
    done

    DIST_DIR="$ROOT/dist/planche-contact"
    echo "==> Gathering the programs in $DIST_DIR"
    rm -rf "$DIST_DIR"
    mkdir -p "$DIST_DIR/engine"
    cp -a "$APP_DIR/." "$DIST_DIR/"
    cp "$RUST_OUT/planche-contact-engine.exe" "$DIST_DIR/engine/"
    cp "$RUST_OUT/$VERB_EXE" "$DIST_DIR/"
    if compgen -G "$ROOT/docs/planche-contact-manual*.html" >/dev/null; then
        mkdir -p "$DIST_DIR/docs"
        cp "$ROOT"/docs/planche-contact-manual*.html "$DIST_DIR/docs/"
    fi
fi

for file in "$APP_EXE" "$APP_PRI" "$ENGINE_EXE" "$VERB_EXE"; do
    [ -f "$DIST_DIR/$file" ] || die "$DIST_DIR does not hold $file"
done

# The Rust programs must run on a Windows that has no MinGW installed: fail
# here rather than on the user's machine if one of them needs a MinGW DLL.
if command -v x86_64-w64-mingw32-objdump >/dev/null 2>&1; then
    for exe in "$ENGINE_EXE" "$VERB_EXE"; do
        if x86_64-w64-mingw32-objdump -p "$DIST_DIR/$exe" | grep -i 'DLL Name' \
            | grep -i -E 'libstdc\+\+|libgcc|libwinpthread'; then
            die "$exe depends on a MinGW DLL (see above): it must be linked statically"
        fi
    done
fi

OUT_DIR="$ROOT/dist"
mkdir -p "$OUT_DIR"
INSTALLER="planche-contact-$VERSION-setup.exe"

echo "==> Building $INSTALLER"
# -V2: only warnings and errors. Absolute paths: makensis resolves relative
# ones from the folder of the script.
makensis -V2 \
    "-DVERSION=$VERSION" \
    "-DDIST=$(cd "$DIST_DIR" && pwd)" \
    "-DOUTFILE=$OUT_DIR/$INSTALLER" \
    "-DICON=$ROOT/app/PlancheContact/Assets/AppIcon.ico" \
    "$ROOT/installer/installer.nsi"

# One .sha256 file per package, in the format "sha256sum -c" reads.
(cd "$OUT_DIR" && sha256sum "$INSTALLER" > "$INSTALLER.sha256")

echo
echo "Done:"
ls -l "$OUT_DIR/$INSTALLER" "$OUT_DIR/$INSTALLER.sha256"
