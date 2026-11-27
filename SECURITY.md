<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Security Policy

🇫🇷 [Version française](SECURITY.fr.md)

Planche-Contact is maintained by one person, on a voluntary basis. This page explains how to report a security problem and what you can expect in return.

## Supported versions

Only the **latest release** receives security fixes. If you use an older version, please update first: the installer is on the [Releases page](https://github.com/gillesmagneville/planche-contact-windows/releases) and the changes are listed in the [CHANGELOG](CHANGELOG.md).

## Reporting a vulnerability

**Please do not report a vulnerability in a public issue, pull request or discussion**: anyone could read it before a fix exists.

Report it privately, in one of these two ways:

1. **On GitHub**: open the [Security tab](https://github.com/gillesmagneville/planche-contact-windows/security) of the repository and click **Report a vulnerability**. Only you and the maintainer can see the report.
2. **By email**: write to [gilles@magneville.fr](mailto:gilles@magneville.fr) with the subject "Planche-Contact security".

Please include, as far as you can:

- the version of the program (**Settings > Apps > Installed apps**) and your version of Windows (`winver`);
- a description of the problem and the steps to reproduce it;
- what you think the impact is;
- if the problem comes from a particular file, the file itself — only if you may share it.

## What to expect

These are goals, not guarantees, since there is only one maintainer:

- an acknowledgement of your report within **7 days**;
- an answer on whether it is considered a vulnerability, then news at least every few weeks until it is settled;
- a fix published in a **new release**, mentioned in the CHANGELOG; for a serious problem, a GitHub security advisory too;
- you are credited in the release notes, unless you prefer to stay anonymous.

Please keep the details private until a fixed version is available, so that users have the time to update.

## What is covered

Planche-Contact works on your computer, on your own photos, and does not send them anywhere. It does, however, **read image files that may come from anywhere** (a download, an email, a memory card). A report is relevant, for example, if:

- an image or RAW file makes the engine or the interface crash in an exploitable way, run code, or read or write files outside the chosen output folder;
- a file or folder name makes the menu handler or the interface run a command;
- the installer, what it writes to the registry, or the build and release scripts can be abused;
- a file of a release does not match its published `.sha256` fingerprint.

## What is not covered

- A vulnerability in **Windows, File Explorer, .NET or the Windows App SDK** itself: please report it to Microsoft.
- A vulnerability in a **component developed by someone else**: please report it to that project. Some of these components are **built into the program** (the Rust image decoders (`image`, `jpeg-decoder`), LibRaw, compiled statically, and the .NET and Windows App SDK runtimes shipped with the window): users cannot update them separately, so do tell me as well, since the fix needs a new release of Planche-Contact.
- Problems that need a computer which is already compromised, or physical access to it.
- Ordinary bugs without security consequences: please open an [issue](https://github.com/gillesmagneville/planche-contact-windows/issues) instead.

## Checking what you download

Download the installer only from the [Releases page](https://github.com/gillesmagneville/planche-contact-windows/releases) of this repository. Each installer is published with its own `.sha256` file (there is no global `SHA256SUMS`). To check a download, put the installer and its `.sha256` file in the same folder, then, in PowerShell:

```powershell
(Get-FileHash .\planche-contact-X.Y.Z-setup.exe -Algorithm SHA256).Hash
Get-Content .\planche-contact-X.Y.Z-setup.exe.sha256
```

The two fingerprints must be the same (letter case aside). On Linux: `sha256sum -c planche-contact-X.Y.Z-setup.exe.sha256`.

The installer is not code-signed: Windows shows a SmartScreen warning when it is run (see the README). The fingerprint is what tells you the file is the one published here.
