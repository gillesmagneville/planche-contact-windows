<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Planche-Contact pour Windows

🇬🇧 [English version](README.md)

Planches contact photographiques de haute qualité (300 dpi), avec une
interface Windows native. C'est la version Windows de
[Planche-Contact](https://github.com/gillesmagneville/planche-contact-linux),
construite comme
[Convert images pour Windows](https://github.com/gillesmagneville/explorer-convert-images) :

| Dossier | Contenu | Langage |
|---|---|---|
| `engine/` | Le moteur : planches contact (JPEG), PDF à texte sélectionnable, galerie HTML, index CSV, fichiers RAW | Rust |
| `app/` | La fenêtre (WinUI 3) et sa bibliothèque testée | C# |
| `shell-verb/` | « Ouvrir dans Planche-Contact » dans le menu clic droit de l'Explorateur | Rust |
| `installer/` | Installateur par utilisateur, sans droits d'administrateur | NSIS — *à venir* |

Le moteur est une réécriture, règle par règle, du moteur Python de la
version Linux : mêmes mises en page, légendes, thèmes, filigrane, galerie et
textes. Les textes des quatre langues (`locales/`) sont les fichiers de la
version Linux, inchangés.

## Construire le moteur

Sous Linux (Debian / Ubuntu), pour Windows :

```bash
sudo apt-get install gcc-mingw-w64-x86-64-posix g++-mingw-w64-x86-64-posix
rustup target add x86_64-pc-windows-gnu
cargo test
cargo build --release --target x86_64-pc-windows-gnu
```

Le moteur est un programme en ligne de commande : `planche-contact-engine
generate -i DOSSIER --pdf --html --csv` accepte les options du `portfolio.py`
de la version Linux. Les autres commandes, utilisées par la fenêtre, sont
décrites en tête de `engine/src/main.rs`.

## Construire la fenêtre

La bibliothèque `app/PlancheContact.Core` (arguments, réglages, dialogue
avec le moteur, traductions) se construit et se teste aussi sous Linux, avec
le SDK .NET 10 et le moteur construit par `cargo build --release` :

```bash
dotnet run -c Release --project app/PlancheContact.Core.Tests
```

La fenêtre elle-même se construit sous Windows :

```powershell
dotnet publish app/PlancheContact -c Release -r win-x64 -p:Platform=x64 -o dist/planche-contact
```

puis `planche-contact-engine.exe` se place dans `dist/planche-contact/engine/`.
Les textes propres à la fenêtre Windows sont dans `locales/windows/` ; tous
les autres sont ceux de `locales/`, partagés avec la version Linux.

## Licence

GPL-3.0-only (voir `LICENSE`). LibRaw (`engine/vendor/`) est sous LGPL 2.1
ou CDDL 1.0 ; les polices DejaVu (`engine/assets/fonts/`) sous leur propre
licence libre.
