<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Planche-Contact pour Windows — Contexte du projet

🇬🇧 [English version](PROJECT_CONTEXT.md)

**Planche-Contact pour Windows** produit des planches contact
photographiques de haute qualité (300 dpi) à partir d'un dossier de photos,
avec une fenêtre Windows native et une entrée dans le menu de l'Explorateur
de fichiers. À partir des mêmes photos, il produit au choix des planches
JPEG, un PDF à texte sélectionnable, une galerie HTML paginée et un index
CSV.

C'est la version Windows de
[planche-contact-linux](https://github.com/gillesmagneville/planche-contact-linux).
Les deux projets partagent leur but, leurs réglages, leurs résultats et
leurs textes ; chacun a la fenêtre et l'intégration propres à son système.
Il est construit comme
[explorer-convert-images](https://github.com/gillesmagneville/explorer-convert-images),
le pendant Windows de nautilus-convert-images.

Il fonctionne **entièrement en local** : aucun compte, aucun service en
ligne, aucune télémétrie, ni base de données ni catalogue.

> **À propos de ce document :** il dit *pourquoi* le projet est fait ainsi.
> Les contrats exacts (commandes et événements du moteur, entrées du
> registre, ce qui doit changer ensemble) sont écrits une seule fois, dans
> [ARCHITECTURE.fr.md](ARCHITECTURE.fr.md).

---

# Pourquoi une version Windows à part

Jusqu'à la version 5.3.0, la version Windows était le programme Linux
lui-même : le moteur Python et la fenêtre GTK 4, emballés avec leurs
environnements dans un installeur pour tous les utilisateurs. Cela
fonctionnait, mais cela embarquait tout un environnement Python et
GTK, démarrait lentement, d'aspect étranger sous
Windows, il fallait des droits d'administrateur, et l'entrée du menu de
l'Explorateur était une DLL C++ chargée dans l'Explorateur.

Ce dépôt la remplace par des programmes écrits pour Windows, en gardant ce
que l'utilisateur connaît : les mêmes réglages, les mêmes résultats, les
mêmes textes, le même fichier de réglages.

---

# Principes

## 1. La même entrée donne le même résultat que sous Linux

Le moteur est une réécriture, règle par règle, du moteur Python : mêmes
valeurs de mise en page, légendes, thèmes, filigrane, profils, CSV et HTML ;
planches identiques au rendu du texte près. Un profil enregistré sur un
système s'ouvre sur l'autre. Un changement de comportement se fait dans les
deux versions.

## 2. Les textes sont ceux de la version Linux

`locales/*.py` sont les fichiers Linux, inchangés, lus par le moteur comme
par la fenêtre. Seuls les quelques textes dont la fenêtre Windows a besoin
sont écrits ici (`locales/windows/`).

## 3. Rien ne tourne dans l'Explorateur

L'entrée du menu est un serveur COM hors processus : l'Explorateur parle à
un de nos programmes, il ne charge jamais une de nos DLL. Quoi qu'il arrive
de notre côté, le gestionnaire de fichiers continue de fonctionner.

## 4. La fenêtre ne décode jamais une photo

Les vignettes, l'aperçu de la planche et la génération sont faits par le
moteur, dans son propre processus. La fenêtre reste réactive, une photo
abîmée peut au pire faire signaler une erreur au moteur, et arrêter une
génération, c'est arrêter un processus.

## 5. Lecture seule sur les photos

Le programme ne modifie, ne déplace et ne supprime jamais une photo. Il
écrit ses résultats dans `Portfolio\` du dossier d'entrée, ou dans le
dossier de sortie choisi.

## 6. Laisser le système comme on l'a trouvé

Installé pour l'utilisateur courant seulement, sans droits
d'administrateur. Le désinstalleur retire tout ce que l'installeur a écrit
— fichiers et registre — et rien d'autre ; seuls les réglages de
l'utilisateur sont conservés.

## 7. Ce qui est écrit deux fois est vérifié par un test

Les extensions de photos du moteur et du menu, les noms des programmes dans
le gestionnaire du menu, l'installeur et le projet, les libellés du menu
cités par l'installeur, les listes de choix du moteur et de la fenêtre, les
textes utilisés par la fenêtre : chaque paire a un test qui échoue quand
elles divergent.

---

# Décisions

* **Rust pour le moteur**, comme pour explorer-convert-images : un seul
  exécutable sans environnement à installer, LibRaw intégrée pour les
  fichiers RAW, rayon pour le travail qui peut se faire en parallèle.
* **C# et WinUI 3 pour la fenêtre**, construite en code (sans pages XAML),
  sans paquet MSIX et autonome : ni paquet, ni certificat, rien à installer
  au préalable.
* **Un protocole texte entre la fenêtre et le moteur** : des lignes JSON et
  les lignes `PROGRESS:` de la version Linux sur la sortie standard. Le
  moteur reste utilisable seul, et la logique de la fenêtre est testée sous
  Linux avec le vrai moteur.
* **La sélection passe par un fichier** : le gestionnaire du menu écrit les
  chemins sélectionnés dans un fichier temporaire et en donne le nom avec
  `--paths-from`, si bien qu'une sélection de toute taille passe en une
  fois.
* **NSIS pour l'installeur**, construit sous Linux, par utilisateur.
* **Le fichier de réglages de la version précédente est conservé**
  (`%USERPROFILE%\.config\planche-contact\settings.json`, mêmes clés) : une
  mise à jour garde les derniers dossiers et la langue.
* **Des dépôts séparés** pour Linux et Windows, au prix de reporter les
  changements à la main ; les tableaux d'`ARCHITECTURE.fr.md` disent où.

---

# Plateformes cibles

Windows 10 (version 1809 ou ultérieure) et Windows 11, en 64 bits (x64).
Windows sur ARM n'est pas une cible à part entière : les programmes x64
devraient y fonctionner par émulation, mais ce n'est pas vérifié.

---

# Limites connues

* Les fichiers **HEIC/HEIF** sont listés, comme sous Linux, mais ne peuvent
  pas être lus : ils sont écartés des planches et signalés dans le journal
  (ils figurent quand même dans l'index CSV).
* **L'installeur n'est pas signé numériquement** : SmartScreen demande une
  confirmation.
* **Sous Windows 11**, l'entrée au premier niveau du menu vient d'un paquet
  partiel non signé, que Windows 11 accepte pour l'utilisateur courant ;
  l'Explorateur redémarre une fois à l'installation. Si le paquet
  ne peut pas être enregistré, l'entrée classique, dans **Afficher plus
  d'options**, est utilisée à la place.
* **Une fenêtre par lancement** : rouvrir Planche-Contact, depuis le menu ou
  le menu Démarrer, ouvre une deuxième fenêtre au lieu de réutiliser la
  première.
* Toute la chaîne — menu, gestionnaire, fenêtre — ne peut être vérifiée que
  sur un vrai Windows (le workflow le fait) ; Wine ne lance pas la fenêtre.

---

# Feuille de route

* Un installeur signé, qui supprimerait l'avertissement SmartScreen.
* La lecture des HEIC/HEIF, avec la version Linux.
* Une seule fenêtre, à laquelle un deuxième lancement donne son dossier.
* Ce que gagnera la version Linux (mises en page, options, langues).

---

# Ce que le projet ne sera pas

Planche-Contact ne deviendra ni un éditeur de photos, ni un gestionnaire de
photothèque, ni un catalogue. Il lit un dossier, produit des planches et
leurs compléments, et laisse les photos telles quelles.
