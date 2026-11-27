<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Contributing to Planche-Contact for Windows

🇫🇷 [Version française](CONTRIBUTING.fr.md)

Thank you for your interest in this project! This document explains how to
report a problem, suggest an improvement, or contribute code.

By taking part in this project, you agree to follow its
[Code of Conduct](CODE_OF_CONDUCT.md).

To report a **security problem**, please do not open a public issue: follow
the [Security Policy](SECURITY.md).

## Reporting a bug

Open an issue with, as far as you can:

- the version of the program (**About** tab, or **Settings > Apps >
  Installed apps**) and your version of Windows (`winver`);
- what you did, what you expected, what happened;
- for a generation problem, the `generation.log` file of the output folder;
- the content of these two files of your temporary folder (type `%TEMP%` in
  the address bar of File Explorer), if they exist:
  `planche-contact-verb.log` (the menu entry) and `planche-contact-app.log`
  (the window);
- if the problem comes from a particular photo, the photo itself — only if
  you may share it.

A problem that also happens with the Linux version (a layout, a caption, a
RAW file that cannot be read) is better reported to
[planche-contact-linux](https://github.com/gillesmagneville/planche-contact-linux/issues),
where the behaviour is defined.

## Suggesting an improvement

Open an issue describing the need rather than the solution. Please read
[PROJECT_CONTEXT.md](PROJECT_CONTEXT.md) first: what the program makes is
decided together with the Linux version.

## Contributing code

The project has three programs in two languages (see
[ARCHITECTURE.md](ARCHITECTURE.md)):

| Part | Where | Builds on |
|---|---|---|
| Engine and menu handler (Rust) | `engine/`, `shell-verb/` | Linux, for Linux (tests) and for Windows |
| Logic of the window (C#) | `app/PlancheContact.Core/` | Linux and Windows |
| The window itself (C#, WinUI 3) | `app/PlancheContact/` | Windows only |

You do not need a Windows machine for most of it: the GitHub Actions
workflow builds and tests everything, including the window and the
installer, on each pull request.

### Setting up on Linux (Ubuntu/Debian)

```bash
git clone git@github.com:gillesmagneville/planche-contact-windows.git ~/Projects/planche-contact-windows
cd ~/Projects/planche-contact-windows

# Rust tests, and the C# tests against the real engine
sudo apt-get install build-essential dotnet-sdk-10.0 python3
cargo test
cargo build --release -p planche-contact-engine
dotnet run -c Release --project app/PlancheContact.Core.Tests
```

(The C# tests use `python3` to check that the window reads the texts of
`locales/` exactly as the Linux version does.)

To also build the Windows programs and the installer:

```bash
sudo apt-get install nsis gcc-mingw-w64-x86-64-posix g++-mingw-w64-x86-64-posix
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

The Windows target needs a Rust installed with `rustup`: the Rust packaged
by distributions does not include it. `rust-toolchain.toml` then selects
the Rust version by itself.

### Setting up on Windows

Install the [.NET 10 SDK](https://dotnet.microsoft.com/download). Then, in
PowerShell, at the root of the repository:

```powershell
dotnet publish app/PlancheContact -c Release -r win-x64 -p:Platform=x64 -o dist\planche-contact
```

The published folder lacks the two Rust programs. Take them from the
`rust-windows` artifact of a workflow run (or build them on Linux) and copy
`planche-contact-engine.exe` into `dist\planche-contact\engine\` and
`planche-contact-verb.exe` into `dist\planche-contact\`. Copy
`docs\planche-contact-manual*.html` into `dist\planche-contact\docs\` for
the Help tab. `dist\planche-contact\PlancheContact.exe` can then be started.
To get the menu entry without installing, run
`planche-contact-verb.exe --register` from that folder (`--unregister`
removes it).

To run the C# tests on Windows, point them at an engine built for Windows:

```powershell
$env:PLANCHE_CONTACT_ENGINE = "C:\path\to\planche-contact-engine.exe"
dotnet run -c Release --project app/PlancheContact.Core.Tests
```

### Building the installer

On Linux, from a folder holding the published window:

```bash
./build-installer.sh --app FOLDER
```

or from a complete folder (the `planche-contact-folder` artifact of a
workflow run):

```bash
./build-installer.sh --dist FOLDER
```

The installer and its `.sha256` file are written to `dist/`.

### Checking the whole chain on Windows

`selftest` does what File Explorer does on **Open in Planche-Contact** and
checks that the window comes on the screen, is still there twenty seconds
later, and that it logged no error:

```powershell
cargo build -p planche-contact-verb --example selftest
target\debug\examples\selftest.exe                              # the installed program
target\debug\examples\selftest.exe --dist dist\planche-contact  # a published folder
```

`simulate_explorer.exe PATH...` (built the same way) invokes the installed
menu handler on the paths you give. `tools\check-install.ps1 -Installer
SETUP.exe` installs silently, checks everything the installer wrote,
uninstalls and checks that nothing is left — on a test machine, since it
replaces the installed version.

## Keeping in step with the Linux version

The engine is a rewrite of the Python engine of planche-contact-linux, and
both must give the same results (see "The engine" in `ARCHITECTURE.md`). A
change of behaviour — a layout value, a caption, a theme, a CSV column, the
gallery — is made in the Linux version and here, ideally in two pull
requests that point to each other. The Rust module to change is given, for
each Python file, by the table of `ARCHITECTURE.md`.

If an engine command or event changes, change `app/PlancheContact.Core/Engine/`
with it, and raise `events::PROTOCOL_VERSION` if an existing event changes
shape. If a photo extension is added, add it to
`shell-verb/src/extensions.rs`. The tests fail until both sides agree.

## Translating the interface

The texts are those of the Linux version: `locales/en.py`, `fr.py`, `de.py`
and `es.py` are copied from planche-contact-linux **without any change**.
A translation is therefore corrected there, then the files are copied here
as they are.

The few texts only the Windows window uses (`win.*` keys) are in
`locales/windows/en.py`, `fr.py`, `de.py` and `es.py`, with the same shape.
They are corrected here. After any change, run the C# tests: they check that
every key the window uses exists in every language, and that the window
reads the files exactly as Python does.

The text of the File Explorer menu entry is in `shell-verb/src/labels.rs`
and is quoted on the last page of the installer (`installer/installer.nsi`):
a test compares them.

**To add a language**, add it to the Linux version first, then copy its
file to `locales/`, write its `locales/windows/` file, and add it to
`shell-verb/src/labels.rs` and to `installer/installer.nsi`. Run
`cargo test` and the C# tests: they tell you what is missing.

## Updating the Windows App SDK or .NET

The Windows App SDK version is the `PackageReference` of
`app/PlancheContact/PlancheContact.csproj`; the .NET SDK version is in
`global.json` and in the workflow. Change one thing at a time, in a pull
request of its own: the workflow builds the window and starts it
(`selftest`), which is what tells whether the new version works.

After an update of the Windows App SDK, see whether
`app/PlancheContact/PublishWinUIResources.targets` is still needed: its
header says how to tell.

## Publishing a release

1. Set the version everywhere it is written:
   `tools/set-version.sh X.Y.Z`
2. In `CHANGELOG.md` and `CHANGELOG.fr.md`, turn the `[Unreleased]`
   (`[Non publié]`) section into the section of the release, with a heading
   of this exact form — `## 2026-11-27 — vX.Y.Z` — and open a new, empty
   `[Unreleased]` section above it.
3. Check the notes that will be published:
   `tools/release-notes.sh X.Y.Z`
4. Commit, then tag and push:
   `git tag vX.Y.Z && git push origin main vX.Y.Z`
5. Wait for the workflow. On a tag it creates a **draft** release with the
   installer, its `.sha256` file and the notes (English first, French
   below). Review the draft on the Releases page, try the installer on a
   Windows machine, then publish.

The workflow refuses to create the release if the tag and the `VERSION`
file disagree, or if either changelog lacks the section. The version
usually follows the Linux version's.

## Code style

- `cargo fmt` before opening a pull request.
- Stable Rust only, no nightly features.
- The window holds as little logic as possible: what can be decided without
  a window goes to `PlancheContact.Core`, with a test.
- The window never decodes a photo: thumbnails and previews are asked of the
  engine.
- Nothing is ever loaded into File Explorer: the menu handler stays an
  out-of-process COM server.
- What must be written in two places gets a test that compares them (see
  "What must change together" in `ARCHITECTURE.md`).

## Documentation

If your change alters observable behaviour, update `CHANGELOG.md` and
`CHANGELOG.fr.md` in the same pull request, under `[Unreleased]`
(`[Non publié]` in the French file), `ARCHITECTURE.md` /
`ARCHITECTURE.fr.md` if the technical contract changes, and the manual
(`docs/planche-contact-manual*.html`, four languages) if what the user sees
changes.

Every document exists in two languages: English (`README.md`,
`PROJECT_CONTEXT.md`, `ARCHITECTURE.md`, `CONTRIBUTING.md`,
`CODE_OF_CONDUCT.md`, `SECURITY.md`, `CHANGELOG.md`, shown by default on
GitHub) and French (the same names with `.fr.md`). If you change one,
change the other. If you only write one language, say so in the pull
request and the maintainer will translate it.

## Questions

For any question that does not fit the cases above, simply open an issue.

## License

By contributing, you agree that your contributions will be released under
the same license as the project (GPL-3.0-only). The code of conduct is the
one exception: it is the Contributor Covenant 3.0, distributed under CC BY-SA 4.0.
