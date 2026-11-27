<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Planche-Contact for Windows

🇫🇷 [Version française](README.fr.md)

High-quality (300 dpi) photographic contact sheets, with a native Windows
interface. This is the Windows version of
[Planche-Contact](https://github.com/gillesmagneville/planche-contact-linux),
built the same way as
[Convert images for Windows](https://github.com/gillesmagneville/explorer-convert-images):

| Folder | What | Language |
|---|---|---|
| `engine/` | The engine: contact sheets (JPEG), PDF with selectable text, HTML gallery, CSV index, RAW files | Rust |
| `app/` | The window (WinUI 3) and its tested core library | C# |
| `shell-verb/` | "Open in Planche-Contact" in the Explorer's right-click menu | Rust |
| `installer/` | Per-user installer, no administrator rights | NSIS |

The engine is a rewrite, rule for rule, of the Python engine of the Linux
version: same layouts, captions, themes, watermark, gallery and texts. The
texts of the four languages (`locales/`) are the Linux version's files,
unchanged.

## Building the engine

On Linux (Debian / Ubuntu), for Windows:

```bash
sudo apt-get install gcc-mingw-w64-x86-64-posix g++-mingw-w64-x86-64-posix
rustup target add x86_64-pc-windows-gnu
cargo test
cargo build --release --target x86_64-pc-windows-gnu
```

The engine is a command-line program: `planche-contact-engine generate -i
FOLDER --pdf --html --csv` takes the options of the Linux version's
`portfolio.py`. See the comment at the top of `engine/src/main.rs` for the
other commands, used by the window.

## Build the window

The `app/PlancheContact.Core` library (arguments, settings, talking to the
engine, translations) also builds and is tested on Linux, with the .NET 10
SDK and the engine built by `cargo build --release`:

```bash
dotnet run -c Release --project app/PlancheContact.Core.Tests
```

The window itself builds on Windows:

```powershell
dotnet publish app/PlancheContact -c Release -r win-x64 -p:Platform=x64 -o dist/planche-contact
```

then `planche-contact-engine.exe` goes in `dist/planche-contact/engine/`.
The texts only the Windows window uses are in `locales/windows/`; all the
others are those of `locales/`, shared with the Linux version.

## License

GPL-3.0-only (see `LICENSE`). LibRaw (`engine/vendor/`) is under the LGPL
2.1 or the CDDL 1.0; the DejaVu fonts (`engine/assets/fonts/`) under their
own free license.
