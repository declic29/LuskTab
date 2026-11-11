# Mémo du projet Lusk Tab

Mémoire du projet, pour reprendre le travail dans une nouvelle conversation avec Claude : il suffit
de lui demander de lire ce fichier (`C:\LuskTab\docs\memo-projet.md`). Le cahier des charges complet
est le document « Lusk Tab — Cahier des charges » dans les artefacts Claude de Philippe. État au
11 novembre 2026.

## Le projet en bref

- **Lusk Tab** : tableur libre et gratuit (Windows, macOS, Linux), en Rust (egui). C'est une
  adaptation française de **GridCraft** 0.16.0 (ArtCraft Team, licences MIT ou Apache 2.0), partie
  du commit `e5ac9474`.
- Porté par l'**association Lusk** (loi 1901), présidée par Philippe. Contact public :
  contact@lusk.bzh. Site : https://www.lusk.bzh.
- Code : https://github.com/declic29/LuskTab (ancien nom LuskCalc), branche `main`. Sur le PC :
  `C:\LuskTab`.
- Projet frère : Lusk Texte (https://github.com/declic29/LuskTX, `C:\lusk-texte`), dont on reprend
  le cahier des charges, **sans le Mode Livre**.

## Décisions prises

- Nom affiché : **Lusk Tab**. Identifiant : `bzh.lusk.tab`. Éditeur : « Lusk ».
- En interne, le programme garde le nom `gridcraft` (binaire, crates, id de mise à jour, noms des
  fichiers d'icône), comme `wordcraft` dans Lusk Texte.
- **Noms des fonctions en français dès la version 1.0**, sur le modèle d'Excel en français
  (SOMME, MOYENNE, SI, RECHERCHEV, NB.SI…), séparateur point-virgule ; les noms anglais restent
  acceptés et les fichiers .xlsx gardent les noms anglais à l'intérieur.
- Logo fourni par l'association : `assets/app-icon/lusk-tab-logo.png` (crochets verts, « // » et
  « TAB » en dégradé), utilisé en entier pour l'icône.
- Couleur : vert **#7bc143** pour la barre verticale de gauche (à la place du vert de GridCraft).
- Fichiers publiés : `Lusk-Tab-…` uniquement ; noms fixes `Lusk-Tab-Windows.msi`,
  `Lusk-Tab-Mac.dmg`, `Lusk-Tab-Linux.AppImage`, `Lusk-Tab-Linux.deb`.
- Comme Lusk Texte : interface en français, dictionnaires français, breton et anglais intégrés,
  mises à jour cochées par défaut, boutons OneDrive et Google Drive, page Confidentialité, renard
  Lusk à l'accueil, installateur Windows en français aux couleurs de Lusk avec raccourci sur le
  Bureau coché, pas de compte Apple Developer, Dependabot mensuel.

## Fait

- 11 novembre 2026 : ménage du dépôt (SonarCloud, CodeRabbit, workflows FreeBSD, Nix et
  house-rules, AGENTS.md, CLAUDE.md, ROADMAP.md, docs/*.md, docs/brand, contributors, omastore.toml
  retirés) ; ligne Lusk dans LICENSE-MIT ; Dependabot mensuel ; identifiant `bzh.lusk.tab` ;
  icônes tirées du logo (`packaging/icons.sh` réécrit avec Pillow).

## À faire (dans l'ordre du cahier des charges)

1. Marque : nom affiché, éditeur, écran de démarrage, À propos, README en français, NOTICE, paquets
   Linux (metainfo, nfpm), macOS (Info.plist, DMG : le fond `packaging/macos/dmg/background.svg`
   pointait vers l'ancienne icône SVG, à refaire), `.exe` Windows ; couleur #7bc143.
2. Interface française relue, audit des textes, **noms des fonctions en français**.
3. Dictionnaires intégrés (reprendre le correcteur `proof` de Lusk Texte ; GridCraft ne lit que
   `/usr/share/dict/words`).
4. Accueil avec le renard, page Mises à jour reliée à `declic29/LuskTab`, OneDrive et Google Drive.
5. Aide en français et page Confidentialité (après un audit réseau).
6. Installateurs en français et workflow « Publication » (repris de Lusk Texte).
7. Version 1.0.0.

## Façon de travailler avec Claude

- Claude travaille dans une copie du dépôt et **copie les fichiers modifiés sur le PC**
  (`C:\LuskTab`), en gardant leurs fins de ligne. **Philippe fait lui-même `git add -A`,
  `git commit` et `git push`.**
- Les fichiers de `.github\` sont envoyés en pièce jointe et copiés à la main ; les suppressions se
  font avec `git rm`.
- Claude ne peut pas lire les journaux de GitHub Actions : copier-coller le message d'erreur.
- Pour compiler dans l'espace de Claude, Cargo doit passer par le proxy :
  `CARGO_HTTP_PROXY="$HTTPS_PROXY" NO_PROXY="" no_proxy=""`.
