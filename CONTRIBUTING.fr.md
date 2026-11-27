<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Contribuer à Planche-Contact pour Windows

🇬🇧 [English version](CONTRIBUTING.md)

Merci de l'intérêt que vous portez à ce projet ! Ce document explique
comment signaler un problème, proposer une amélioration ou contribuer du
code.

En participant à ce projet, vous acceptez d'en respecter le
[Code de conduite](CODE_OF_CONDUCT.fr.md).

Pour signaler un **problème de sécurité**, merci de ne pas ouvrir de ticket
public : suivez la [Politique de sécurité](SECURITY.fr.md).

## Signaler un bug

Ouvrez un ticket avec, autant que possible :

- la version du programme (onglet **À propos**, ou **Paramètres >
  Applications > Applications installées**) et votre version de Windows
  (`winver`) ;
- ce que vous avez fait, ce que vous attendiez, ce qui s'est passé ;
- pour un problème de génération, le fichier `generation.log` du dossier de
  sortie ;
- le contenu de ces deux fichiers de votre dossier temporaire (tapez
  `%TEMP%` dans la barre d'adresse de l'Explorateur), s'ils existent :
  `planche-contact-verb.log` (l'entrée du menu) et `planche-contact-app.log`
  (la fenêtre) ;
- si le problème vient d'une photo précise, la photo elle-même — seulement
  si vous pouvez la partager.

Un problème qui se produit aussi avec la version Linux (une mise en page,
une légende, un fichier RAW illisible) est plutôt à signaler à
[planche-contact-linux](https://github.com/gillesmagneville/planche-contact-linux/issues),
où le comportement est défini.

## Proposer une amélioration

Ouvrez un ticket qui décrit le besoin plutôt que la solution. Lisez d'abord
[PROJECT_CONTEXT.fr.md](PROJECT_CONTEXT.fr.md) : ce que fait le programme
se décide avec la version Linux.

## Contribuer du code

Le projet compte trois programmes en deux langages (voir
[ARCHITECTURE.fr.md](ARCHITECTURE.fr.md)) :

| Partie | Où | Se construit sous |
|---|---|---|
| Moteur et gestionnaire du menu (Rust) | `engine/`, `shell-verb/` | Linux, pour Linux (tests) et pour Windows |
| Logique de la fenêtre (C#) | `app/PlancheContact.Core/` | Linux et Windows |
| La fenêtre elle-même (C#, WinUI 3) | `app/PlancheContact/` | Windows seulement |

Une machine Windows n'est pas nécessaire pour l'essentiel : le workflow
GitHub Actions construit et teste tout, fenêtre et installeur compris, à
chaque pull request.

### Se préparer sous Linux (Ubuntu/Debian)

```bash
git clone git@github.com:gillesmagneville/planche-contact-windows.git ~/Projects/planche-contact-windows
cd ~/Projects/planche-contact-windows

# Tests Rust, et tests C# avec le vrai moteur
sudo apt-get install build-essential dotnet-sdk-10.0 python3
cargo test
cargo build --release -p planche-contact-engine
dotnet run -c Release --project app/PlancheContact.Core.Tests
```

(Les tests C# utilisent `python3` pour vérifier que la fenêtre lit les
textes de `locales/` exactement comme la version Linux.)

Pour construire aussi les programmes Windows et l'installeur :

```bash
sudo apt-get install nsis gcc-mingw-w64-x86-64-posix g++-mingw-w64-x86-64-posix
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

La cible Windows demande un Rust installé avec `rustup` : le Rust des
distributions ne la contient pas. `rust-toolchain.toml` choisit ensuite la
version de Rust tout seul.

### Se préparer sous Windows

Installez le [SDK .NET 10](https://dotnet.microsoft.com/download). Puis,
dans PowerShell, à la racine du dépôt :

```powershell
dotnet publish app/PlancheContact -c Release -r win-x64 -p:Platform=x64 -o dist\planche-contact
```

Il manque au dossier publié les deux programmes Rust. Prenez-les dans
l'artefact `rust-windows` d'une exécution du workflow (ou construisez-les
sous Linux) et copiez `planche-contact-engine.exe` dans
`dist\planche-contact\engine\` et `planche-contact-verb.exe` dans
`dist\planche-contact\`. Copiez `docs\planche-contact-manual*.html` dans
`dist\planche-contact\docs\` pour l'onglet Aide.
`dist\planche-contact\PlancheContact.exe` peut alors être lancé. Pour avoir
l'entrée du menu sans installer, lancez `planche-contact-verb.exe
--register` depuis ce dossier (`--unregister` la retire).

Pour lancer les tests C# sous Windows, indiquez-leur un moteur construit
pour Windows :

```powershell
$env:PLANCHE_CONTACT_ENGINE = "C:\chemin\vers\planche-contact-engine.exe"
dotnet run -c Release --project app/PlancheContact.Core.Tests
```

### Construire l'installeur

Sous Linux, à partir d'un dossier contenant la fenêtre publiée :

```bash
./build-installer.sh --app DOSSIER
```

ou d'un dossier complet (l'artefact `planche-contact-folder` d'une
exécution du workflow) :

```bash
./build-installer.sh --dist DOSSIER
```

L'installeur et son fichier `.sha256` sont écrits dans `dist/`.

### Vérifier toute la chaîne sous Windows

`selftest` fait ce que fait l'Explorateur sur **Ouvrir dans
Planche-Contact** et vérifie que la fenêtre apparaît à l'écran, qu'elle est
toujours là vingt secondes plus tard et qu'elle n'a journalisé aucune
erreur :

```powershell
cargo build -p planche-contact-verb --example selftest
target\debug\examples\selftest.exe                              # le programme installé
target\debug\examples\selftest.exe --dist dist\planche-contact  # un dossier publié
```

`simulate_explorer.exe CHEMIN...` (construit de la même façon) appelle le
gestionnaire du menu installé sur les chemins donnés. `tools\check-install.ps1
-Installer SETUP.exe` installe en silence, vérifie tout ce que l'installeur
a écrit, désinstalle et vérifie qu'il ne reste rien — sur une machine de
test, puisqu'il remplace la version installée.

## Rester au pas de la version Linux

Le moteur est une réécriture du moteur Python de planche-contact-linux, et
les deux doivent donner les mêmes résultats (voir « Le moteur » dans
`ARCHITECTURE.fr.md`). Un changement de comportement — une valeur de mise
en page, une légende, un thème, une colonne du CSV, la galerie — se fait
dans la version Linux et ici, idéalement en deux pull requests qui se
citent. Le module Rust à modifier est donné, pour chaque fichier Python,
par le tableau d'`ARCHITECTURE.fr.md`.

Si une commande ou un événement du moteur change, modifiez
`app/PlancheContact.Core/Engine/` avec, et augmentez
`events::PROTOCOL_VERSION` si un événement existant change de forme. Si une
extension de photo est ajoutée, ajoutez-la à `shell-verb/src/extensions.rs`.
Les tests échouent tant que les deux côtés ne sont pas d'accord.

## Traduire l'interface

Les textes sont ceux de la version Linux : `locales/en.py`, `fr.py`,
`de.py` et `es.py` sont copiés de planche-contact-linux **sans aucune
modification**. Une traduction se corrige donc là-bas, puis les fichiers
sont recopiés ici tels quels.

Les quelques textes propres à la fenêtre Windows (clés `win.*`) sont dans
`locales/windows/en.py`, `fr.py`, `de.py` et `es.py`, de même forme. Ils se
corrigent ici. Après tout changement, lancez les tests C# : ils vérifient
que chaque clé utilisée par la fenêtre existe dans chaque langue, et que la
fenêtre lit les fichiers exactement comme Python.

Le texte de l'entrée du menu de l'Explorateur est dans
`shell-verb/src/labels.rs` et cité sur la dernière page de l'installeur
(`installer/installer.nsi`) : un test les compare.

**Pour ajouter une langue**, ajoutez-la d'abord à la version Linux, puis
copiez son fichier dans `locales/`, écrivez son fichier
`locales/windows/`, et ajoutez-la à `shell-verb/src/labels.rs` et à
`installer/installer.nsi`. Lancez `cargo test` et les tests C# : ils disent
ce qui manque.

## Mettre à jour le Windows App SDK ou .NET

La version du Windows App SDK est le `PackageReference` de
`app/PlancheContact/PlancheContact.csproj` ; celle du SDK .NET est dans
`global.json` et dans le workflow. Changez une chose à la fois, dans une
pull request à part : le workflow construit la fenêtre et la lance
(`selftest`), c'est ce qui dit si la nouvelle version fonctionne.

Après une mise à jour du Windows App SDK, regardez si
`app/PlancheContact/PublishWinUIResources.targets` est encore utile : son
en-tête explique comment le savoir.

## Publier une version

1. Mettre la version partout où elle est écrite :
   `tools/set-version.sh X.Y.Z`
2. Dans `CHANGELOG.md` et `CHANGELOG.fr.md`, transformer la section
   `[Unreleased]` (`[Non publié]`) en section de la version, avec un titre
   de cette forme exacte — `## 2026-11-27 — vX.Y.Z` — et ouvrir au-dessus
   une nouvelle section `[Non publié]` vide.
3. Vérifier les notes qui seront publiées :
   `tools/release-notes.sh X.Y.Z`
4. Committer, puis étiqueter et pousser :
   `git tag vX.Y.Z && git push origin main vX.Y.Z`
5. Attendre le workflow. Sur une étiquette, il crée une release en
   **brouillon** avec l'installeur, son fichier `.sha256` et les notes
   (anglais d'abord, français ensuite). Relisez le brouillon sur la page des
   Releases, essayez l'installeur sur une machine Windows, puis publiez.

Le workflow refuse de créer la release si l'étiquette et le fichier
`VERSION` ne concordent pas, ou si un des deux CHANGELOG n'a pas la
section. La version suit d'ordinaire celle de la version Linux.

## Style de code

- `cargo fmt` avant d'ouvrir une pull request.
- Rust stable seulement, aucune fonctionnalité nightly.
- La fenêtre contient le moins de logique possible : ce qui peut se décider
  sans fenêtre va dans `PlancheContact.Core`, avec un test.
- La fenêtre ne décode jamais une photo : vignettes et aperçus sont demandés
  au moteur.
- Rien n'est jamais chargé dans l'Explorateur : le gestionnaire du menu
  reste un serveur COM hors processus.
- Ce qui doit être écrit à deux endroits a un test qui les compare (voir
  « Ce qui doit changer ensemble » dans `ARCHITECTURE.fr.md`).

## Documentation

Si votre changement modifie un comportement visible, mettez à jour
`CHANGELOG.md` et `CHANGELOG.fr.md` dans la même pull request, sous
`[Unreleased]` (`[Non publié]` dans le fichier français),
`ARCHITECTURE.md` / `ARCHITECTURE.fr.md` si le contrat technique change, et
le manuel (`docs/planche-contact-manual*.html`, quatre langues) si ce que
voit l'utilisateur change.

Chaque document existe en deux langues : anglais (`README.md`,
`PROJECT_CONTEXT.md`, `ARCHITECTURE.md`, `CONTRIBUTING.md`,
`CODE_OF_CONDUCT.md`, `SECURITY.md`, `CHANGELOG.md`, affichés par défaut
sur GitHub) et français (les mêmes noms en `.fr.md`). Si vous en changez
un, changez l'autre. Si vous n'écrivez qu'une langue, dites-le dans la pull
request et le mainteneur traduira.

## Questions

Pour toute question qui n'entre pas dans les cas ci-dessus, ouvrez
simplement un ticket.

## Licence

En contribuant, vous acceptez que vos contributions soient publiées sous la
même licence que le projet (GPL-3.0-only). Le code de conduite est la seule
exception : c'est le Contributor Covenant 3.0, distribué sous CC BY-SA 4.0.
