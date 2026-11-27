<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Planche-Contact pour Windows

🇬🇧 [English version](README.md)

Des planches contact photographiques de haute qualité (300 dpi), à partir
d'un dossier de photos, avec une fenêtre Windows native. À partir des mêmes
photos, Planche-Contact produit en une fois :

- des **planches contact** en JPEG (une ou plusieurs pages) ;
- un **PDF** de toutes les planches, dont les légendes, le titre et les
  numéros de page peuvent être sélectionnés, copiés et recherchés ;
- une **galerie HTML** paginée et autonome, avec une visionneuse plein écran ;
- un **index CSV** des photos.

C'est la version Windows de
[Planche-Contact](https://github.com/gillesmagneville/planche-contact-linux) :
mêmes réglages, mêmes résultats, mêmes textes en quatre langues (français,
anglais, allemand, espagnol). Elle est construite comme
[Convertir les images pour Windows](https://github.com/gillesmagneville/explorer-convert-images) :
une fenêtre native (C#, WinUI 3), un moteur en Rust et une entrée dans le
menu de l'Explorateur de fichiers.

## Installation

Téléchargez `planche-contact-X.Y.Z-setup.exe` depuis la page des
[Releases](https://github.com/gillesmagneville/planche-contact-windows/releases)
et lancez-le. Il faut Windows 10 (version 1809 ou ultérieure) ou Windows 11,
en 64 bits ; rien d'autre n'est à installer au préalable.

L'installeur n'est pas signé numériquement : Windows affichera très
probablement un écran bleu « Windows a protégé votre PC » (SmartScreen).
Cliquez sur **Informations complémentaires**, puis sur **Exécuter quand
même**. Pour vérifier que le fichier est bien celui qui a été publié,
comparez son empreinte avec le fichier `.sha256` publié à côté :

```powershell
(Get-FileHash .\planche-contact-X.Y.Z-setup.exe -Algorithm SHA256).Hash
Get-Content .\planche-contact-X.Y.Z-setup.exe.sha256
```

### Ce que fait l'installeur

- Il installe **pour l'utilisateur courant seulement**, dans
  `%LOCALAPPDATA%\Programs\Planche-Contact` : aucun droit d'administrateur.
- Il ajoute **Planche-Contact** au menu Démarrer et à **Paramètres >
  Applications > Applications installées** (d'où on le désinstalle).
- Il ajoute **« Ouvrir dans Planche-Contact »** au menu du clic droit de
  l'Explorateur de fichiers.
- Une installation par-dessus une précédente la remplace ; le désinstalleur
  retire les fichiers, l'entrée du menu et le raccourci. Les réglages de la
  fenêtre (`%USERPROFILE%\.config\planche-contact\settings.json` : derniers
  dossiers et langue) sont conservés.

Si la **version précédente pour Windows** (fenêtre GTK, installée pour tous
les utilisateurs dans `C:\Program Files\Planche-Contact`) est encore
installée, l'installeur le signale : désinstallez-la d'abord depuis
Paramètres > Applications, sinon le menu de l'Explorateur affichera deux
entrées. La nouvelle version reprend ses derniers dossiers et sa langue.

## Utilisation

- **Depuis le menu Démarrer** : choisissez le dossier d'entrée, les options,
  puis **Lancer la génération**. Les résultats vont dans `Portfolio\` du
  dossier d'entrée, ou dans le dossier de sortie choisi ; **Afficher les
  résultats** les ouvre.
- **Depuis l'Explorateur de fichiers** : sélectionnez un dossier ou des
  photos, clic droit, **Ouvrir dans Planche-Contact** (sous Windows 11, dans
  **Afficher plus d'options**). La fenêtre s'ouvre sur ce dossier, ou sur
  exactement les photos sélectionnées.

Le manuel complet est dans l'onglet **Aide** de la fenêtre, en quatre
langues (`docs/` dans ce dépôt).

## Formats

| Type | Extensions |
|---|---|
| Standards | `.jpg`, `.jpeg`, `.png`, `.tif`, `.tiff`, `.bmp`, `.webp` |
| RAW | `.cr2`, `.cr3` (Canon), `.nef` (Nikon), `.dng`, `.arw` (Sony) |

Les fichiers RAW sont lus par LibRaw, intégrée au moteur : l'aperçu embarqué
par l'appareil est utilisé quand il est assez grand, sinon le fichier est
entièrement développé. Les fichiers HEIC/HEIF sont listés, comme sous Linux,
mais ne peuvent pas encore être lus.

## Ligne de commande

Le moteur est un programme en ligne de commande, dans le dossier
d'installation :

```bat
"%LOCALAPPDATA%\Programs\Planche-Contact\engine\planche-contact-engine.exe" generate -i C:\Photos\2025 --pdf --html --csv
```

`generate` accepte les options du `portfolio.py` de la version Linux
(`-n`, `--format`, `--title`, `--watermark`, `--profile`…) : voir le §4 du
manuel. Les autres commandes servent à la fenêtre ; elles sont décrites en
tête de `engine/src/main.rs`.

## Comment c'est fait

| Dossier | Contenu | Langage |
|---|---|---|
| `engine/` | Le moteur : planches contact (JPEG), PDF à texte sélectionnable, galerie HTML, index CSV, fichiers RAW | Rust |
| `app/` | La fenêtre (WinUI 3) et sa bibliothèque testée | C# |
| `shell-verb/` | « Ouvrir dans Planche-Contact » dans le menu de l'Explorateur (gestionnaire COM hors processus) | Rust |
| `installer/` | Installateur par utilisateur, sans droits d'administrateur | NSIS |
| `locales/` | Les textes en quatre langues : les fichiers de la version Linux, inchangés, plus `windows/` pour les quelques textes propres à cette fenêtre | Dictionnaires Python |
| `docs/` | Le manuel, en quatre langues | HTML |

Le moteur est une réécriture, règle par règle, du moteur Python de la
version Linux : mêmes mises en page, légendes, thèmes, filigrane, galerie et
textes. La façon dont les programmes se parlent est décrite dans
[ARCHITECTURE.fr.md](ARCHITECTURE.fr.md).

## Construire

Sous Linux (Ubuntu/Debian), les deux programmes Rust et l'installeur :

```bash
sudo apt-get install nsis gcc-mingw-w64-x86-64-posix g++-mingw-w64-x86-64-posix
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

Sous Windows (SDK .NET 10), la fenêtre, sous forme de dossier :

```powershell
dotnet publish app/PlancheContact -c Release -r win-x64 -p:Platform=x64 -o dist/planche-contact
```

Puis, de retour sous Linux, `./build-installer.sh --app DOSSIER` rassemble
le tout et construit `dist/planche-contact-X.Y.Z-setup.exe`. L'intégration
continue fait tout cela à chaque pull request : voir
[CONTRIBUTING.fr.md](CONTRIBUTING.fr.md).

## Tests

```bash
cargo test                                                       # moteur et gestionnaire du menu
cargo build --release
dotnet run -c Release --project app/PlancheContact.Core.Tests    # logique de la fenêtre, avec le vrai moteur
```

Sur GitHub Actions, en plus, la fenêtre est construite sous Windows, lancée
directement puis par le gestionnaire du menu, et l'installeur est installé,
vérifié et désinstallé sur un Windows vierge.

## Sécurité

Voir [SECURITY.fr.md](SECURITY.fr.md) pour signaler une vulnérabilité.

## Licence

GPL-3.0-only (voir `LICENSE`). LibRaw (`engine/vendor/`) est sous LGPL 2.1
ou CDDL 1.0 ; les polices DejaVu (`engine/assets/fonts/`) sous leur propre
licence libre.
