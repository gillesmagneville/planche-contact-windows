<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Planche-Contact for Windows

🇫🇷 [Version française](README.fr.md)

High-quality (300 dpi) photographic contact sheets, from a folder of photos,
with a native Windows window. From the same photos, Planche-Contact makes at
once:

- **contact sheets** in JPEG (one or more pages);
- a **PDF** of all the sheets, whose captions, title and page numbers can be
  selected, copied and searched;
- a paginated, self-contained **HTML gallery**, with a full-screen viewer;
- a **CSV index** of the photos.

This is the Windows version of
[Planche-Contact](https://github.com/gillesmagneville/planche-contact-linux):
same settings, same results, same texts in four languages (English, French,
German, Spanish). It is built the same way as
[Convert images for Windows](https://github.com/gillesmagneville/explorer-convert-images):
a native window (C#, WinUI 3), an engine in Rust, and an entry in the
File Explorer menu.

## Installation

Download `planche-contact-X.Y.Z-setup.exe` from the
[Releases page](https://github.com/gillesmagneville/planche-contact-windows/releases)
and run it. Windows 10 (version 1809 or later) or Windows 11, 64-bit, is
needed; nothing else has to be installed beforehand.

The installer is not code-signed: Windows will very likely show a blue
"Windows protected your PC" screen (SmartScreen). Click **More info**, then
**Run anyway**. To check that the file is the published one, compare its
fingerprint with the `.sha256` file published next to it:

```powershell
(Get-FileHash .\planche-contact-X.Y.Z-setup.exe -Algorithm SHA256).Hash
Get-Content .\planche-contact-X.Y.Z-setup.exe.sha256
```

### What the installer does

- It installs for **the current user only**, in
  `%LOCALAPPDATA%\Programs\Planche-Contact`: no administrator rights.
- It adds **Planche-Contact** to the Start menu and to **Settings > Apps >
  Installed apps** (where it is uninstalled).
- It adds **"Open in Planche-Contact"** to the right-click menu of File
  Explorer.
- An installation over an earlier one replaces it; the uninstaller removes
  the files, the menu entry and the shortcut. The settings of the window
  (`%USERPROFILE%\.config\planche-contact\settings.json`: last folders and
  language) are kept.

If the **earlier version for Windows** (GTK window, installed for all users
in `C:\Program Files\Planche-Contact`) is still installed, the installer says
so: uninstall it first from Settings > Apps, otherwise the File Explorer menu
shows two entries. The new version keeps its last folders and language.

## Usage

- **From the Start menu**: choose the input folder, the options, then
  **Start generation**. The results go to `Portfolio\` in the input folder, or to the
  output folder you choose; **View results** opens them.
- **From File Explorer**: select a folder or photos, right-click, **Open in
  Planche-Contact**. The window
  opens with that folder, or with exactly the selected photos.

The full manual is in the **Help** tab of the window, in the four languages
(`docs/` in this repository).

## Formats

| Type | Extensions |
|---|---|
| Standard | `.jpg`, `.jpeg`, `.png`, `.tif`, `.tiff`, `.bmp`, `.webp` |
| RAW | `.cr2`, `.cr3` (Canon), `.nef` (Nikon), `.dng`, `.arw` (Sony) |

RAW files are read with LibRaw, built into the engine: the preview embedded
by the camera is used when it is large enough, otherwise the file is fully
developed. HEIC/HEIF files are listed, as on Linux, but cannot be read yet.

## Command line

The engine is a command-line program, in the installation folder:

```bat
"%LOCALAPPDATA%\Programs\Planche-Contact\engine\planche-contact-engine.exe" generate -i C:\Photos\2025 --pdf --html --csv
```

`generate` takes the options of the Linux version's `portfolio.py`
(`-n`, `--format`, `--title`, `--watermark`, `--profile`…): see §4 of the
manual. The other commands are used by the window; they are described at
the top of `engine/src/main.rs`.

## How it is made

| Folder | What | Language |
|---|---|---|
| `engine/` | The engine: contact sheets (JPEG), PDF with selectable text, HTML gallery, CSV index, RAW files | Rust |
| `app/` | The window (WinUI 3) and its tested core library | C# |
| `shell-verb/` | "Open in Planche-Contact" in the File Explorer menu (out-of-process COM handler) | Rust |
| `installer/` | Per-user installer, no administrator rights | NSIS |
| `locales/` | The texts in four languages: the Linux version's files, unchanged, plus `windows/` for the few texts only this window uses | Python dictionaries |
| `docs/` | The manual, in four languages | HTML |

The engine is a rewrite, rule for rule, of the Python engine of the Linux
version: same layouts, captions, themes, watermark, gallery and texts. How
the programs talk to each other is described in
[ARCHITECTURE.md](ARCHITECTURE.md).

## Building

On Linux (Ubuntu/Debian), the two Rust programs and the installer:

```bash
sudo apt-get install nsis gcc-mingw-w64-x86-64-posix g++-mingw-w64-x86-64-posix
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

On Windows (.NET 10 SDK), the window, as a folder:

```powershell
dotnet publish app/PlancheContact -c Release -r win-x64 -p:Platform=x64 -o dist/planche-contact
```

Then, back on Linux, `./build-installer.sh --app DIR` gathers everything and
builds `dist/planche-contact-X.Y.Z-setup.exe`. The continuous integration
does all of it on each pull request: see [CONTRIBUTING.md](CONTRIBUTING.md).

## Tests

```bash
cargo test                                                       # engine and menu handler
cargo build --release
dotnet run -c Release --project app/PlancheContact.Core.Tests    # window logic, against the real engine
```

On GitHub Actions, on top of these, the window is built on Windows, started
directly and through the menu handler, and the installer is installed,
checked and uninstalled on a clean Windows.

## Security

See [SECURITY.md](SECURITY.md) to report a vulnerability.

## License

GPL-3.0-only (see `LICENSE`). LibRaw (`engine/vendor/`) is under the LGPL
2.1 or the CDDL 1.0; the DejaVu fonts (`engine/assets/fonts/`) under their
own free license.
