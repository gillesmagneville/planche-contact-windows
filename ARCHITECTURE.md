<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Planche-Contact for Windows — Architecture

🇫🇷 [Version française](ARCHITECTURE.fr.md)

This document is the technical contract of the project: what each program
does, what they say to each other, and what must change together. The
reasons behind it are in [PROJECT_CONTEXT.md](PROJECT_CONTEXT.md).

## Overview

```
 File Explorer                         (explorer.exe — nothing of ours runs in it)
      │  right-click → "Open in Planche-Contact"
      │  COM: starts the handler, gives it the whole selection in one call
      ▼
 planche-contact-verb.exe              the menu handler (Rust)
      │  writes the selected paths to a text file
      │  starts: PlancheContact.exe --paths-from FILE
      ▼
 PlancheContact.exe                    the window (C#, WinUI 3)
      │  starts, as often as needed:
      │    engine\planche-contact-engine.exe selection --paths-from FILE
      │    engine\planche-contact-engine.exe photos | thumbnails | folders | profile …
      │    engine\planche-contact-engine.exe generate -i … -o … --pdf --html --csv …
      │  reads what it writes on its standard output, line by line
      ▼
 planche-contact-engine.exe            the engine (Rust): sheets, PDF, gallery, CSV
```

Three separate programs, on purpose:

* **Nothing runs inside File Explorer.** The handler is an *out-of-process*
  COM server: File Explorer talks to a program of ours, it never loads a DLL
  of ours. A bug on our side cannot crash or slow down `explorer.exe`.
* **The window never decodes a photo.** The thumbnails, the sheet preview and
  the generation are made by the engine, in its own process: the window
  stays responsive, a damaged file can at worst make the engine report an
  error, and stopping a generation is stopping that process.
* **The engine is usable on its own**, from a command line or a script, with
  the options of the Linux version.

## The engine (`engine/`)

A rewrite in Rust of the Python engine of
[planche-contact-linux](https://github.com/gillesmagneville/planche-contact-linux)
(`portfolio/`), module by module:

| Rust module | Python counterpart | Role |
|---|---|---|
| `main.rs` | `portfolio.py`, `config.py`, `csvindex.py` | Commands, options, CSV index |
| `scanner.rs` | `scanner.py` | Finding the photos, subfolders, file lists |
| `metadata.rs`, `raw.rs` | `rawloader.py`, `utils.py` | EXIF, RAW files (LibRaw, `vendor/libraw-sys`) |
| `imaging.rs` | `thumbnail.py` | Loading, orientation, thumbnails, watermark |
| `layout.rs`, `captions.rs`, `themes.rs`, `profile.rs` | `presentation/` | Layout templates, captions, themes, profiles |
| `contactsheet.rs`, `text.rs` | `contactsheet.py` | Drawing the sheets (DejaVu fonts, Pillow metrics) |
| `pdf.rs` | `pdfexport.py`, `textlayer.py` | The PDF, with invisible selectable text |
| `gallery.rs` | `htmlgallery.py` | The HTML gallery (`assets/gallery/`) |
| `launch.rs` | `launch.py` | What a context-menu selection designates |
| `i18n.rs` | `i18n.py` | The texts of `locales/`, read at build time |

Rule: **the same input gives the same result as on Linux** — same layout
numbers, same captions, same CSV and HTML, sheets identical to within the
rasterisation of the text. A change of behaviour is made in the Linux
version and here together.

### Commands

| Command | Writes on the standard output |
|---|---|
| `generate OPTIONS` | `PROGRESS:X/100 message` lines and messages; exit code 0 on success |
| `generate … --preview-output FILE.png` | the first sheet only, then `PREVIEW:shown/total` |
| `info` | `{"event":"info", protocol, version, choices, default profiles…}` |
| `photos --input DIR` | one `{"event":"photo","path":…}` per photo, then `summary` |
| `folders --input DIR [--output DIR]` | one `{"event":"folder","path":…,"count":…}` per folder, then `summary` |
| `selection [--paths-from FILE] [PATH…]` | `{"event":"selection","input_dir":…,"files":[…] or null}` |
| `thumbnails --size N --output-dir DIR --input-list FILE` | one `{"event":"thumbnail",…}` per photo, in order |
| `profile --load FILE` | `{"event":"profile","profile":{…}}`, the file as the engine understands it |
| `--version` | `planche-contact-engine X.Y.Z` |

Everything is UTF-8 and flushed line by line. A fatal error is a
`{"event":"fatal_error","message":…}` line and a non-zero exit code. The log
(`2026-01-31 18:05:12,345 [INFO] …`) goes to the standard error and to
`generation.log` in the output folder. `events::PROTOCOL_VERSION` is raised
when an existing event changes shape.

## The window (`app/`)

* `PlancheContact.Core` — everything that does not draw: the form turned into
  engine arguments (`Generation.cs`), the profile (`Profile.cs`), the
  settings (`Settings.cs`), running the engine (`Engine/`), the texts
  (`Localization/`). No Windows dependency: it is built and tested on Linux
  too.
* `PlancheContact` — the WinUI 3 window, built in code: `MainWindow.cs`
  (frame, Generation tab, generation), `MainWindow.Preview.cs` (thumbnails of
  the input folder), `MainWindow.Advanced.cs` (advanced settings, profiles,
  sheet preview), `MainWindow.Dialogs.cs` (subfolders, preferences, Help,
  About). Unpackaged and self-contained: the published folder holds the .NET
  and Windows App SDK runtimes.
* `PlancheContact.Core.Tests` — a console test runner (no framework to
  download), run against the real engine.

The window finds the engine in `engine\planche-contact-engine.exe` next to
itself, or where `PLANCHE_CONTACT_ENGINE` says (development and tests). Its
settings are `%USERPROFILE%\.config\planche-contact\settings.json`, the file
and keys of the earlier GTK version for Windows.

## Texts (`locales/`)

`locales/en.py`, `fr.py`, `de.py`, `es.py` are **the files of the Linux
version, unchanged**: the engine embeds them at build time, the window
embeds them as resources and reads their `"key": "value"` lines. The few
texts only the Windows window needs (`win.*` keys: engine errors, "quit
anyway"…) are in `locales/windows/xx.py`, same shape, merged by the window.
Tests check that every key the window uses exists in every language, and
that the window reads exactly what Python reads.

## The menu handler (`shell-verb/`)

`planche-contact-verb.exe` is the handler of explorer-convert-images,
adapted:

* `--register` / `--unregister` write or remove, under
  `HKEY_CURRENT_USER\Software\Classes`, the class
  `{8BA32AF5-D148-4EEE-8021-9DF550039E2F}` (`LocalServer32` = this program)
  and a `PlancheContact` verb on `Directory`, `Directory\Background` and
  `SystemFileAssociations\.ext` for each extension of
  `engine/src/scanner.rs`. The label is chosen from the user's language at
  registration (`labels.rs`).
* Started by COM with `-Embedding`, it serves one command: File Explorer
  gives the selection (`IObjectWithSelection`), then `IExecuteCommand::Execute`
  queues the work and returns at once; the program then writes the paths
  to `%TEMP%\planche-contact-selection-….txt` and starts the window with
  `--paths-from`. The engine's `selection` command reads that file and
  deletes it.

On Windows 11 the first level of the menu only shows entries declared by a
package. `shell-menu/` is that entry:

* `planche_contact_menu.dll` implements `IExplorerCommand` (class
  `{5E3D6C2A-9B41-4F7E-A0C8-3F2B7D91C4E6}`): its title in the user's
  language (`labels.rs`), the window's icon, hidden unless a photo or a
  folder is selected, and on a click the same hand-over as the classic
  handler (`launch.rs`).
* The sparse package `planche-contact-menu.msix` (`shell-menu/packaging/`)
  declares it, in a COM surrogate: `dllhost.exe` loads the DLL, never
  `explorer.exe`. It holds only its manifest and logos; the files stay in
  the installation folder (`-ExternalLocation`).
* `tools/build-menu-package.ps1` builds and signs the package in the
  workflow, with a self-signed certificate made for each build;
  `planche-contact-menu.cer` is its public part.
* `menu-package.ps1`, installed with the programs, trusts the certificate
  (current user's "Trusted People") and registers the package, or removes
  both. When the package is registered, the classic entry is not: one entry
  only under "Show more options". On Windows 10, or if the registration
  fails, the classic entry is used.

## The installer (`installer/`)

NSIS, built on Linux by `build-installer.sh`. Per user
(`%LOCALAPPDATA%\Programs\Planche-Contact`, `RequestExecutionLevel user`):
files, Start menu shortcut, Settings > Apps entry (HKCU), then
`planche-contact-verb.exe --register`. An upgrade unregisters and removes the
previous files first; the uninstaller does the reverse.

Installed layout:

```
%LOCALAPPDATA%\Programs\Planche-Contact\
    PlancheContact.exe, PlancheContact.pri, .NET and Windows App SDK files
    planche-contact-verb.exe
    planche_contact_menu.dll, planche-contact-menu.msix, .cer, menu-package.ps1
    engine\planche-contact-engine.exe
    docs\planche-contact-manual[.en|.de|.es].html
    Uninstall.exe
```

## What must change together

| If you change… | …also change |
|---|---|
| The extensions of `engine/src/scanner.rs` | `shell-verb/src/extensions.rs` (a test compares them) |
| A program name | `shell-verb/src/plan.rs`, `installer/installer.nsi`, the `.csproj`, `build-installer.sh` (tests compare the first three) |
| The menu labels | `shell-verb/src/labels.rs` and the last page of `installer/installer.nsi` (a test compares them) |
| The class, DLL or package name of the Windows 11 entry | `shell-menu/src/lib.rs` and `shell-menu/packaging/AppxManifest.template.xml` (a test compares them), `installer/menu-package.ps1`, `tools/check-install.ps1` |
| A choice list of the engine (`info`) | `Choices` in `app/PlancheContact.Core/Profile.cs` (a test compares them) |
| An engine command or event | `app/PlancheContact.Core/Engine/` and `PROTOCOL_VERSION` if a shape changes |
| `locales/*.py` | Nothing here: take the Linux files as they are |
| The version | `tools/set-version.sh X.Y.Z` (VERSION, Cargo.toml, Cargo.lock) |

## Continuous integration

`.github/workflows/build.yml`, on each pull request:

1. **rust** (Linux): `cargo test`, cross-build of the engine and the handler,
   check that neither needs a MinGW DLL.
2. **app** (Windows): the C# tests against the engine built for Windows, the
   handler's COM tests, `dotnet publish` of the window, then
   `shell-verb/examples/selftest.rs --dist`: the folder's content, the window
   started directly, then the whole chain menu handler → window.
3. **installer** (Linux): `build-installer.sh --dist`.
4. **install** (Windows): `tools/check-install.ps1` installs silently, checks
   files, registry, shortcut, engine and window, uninstalls and checks that
   nothing is left.
5. **release** (on a `v*` tag): a draft GitHub release with the installer
   and its `.sha256`, notes taken from the CHANGELOGs.
