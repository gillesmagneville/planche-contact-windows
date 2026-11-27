<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Changelog

🇫🇷 [Version française](CHANGELOG.fr.md)

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

Each release has a section whose heading is its date and its version, in
this exact form: `## 2026-11-27 — v5.3.0`. The release notes published on
GitHub are taken from it (see "Publishing a release" in `CONTRIBUTING.md`).

## [Unreleased]

## 2026-11-27 — v5.3.0

First release of **Planche-Contact for Windows**, which replaces the earlier
version for Windows (GTK window, Python engine). It makes the same contact
sheets, PDF, HTML gallery and CSV index as version 5.3.0 of
[planche-contact-linux](https://github.com/gillesmagneville/planche-contact-linux),
from the same settings, in the same four languages.

### Added

- **An engine in Rust** (`planche-contact-engine.exe`), a rewrite rule for
  rule of the Python engine of the Linux version: layout templates, captions,
  themes, watermark, profiles, PDF with selectable text, HTML gallery, CSV
  index. RAW files (`.cr2`, `.cr3`, `.nef`, `.dng`, `.arw`) are read with
  LibRaw, built into the engine: nothing else to install. The engine can be
  used on its own from a command line, with the options of `portfolio.py`.
- **A native Windows window** (C#, WinUI 3): the Generation tab, the
  thumbnails of the input folder, the advanced settings with the sheet
  preview, profiles to save and load, the choice of subfolders, the
  preferences (language), the manual in the Help tab, and About. The window
  never decodes a photo itself: a damaged file cannot freeze it.
- **"Open in Planche-Contact" in the File Explorer menu**, on a folder, the
  background of a folder or a selection of photos. It is an out-of-process
  COM handler: nothing of the program is loaded into File Explorer. On
  Windows 11 the entry is under **Show more options**.
- **A per-user installer**, without administrator rights, in
  `%LOCALAPPDATA%\Programs\Planche-Contact`: Start menu shortcut, entry in
  Settings > Apps, menu entry. It warns when the earlier version for Windows
  is still installed. The uninstaller removes everything it wrote and keeps
  the settings.
- The installer is published with its `.sha256` file.

### Changed, compared with the earlier version for Windows

- No Python or GTK runtime any more: the program starts faster and the
  installer is smaller.
- Installed for the current user instead of for all users: no
  administrator rights needed, at install or at uninstall.
- The settings (`%USERPROFILE%\.config\planche-contact\settings.json`) are
  the same file: last folders and language are kept.

### Known limits

- HEIC/HEIF files are listed, as on Linux, but cannot be read yet: they are
  left off the sheets and reported in the log.
- The installer is not code-signed: SmartScreen asks for confirmation.
