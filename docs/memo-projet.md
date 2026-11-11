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
- 11 novembre 2026, marque : « Lusk Tab » dans tous les textes affichés et leurs traductions
  (commentaires, noms internes et données stockées dans les classeurs inchangés) ; éditeur « Lusk »
  et copyright dans le `.exe`, Info.plist, la fiche Linux (metainfo, .desktop, nfpm) ; À propos
  simplifié (Lusk, origine GridCraft, liens lusk.bzh et GitHub) ; menu Aide macOS vers lusk.bzh ;
  réglages et journaux dans `%APPDATA%\LuskTab`, `~/Library/Application Support/Lusk Tab`,
  `~/.config/lusk-tab` ; barre de gauche en #7bc143 avec texte vert très foncé (contraste 7,4:1) ;
  README en français ; NOTICE au nom de Lusk. Option `--backstage <page>` ajoutée à l'exemple
  `snapshot` pour photographier les pages du menu Fichier.
- Le 11 novembre à 16 h 02, des modifications d'origine inconnue sont apparues dans la copie de
  travail de Claude ; elles ont été mises de côté (`git stash`) et l'étape refaite à la main.

- 11 novembre 2026, **fonctions en français** : `FormulaLocale::Fr` (crate `formula`), utilisé
  quand l'interface est en français. 393 noms traduits dans `crates/formula/src/french.rs`
  (sources : tables anglais/français d'excel-pratique.com, recoupées avec HyperFormula frFR et la
  liste de Microsoft) ; les autres fonctions gardent leur nom (ABS, DATE, INDEX…). Point-virgule
  entre les arguments, virgule décimale, `{1.2;3.4}` pour les tableaux, VRAI/FAUX. Les formules
  restent enregistrées en anglais dans les fichiers. Les noms anglais restent acceptés, sauf TRIM
  (= TRI modifié, MIRR, en français) et VAR.P (= VARP), comme dans Excel en français. Aide en
  français des ~50 fonctions courantes (`crates/ui-egui/src/i18n/fr_functions.rs`). Tests :
  `crates/formula/tests/it/french_input.rs`, `formula_locale::tests::french_function_help`.

## À faire (dans l'ordre du cahier des charges)

- **Affichage régional** : les cellules affichent encore `1.5` et les dates à l'américaine. Les
  formats régionaux français existent (`crates/core/src/region.rs`, `FR_FR`) mais
  `Workbook.region` n'est jamais réglé ni utilisé à l'affichage : à brancher (région du système,
  réglable dans les options).
- Codes d'erreur en français (#VALEUR!, #NOM?, #DIV/0!…) et noms par défaut Classeur1 / Feuil1.

1. Fin de la marque avec les installateurs : fichiers Windows (`gridcraft.wxs`, dossier
   `ArtCraft\GridCraft`, éditeur), macOS (`package.sh`, nom de l'app, DMG : le fond
   `packaging/macos/dmg/background.svg` pointait vers l'ancienne icône SVG, à refaire),
   `sign.ps1`, `flatpak-oci.sh`. Noms « Book1 » et « Sheet1 » à traduire (Classeur1, Feuil1).
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
