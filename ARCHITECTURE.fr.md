<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Planche-Contact pour Windows — Architecture

🇬🇧 [English version](ARCHITECTURE.md)

Ce document est le contrat technique du projet : ce que fait chaque
programme, ce qu'ils se disent, et ce qui doit changer ensemble. Les raisons
de ces choix sont dans [PROJECT_CONTEXT.fr.md](PROJECT_CONTEXT.fr.md).

## Vue d'ensemble

```
 Explorateur de fichiers               (explorer.exe — rien de nous n'y tourne)
      │  clic droit → « Ouvrir dans Planche-Contact »
      │  COM : lance le gestionnaire, lui donne toute la sélection en un appel
      ▼
 planche-contact-verb.exe              le gestionnaire du menu (Rust)
      │  écrit les chemins sélectionnés dans un fichier texte
      │  lance : PlancheContact.exe --paths-from FICHIER
      ▼
 PlancheContact.exe                    la fenêtre (C#, WinUI 3)
      │  lance, autant que nécessaire :
      │    engine\planche-contact-engine.exe selection --paths-from FICHIER
      │    engine\planche-contact-engine.exe photos | thumbnails | folders | profile …
      │    engine\planche-contact-engine.exe generate -i … -o … --pdf --html --csv …
      │  lit ce qu'il écrit sur sa sortie standard, ligne par ligne
      ▼
 planche-contact-engine.exe            le moteur (Rust) : planches, PDF, galerie, CSV
```

Trois programmes séparés, volontairement :

* **Rien ne tourne dans l'Explorateur.** Le gestionnaire est un serveur COM
  *hors processus* : l'Explorateur parle à un de nos programmes, il ne
  charge jamais une de nos DLL. Un bug de notre côté ne peut ni planter ni
  ralentir `explorer.exe`.
* **La fenêtre ne décode jamais une photo.** Les vignettes, l'aperçu de la
  planche et la génération sont faits par le moteur, dans son propre
  processus : la fenêtre reste réactive, un fichier abîmé peut au pire faire
  signaler une erreur au moteur, et arrêter une génération, c'est arrêter ce
  processus.
* **Le moteur s'utilise seul**, en ligne de commande ou dans un script, avec
  les options de la version Linux.

## Le moteur (`engine/`)

Une réécriture en Rust du moteur Python de
[planche-contact-linux](https://github.com/gillesmagneville/planche-contact-linux)
(`portfolio/`), module par module :

| Module Rust | Équivalent Python | Rôle |
|---|---|---|
| `main.rs` | `portfolio.py`, `config.py`, `csvindex.py` | Commandes, options, index CSV |
| `scanner.rs` | `scanner.py` | Recherche des photos, sous-dossiers, listes de fichiers |
| `metadata.rs`, `raw.rs` | `rawloader.py`, `utils.py` | EXIF, fichiers RAW (LibRaw, `vendor/libraw-sys`) |
| `imaging.rs` | `thumbnail.py` | Chargement, orientation, vignettes, filigrane |
| `layout.rs`, `captions.rs`, `themes.rs`, `profile.rs` | `presentation/` | Modèles de mise en page, légendes, thèmes, profils |
| `contactsheet.rs`, `text.rs` | `contactsheet.py` | Dessin des planches (polices DejaVu, métriques de Pillow) |
| `pdf.rs` | `pdfexport.py`, `textlayer.py` | Le PDF, avec son texte invisible sélectionnable |
| `gallery.rs` | `htmlgallery.py` | La galerie HTML (`assets/gallery/`) |
| `launch.rs` | `launch.py` | Ce que désigne une sélection du menu contextuel |
| `i18n.rs` | `i18n.py` | Les textes de `locales/`, intégrés à la compilation |

Règle : **la même entrée donne le même résultat que sous Linux** — mêmes
valeurs de mise en page, mêmes légendes, mêmes CSV et HTML, planches
identiques au rendu du texte près. Un changement de comportement se fait à
la fois dans la version Linux et ici.

### Commandes

| Commande | Ce qu'elle écrit sur la sortie standard |
|---|---|
| `generate OPTIONS` | des lignes `PROGRESS:X/100 message` et des messages ; code de sortie 0 en cas de succès |
| `generate … --preview-output FICHIER.png` | la première planche seulement, puis `PREVIEW:affichées/total` |
| `info` | `{"event":"info", protocol, version, choices, profils par défaut…}` |
| `photos --input DOSSIER` | un `{"event":"photo","path":…}` par photo, puis `summary` |
| `folders --input DOSSIER [--output DOSSIER]` | un `{"event":"folder","path":…,"count":…}` par dossier, puis `summary` |
| `selection [--paths-from FICHIER] [CHEMIN…]` | `{"event":"selection","input_dir":…,"files":[…] ou null}` |
| `thumbnails --size N --output-dir DOSSIER --input-list FICHIER` | un `{"event":"thumbnail",…}` par photo, dans l'ordre |
| `profile --load FICHIER` | `{"event":"profile","profile":{…}}`, le fichier tel que le moteur le comprend |
| `--version` | `planche-contact-engine X.Y.Z` |

Tout est en UTF-8 et écrit ligne par ligne. Une erreur fatale est une ligne
`{"event":"fatal_error","message":…}` et un code de sortie non nul. Le
journal (`2026-01-31 18:05:12,345 [INFO] …`) va sur la sortie d'erreur et
dans `generation.log` du dossier de sortie. `events::PROTOCOL_VERSION`
augmente quand un événement existant change de forme.

## La fenêtre (`app/`)

* `PlancheContact.Core` — tout ce qui ne dessine pas : le formulaire traduit
  en arguments du moteur (`Generation.cs`), le profil (`Profile.cs`), les
  réglages (`Settings.cs`), le lancement du moteur (`Engine/`), les textes
  (`Localization/`). Aucune dépendance à Windows : construite et testée
  aussi sous Linux.
* `PlancheContact` — la fenêtre WinUI 3, construite en code : `MainWindow.cs`
  (cadre, onglet Génération, génération), `MainWindow.Preview.cs` (vignettes
  du dossier d'entrée), `MainWindow.Advanced.cs` (réglages avancés, profils,
  aperçu de la planche), `MainWindow.Dialogs.cs` (sous-dossiers,
  préférences, Aide, À propos). Sans paquet MSIX et autonome : le dossier
  publié contient les environnements .NET et Windows App SDK.
* `PlancheContact.Core.Tests` — un lanceur de tests en console (aucun
  framework à télécharger), exécuté avec le vrai moteur.

La fenêtre trouve le moteur dans `engine\planche-contact-engine.exe` à côté
d'elle, ou là où l'indique `PLANCHE_CONTACT_ENGINE` (développement et tests).
Ses réglages sont `%USERPROFILE%\.config\planche-contact\settings.json`, le
fichier et les clés de l'ancienne version GTK pour Windows.

## Les textes (`locales/`)

`locales/en.py`, `fr.py`, `de.py`, `es.py` sont **les fichiers de la version
Linux, inchangés** : le moteur les intègre à la compilation, la fenêtre les
embarque comme ressources et lit leurs lignes `"clé": "valeur"`. Les
quelques textes propres à la fenêtre Windows (clés `win.*` : erreurs du
moteur, « Quitter quand même »…) sont dans `locales/windows/xx.py`, de même
forme, fusionnés par la fenêtre. Des tests vérifient que chaque clé utilisée
par la fenêtre existe dans chaque langue, et que la fenêtre lit exactement
ce que lit Python.

## Le gestionnaire du menu (`shell-verb/`)

`planche-contact-verb.exe` est le gestionnaire d'explorer-convert-images,
adapté :

* `--register` / `--unregister` écrivent ou retirent, sous
  `HKEY_CURRENT_USER\Software\Classes`, la classe
  `{8BA32AF5-D148-4EEE-8021-9DF550039E2F}` (`LocalServer32` = ce programme)
  et un verbe `PlancheContact` sur `Directory`, `Directory\Background` et
  `SystemFileAssociations\.ext` pour chaque extension de
  `engine/src/scanner.rs`. Le libellé est choisi selon la langue de
  l'utilisateur à l'enregistrement (`labels.rs`).
* Lancé par COM avec `-Embedding`, il sert une commande : l'Explorateur
  donne la sélection (`IObjectWithSelection`), puis
  `IExecuteCommand::Execute` met le travail en file et rend la main aussitôt ;
  le programme écrit alors les chemins dans
  `%TEMP%\planche-contact-selection-….txt` et lance la fenêtre avec
  `--paths-from`. La commande `selection` du moteur lit ce fichier et le
  supprime.

## L'installeur (`installer/`)

NSIS, construit sous Linux par `build-installer.sh`. Par utilisateur
(`%LOCALAPPDATA%\Programs\Planche-Contact`, `RequestExecutionLevel user`) :
fichiers, raccourci du menu Démarrer, entrée de Paramètres > Applications
(HKCU), puis `planche-contact-verb.exe --register`. Une mise à jour
désenregistre et retire d'abord les fichiers précédents ; le désinstalleur
fait l'inverse.

Contenu installé :

```
%LOCALAPPDATA%\Programs\Planche-Contact\
    PlancheContact.exe, PlancheContact.pri, fichiers .NET et Windows App SDK
    planche-contact-verb.exe
    engine\planche-contact-engine.exe
    docs\planche-contact-manual[.en|.de|.es].html
    Uninstall.exe
```

## Ce qui doit changer ensemble

| Si vous changez… | …changez aussi |
|---|---|
| Les extensions de `engine/src/scanner.rs` | `shell-verb/src/extensions.rs` (un test les compare) |
| Le nom d'un programme | `shell-verb/src/plan.rs`, `installer/installer.nsi`, le `.csproj`, `build-installer.sh` (des tests comparent les trois premiers) |
| Les libellés du menu | `shell-verb/src/labels.rs` et la dernière page d'`installer/installer.nsi` (un test les compare) |
| Une liste de choix du moteur (`info`) | `Choices` dans `app/PlancheContact.Core/Profile.cs` (un test les compare) |
| Une commande ou un événement du moteur | `app/PlancheContact.Core/Engine/`, et `PROTOCOL_VERSION` si une forme change |
| `locales/*.py` | Rien ici : reprendre les fichiers Linux tels quels |
| La version | `tools/set-version.sh X.Y.Z` (VERSION, Cargo.toml, Cargo.lock) |

## Intégration continue

`.github/workflows/build.yml`, à chaque pull request :

1. **rust** (Linux) : `cargo test`, compilation croisée du moteur et du
   gestionnaire, vérification qu'aucun ne dépend d'une DLL MinGW.
2. **app** (Windows) : les tests C# avec le moteur construit pour Windows,
   les tests COM du gestionnaire, `dotnet publish` de la fenêtre, puis
   `shell-verb/examples/selftest.rs --dist` : le contenu du dossier, la
   fenêtre lancée directement, puis toute la chaîne gestionnaire du menu →
   fenêtre.
3. **installer** (Linux) : `build-installer.sh --dist`.
4. **install** (Windows) : `tools/check-install.ps1` installe en silence,
   vérifie fichiers, registre, raccourci, moteur et fenêtre, désinstalle et
   vérifie qu'il ne reste rien.
5. **release** (sur une étiquette `v*`) : une release GitHub en brouillon
   avec l'installeur et son `.sha256`, notes tirées des CHANGELOG.
