<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Changelog

🇬🇧 [English version](CHANGELOG.md)

Toutes les modifications notables de ce projet sont documentées dans ce fichier.

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/),
et ce projet adhère au [versionnage sémantique](https://semver.org/lang/fr/).

Chaque version a une section dont le titre est sa date et sa version, sous
cette forme exacte : `## 2026-11-27 — v5.3.0`. Les notes de version publiées
sur GitHub en sont tirées (voir « Publier une version » dans
`CONTRIBUTING.fr.md`).

## [Non publié]

## 2026-11-27 — v5.3.0

Première version de **Planche-Contact pour Windows**, qui remplace la
version précédente pour Windows (fenêtre GTK, moteur Python). Elle produit
les mêmes planches contact, PDF, galerie HTML et index CSV que la version
5.3.0 de
[planche-contact-linux](https://github.com/gillesmagneville/planche-contact-linux),
avec les mêmes réglages, dans les mêmes quatre langues.

### Ajouté

- **Un moteur en Rust** (`planche-contact-engine.exe`), réécriture règle par
  règle du moteur Python de la version Linux : modèles de mise en page,
  légendes, thèmes, filigrane, profils, PDF à texte sélectionnable, galerie
  HTML, index CSV. Les fichiers RAW (`.cr2`, `.cr3`, `.nef`, `.dng`, `.arw`)
  sont lus par LibRaw, intégrée au moteur : rien d'autre à installer. Le
  moteur s'utilise seul en ligne de commande, avec les options de
  `portfolio.py`.
- **Une fenêtre Windows native** (C#, WinUI 3) : l'onglet Génération, les
  vignettes du dossier d'entrée, les réglages avancés avec l'aperçu de la
  planche, les profils à enregistrer et à charger, le choix des
  sous-dossiers, les préférences (langue), le manuel dans l'onglet Aide, et
  À propos. La fenêtre ne décode jamais une photo elle-même : un fichier
  abîmé ne peut pas la figer.
- **« Ouvrir dans Planche-Contact » dans le menu de l'Explorateur**, sur un
  dossier, le fond d'un dossier ou une sélection de photos. C'est un
  gestionnaire COM hors processus : rien du programme n'est chargé dans
  l'Explorateur. Sous Windows 11, l'entrée est dans **Afficher plus
  d'options**.
- **Un installeur par utilisateur**, sans droits d'administrateur, dans
  `%LOCALAPPDATA%\Programs\Planche-Contact` : raccourci du menu Démarrer,
  entrée dans Paramètres > Applications, entrée du menu. Il prévient quand
  la version précédente pour Windows est encore installée. Le désinstalleur
  retire tout ce qu'il a écrit et garde les réglages.
- L'installeur est publié avec son fichier `.sha256`.

### Modifié, par rapport à la version précédente pour Windows

- Plus d'environnement Python ni GTK : le programme démarre plus vite et
  l'installeur est plus petit.
- Installé pour l'utilisateur courant au lieu de tous les utilisateurs :
  aucun droit d'administrateur, ni à l'installation ni à la désinstallation.
- Les réglages (`%USERPROFILE%\.config\planche-contact\settings.json`) sont
  le même fichier : derniers dossiers et langue sont conservés.

### Limites connues

- Les fichiers HEIC/HEIF sont listés, comme sous Linux, mais ne peuvent pas
  encore être lus : ils sont écartés des planches et signalés dans le
  journal.
- L'installeur n'est pas signé numériquement : SmartScreen demande une
  confirmation.
