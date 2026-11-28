<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Planche-Contact for Windows — Project context

🇫🇷 [Version française](PROJECT_CONTEXT.fr.md)

**Planche-Contact for Windows** makes high-quality (300 dpi) photographic
contact sheets from a folder of photos, with a native Windows window and an
entry in the File Explorer menu. From the same photos it makes, as chosen,
JPEG contact sheets, a PDF with selectable text, a paginated HTML gallery
and a CSV index.

It is the Windows version of
[planche-contact-linux](https://github.com/gillesmagneville/planche-contact-linux).
The two projects share their purpose, their settings, their results and
their texts; each has the window and the integration that belong to its
system. It is built the same way as
[explorer-convert-images](https://github.com/gillesmagneville/explorer-convert-images),
the Windows counterpart of nautilus-convert-images.

It works **entirely locally**: no account, no online service, no telemetry,
no database or catalog.

> **Note on this document:** it says *why* the project is made as it is.
> The exact contracts (the engine's commands and events, the registry
> entries, what must change together) are written once, in
> [ARCHITECTURE.md](ARCHITECTURE.md).

---

# Why a separate Windows version

Up to version 5.3.0, the Windows version was the Linux program itself:
the Python engine and the GTK 4 window, packed with their runtimes in an
installer for all users. It worked, but it carried a whole Python and GTK
runtime, was slow to start, foreign-looking on
Windows, needed administrator rights, and its File Explorer menu entry was
a C++ DLL loaded into File Explorer.

This repository replaces it with programs written for Windows, while
keeping what the user knows: the same settings, the same results, the same
texts, the same settings file.

---

# Principles

## 1. The same input gives the same result as on Linux

The engine is a rewrite, rule for rule, of the Python engine: same layout
numbers, captions, themes, watermark, profiles, CSV and HTML; sheets
identical to within the rasterisation of the text. A profile saved on one
system opens on the other. A change of behaviour is made in both versions.

## 2. The texts are those of the Linux version

`locales/*.py` are the Linux files, unchanged, read by both the engine and
the window. Only the few texts that the Windows window alone needs are
written here (`locales/windows/`).

## 3. Nothing runs inside File Explorer

The menu entry is an out-of-process COM server: File Explorer talks to a
program of ours, it never loads a DLL of ours. Whatever goes wrong on our
side, the user's file manager keeps working.

## 4. The window never decodes a photo

Thumbnails, the sheet preview and the generation are done by the engine,
in its own process. The window stays responsive, a damaged photo can at
worst make the engine report an error, and stopping a generation is
stopping a process.

## 5. Read-only on the photos

The program never modifies, moves or deletes a photo. It writes its
results in `Portfolio\` of the input folder, or in the chosen output
folder.

## 6. Leave the system as it was found

Installed for the current user only, without administrator rights. The
uninstaller removes everything the installer wrote — files and registry —
and nothing else; only the user's settings are kept.

## 7. What is written twice is checked by a test

The photo extensions of the engine and of the menu, the program names in
the menu handler, the installer and the project, the menu labels quoted by
the installer, the choice lists of the engine and of the window, the texts
the window uses: each pair has a test that fails when they disagree.

---

# Decisions

* **Rust for the engine**, as for explorer-convert-images: a single
  executable with no runtime, LibRaw built in for RAW files, rayon for the
  work that can run in parallel.
* **C# and WinUI 3 for the window**, built in code (no XAML pages),
  unpackaged and self-contained: no MSIX package, no certificate, nothing to
  install beforehand.
* **A text protocol between window and engine**: JSON lines and the
  `PROGRESS:` lines of the Linux version on the standard output. The engine
  stays usable on its own, and the window logic is tested on Linux against
  the real engine.
* **The selection goes through a file**: the menu handler writes the
  selected paths to a temporary file and passes its name with
  `--paths-from`, so a selection of any size is handed over in one go.
* **NSIS for the installer**, built on Linux, per user.
* **The settings file of the earlier version is kept**
  (`%USERPROFILE%\.config\planche-contact\settings.json`, same keys), so an
  upgrade keeps the last folders and the language.
* **Separate repositories** for Linux and Windows, at the price of carrying
  changes over by hand; the tables of `ARCHITECTURE.md` say where.

---

# Target platforms

Windows 10 (version 1809 or later) and Windows 11, 64-bit (x64). Windows
on ARM is not a target of its own: the x64 programs are expected to run
there through emulation, but this has not been verified.

---

# Known limits

* **HEIC/HEIF** files are listed, as on Linux, but cannot be read: they
  are left off the sheets and reported in the log (they still appear in the
  CSV index).
* **The installer is not code-signed**: SmartScreen asks for confirmation.
* **On Windows 11**, the first-level menu entry comes from an unsigned
  sparse package, which Windows 11 accepts for the current user; File
  Explorer is restarted once at installation. Where the package cannot be registered,
  the classic entry, under **Show more options**, is used instead.
* **One window per launch**: opening Planche-Contact again, from the menu
  or the Start menu, opens a second window rather than reusing the first.
* The complete chain — menu, handler, window — can only be checked on a
  real Windows (the workflow does it); Wine does not run the window.

---

# Roadmap

* A code-signed installer, which would remove the SmartScreen warning.
* Reading HEIC/HEIF, together with the Linux version.
* A single window, to which a second launch hands its folder.
* Whatever the Linux version gains (layouts, options, languages).

---

# Non-goals

Planche-Contact will not become a photo editor, a library manager or a
catalog. It reads a folder, makes sheets and their companions, and leaves
the photos as they are.
