<!-- SPDX-License-Identifier: GPL-3.0-only -->

# vendor/

🇬🇧 [English version](README.md)

## libraw-sys

Copie de [`rsraw-sys` 0.1.1](https://crates.io/crates/rsraw-sys) (licence
MIT) : les liaisons Rust vers **LibRaw 0.21.3**, dont le code source C++
est inclus (`libraw-sys/LibRaw/`, licences `LICENSE.LGPL` et
`LICENSE.CDDL` conservées) et compilé **statiquement** dans
`planche-contact-engine`. Aucune bibliothèque LibRaw n'est donc à installer
sur la machine, ni pour compiler ni pour exécuter.

LibRaw lit tous les fichiers RAW (comme rawpy, qui enveloppe la même
bibliothèque, dans la version Linux) : l'aperçu intégré par l'appareil, le
développement complet quand cet aperçu est trop petit, et les informations de
prise de vue - voir `src/raw.rs`.

### Différences avec la version publiée sur crates.io

* `build.rs` : les liaisons sont toujours reprises du fichier pré-généré
  `src/bindings.rs` ; `bindgen` n'est plus appelé. Cela évite d'exiger
  `libclang` sur la machine de build. Seul un compilateur C++ (`g++`,
  fourni par `build-essential`) est nécessaire.
* `Cargo.toml` : la dépendance de build à `bindgen` est retirée.

Ces deux modifications viennent de nautilus-convert-images ; les trois
suivantes viennent d'explorer-convert-images, où elles ont été nécessaires
pour construire pour Windows (cette copie en est reprise telle quelle) :

* `build.rs` : pour Windows avec la chaîne GNU (MinGW-w64), la bibliothèque
  d'exécution C++ est liée statiquement ; sans cela le programme ne
  démarrerait pas sans `libstdc++-6.dll` à côté de lui.
* `build.rs` : une modification de `src/bindings.rs` déclenche désormais une
  reconstruction (elle était ignorée en silence).
* `src/bindings.rs` : sous Windows, `time_t` fait 64 bits. Les liaisons ont
  été générées sous Linux, où c'est un `long` ; sous Windows 64 bits un
  `long` fait 32 bits, et tout champ situé après un `time_t` aurait été lu
  au mauvais endroit. Avec cette modification les structures Rust ont, sous
  Windows aussi, exactement la disposition qu'utilise le code C++ (vérifiée
  champ par champ avec le compilateur MinGW-w64).

Rien d'autre n'est modifié.

### Mettre à jour LibRaw

Remplacer le contenu de `libraw-sys/` par une version plus récente de
`rsraw-sys`, ré-appliquer les modifications ci-dessus, puis lancer
`cargo test` (les tests de `src/raw.rs` décodent un DNG synthétique) —
nativement, et pour la cible Windows.
