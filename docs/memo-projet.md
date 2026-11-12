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
  quand l'interface est en français. 394 noms traduits dans `crates/formula/src/french.rs`
  (sources : tables anglais/français d'excel-pratique.com, recoupées avec HyperFormula frFR et la
  liste de Microsoft) ; les autres fonctions gardent leur nom (ABS, DATE, INDEX…). Point-virgule
  entre les arguments, virgule décimale, `{1.2;3.4}` pour les tableaux, VRAI/FAUX. Les formules
  restent enregistrées en anglais dans les fichiers. Les noms anglais restent acceptés, sauf TRIM
  (= TRI modifié, MIRR, en français) et VAR.P (= VARP), comme dans Excel en français. Aide en
  français des ~50 fonctions courantes (`crates/ui-egui/src/i18n/fr_functions.rs`). Tests :
  `crates/formula/tests/it/french_input.rs`, `formula_locale::tests::french_function_help`.

- 11 novembre 2026, **reprise de GridCraft 0.19.0** (fusion de `storytold/gridcraft` main
  `6dbfbc40`, sans conflit). Pour reprendre plus tard les nouveautés de GridCraft :
  `git pull --no-rebase https://github.com/storytold/gridcraft main`, puis vérifier.
- 11 novembre 2026, **affichage à la française** : la région du système (Windows › Région ›
  Format régional) règle l'affichage des nombres et des dates (`1 234,50`, `11/11/2026`, mois et
  jours en français) ; la langue de l'interface règle VRAI/FAUX, les erreurs (#VALEUR!, #NOM?,
  #NOMBRE!, #NUL!, #PROPAGATION!) et les noms des nouveaux classeurs et feuilles (Classeur1,
  Feuil1). Les dates et nombres se tapent à la française (`25/12/2026`, `1 234,5`). Les valeurs
  enregistrées, les formules dans les fichiers et TEXTE() restent en anglais canonique. Code :
  `crates/numfmt/src/display.rs` (région et mots d'affichage), `formula_locale::regional_date`,
  `model::workbook::set_default_names`, `i18n::use_system_display` (appelé par l'app de bureau et
  la version web uniquement : les tests et l'outil en ligne de commande restent en en-US).
  L'exemple `snapshot` accepte `--region fr-FR`.

- 11 novembre 2026, **fin de l'affichage à la française** : Fichier › Options › Général ›
  « Format régional » (Automatique = celui du système, ou une région au choix ; réglage `region`
  des préférences de l'interface) ; dates tapées avec le nom du mois (« 7 oct. 2026 »,
  « 1er janvier 2027 », « 25 déc. ») ; codes d'erreur tapés en français dans les formules et les
  cellules (#VALEUR!, #NOM?, #NOMBRE!, #NUL!, #PROPAGATION! ; les noms anglais restent acceptés).
  Code : `i18n::apply_region`, `formula_locale::named_month_date`, `FormulaLocale::error_name`.
- 11 novembre 2026, **dictionnaires intégrés** : le correcteur de Lusk Texte est repris tel quel
  dans `crates/proof` (paquet `gridcraft-proof`), avec ses dictionnaires (`assets/dictionaries/fr`
  Grammalecte, `assets/dictionaries/br` An Drouizig, liste anglaise Moby dans
  `assets/hyphenation/en-us.dic`) ; mentions dans NOTICE et ATTRIBUTION.md ; `.gitattributes`
  garde ces fichiers à l'octet près. Révision › Orthographe (F7) vérifie désormais les mots
  accentués, dans la langue choisie dans Fichier › Options › « Langue du dictionnaire »
  (Français par défaut, Brezhoneg, English US/UK ; préférence `spellingLanguage`, commande
  `review.spellingLanguage`). Les mots tout en capitales (TVA, SIRET) sont ignorés. Le dictionnaire
  personnel (Ajouter au dictionnaire) est inchangé.
- 11 novembre 2026, **dictionnaire des synonymes** : Révision › Synonymes (Maj+F7) ouvre la
  fenêtre « Dictionnaire des synonymes » : les sens du mot de la cellule active avec leurs
  synonymes, double-clic pour chercher un synonyme, « Insérer » pour remplacer le mot dans la
  cellule. Français : thésaurus Dicollecte 2.3 de LibreOffice (`assets/thesaurus/fr/thes_fr.dat`,
  LGPL 2.1, intégré tel quel) ; anglais : WordNet (`assets/thesaurus/en/th_en_US_v2.dat.gz`,
  compressé). Pas de thésaurus breton libre. Un `thes_fr.dat` placé dans un dossier
  `dictionnaires` à côté du programme remplace celui qui est intégré (obligation de la LGPL,
  expliquée dans `assets/thesaurus/LISEZ-MOI.txt`). Code : `crates/proof/src/thesaurus.rs`,
  commande `review.thesaurus`.
- 11 novembre 2026, **relecture de l'interface française** : les 735 textes que le catalogue
  français n'avait pas sont traduits (vocabulaire d'Excel en français), et environ 300 textes
  écrits en dur dans le code passent maintenant par la traduction (fenêtres Rechercher,
  Orthographe, Solveur, Analyse de données, contrôles de formulaire, éditeur de macros, volets,
  menus du ruban, barre d'état « Moyenne : … Nombre : … Somme : … », options de recopie,
  filtre « (Sélectionner tout) »…), avec environ 480 traductions de plus. Outils :
  `crate::i18n::tr(ui, "…")` dans le code d'affichage, `crate::i18n::tr_now("…")` ailleurs ;
  les phrases avec une valeur utilisent des clés à emplacement (`"Average: {value}"`).
  `cargo xtask i18n-report` donne « fr: 0 missing ».
- 11 novembre 2026, **rapports en français** : quand l'interface est en français, les intitulés
  que le moteur écrit dans les cellules sont en français, comme dans Excel en français :
  Analyse de données (Moyenne, Erreur-type, RAPPORT DÉTAILLÉ, ANALYSE DE VARIANCE, Niveau de
  confiance(95,0%)…), rapports du Solveur (feuilles « Rapport des réponses 1 »…), Synthèse de
  scénarios, Feuille de prévision (Chronologie, Prévision(…)), tableaux croisés dynamiques
  (Étiquettes de lignes, Total général, « Total Est », champs « Somme de Ventes »). Table :
  `crates/engine/src/report_words.rs`, activée par le réglage des mots français de l'affichage
  (`gridcraft_numfmt::french_words`) ; les tests et l'outil en ligne de commande restent en
  anglais. « (blank) » des tableaux croisés reste en anglais, car il sert aussi aux filtres
  enregistrés.
- 11 novembre 2026, **messages aux agents en français** : les erreurs que le canal de pilotage
  renvoie aux agents (captures d'écran, saisie synthétique, impression, commandes `ui.*`) sont
  traduites au moment de l'envoi (`crates/ui-egui/src/agent_messages.rs`), dans la langue de
  l'interface ; les autres messages passent par le catalogue français.
- 11 novembre 2026, **écran d'accueil** (repris de Lusk Texte) : au démarrage, une fenêtre
  sans bordure apparaît en fondu avec l'image de fond (`assets/splash/accueil-lusk-tab.jpg`,
  fournie par l'association), le logo seul (sans texte à côté, à la demande de Philippe), une
  barre de chargement et les mentions légales (version, association loi
  1901, © 2026 Lusk, licence MIT ou Apache 2.0), puis disparaît en fondu. La barre suit le
  chargement de l'interface et des dictionnaires (français et synonymes, chargés en
  arrière-plan). Pas d'écran d'accueil sur macOS, pour les agents (`--control`), avec
  `--sample`, `--no-splash` ou `LUSK_NO_SPLASH=1`. Textes à modifier en haut de
  `crates/ui-egui/src/splash.rs`. Photo : `cargo run -p gridcraft-ui-egui --example splash --
  sortie.png`.
- 12 novembre 2026, **Enregistrer sous › OneDrive et Google Drive** (comme Lusk Texte) : à
  côté de « Parcourir… », un bouton par dossier OneDrive et par « Mon Drive » de Google Drive
  pour ordinateur trouvés sur l'ordinateur, grisé quand l'application n'est pas installée, avec
  une icône « ? » qui dit quoi installer. Le bouton ouvre la fenêtre d'enregistrement de Windows
  directement dans ce dossier (Lusk Tab n'a pas le navigateur intégré de Lusk Texte) ; vaut aussi
  pour les copies CSV, page web et PDF. Aucune connexion à un compte : l'application du service
  envoie le fichier en ligne. Code : `crates/ui-egui/src/cloud.rs`, `backstage.rs`
  (`cloud_buttons`), service `pick_save_in`.
- 12 novembre 2026, **menu Aide, page Mises à jour, modèles et palette en français** :
  - onglet **Aide** du ruban (après Automatiser) avec « Aide » (fenêtre d'aide en français :
    formules, raccourcis, fichiers, site, contact, signalement ; `dialogs/help.rs`) et « À propos
    de Lusk Tab », retirés d'Automatiser ;
  - **Fichier › Mises à jour**, en bas à gauche sous Options comme dans Lusk Texte : version
    installée, bouton « Rechercher des mises à jour », case « au démarrage » (enregistrée dans les
    préférences). Les mises à jour viennent maintenant de `declic29/LuskTab`
    (`UPDATE_APP` dans `lib.rs`, page dans `updates_ui.rs`) ;
  - **modèles de Fichier › Nouveau** en français : Budget familial, Comptes de l'association,
    Suivi des adhérents (`crates/engine/src/sample.rs`, données inventées) ;
  - **palette « Rechercher des commandes »** (loupe) en français : noms, menus et touches (Maj,
    Entrée…), recherche sans accents en français comme en anglais (`dialogs/command_search.rs`),
    190 libellés ajoutés au catalogue ;
  - vérification des fonctions : un test tape chacune des fonctions du moteur sous son nom
    français et contrôle qu'elle est calculée (jamais #NOM?) et que chaque nom français mène à
    la bonne fonction (`crates/engine/tests/it/french_functions.rs`).

## À faire (dans l'ordre du cahier des charges)

1. Fin de la marque avec les installateurs : fichiers Windows (`gridcraft.wxs`, dossier
   `ArtCraft\GridCraft`, éditeur), macOS (`package.sh`, nom de l'app, DMG : le fond
   `packaging/macos/dmg/background.svg` pointait vers l'ancienne icône SVG, à refaire),
   `sign.ps1`, `flatpak-oci.sh`.
2. Accueil avec le renard, OneDrive et Google Drive aussi dans Fichier › Ouvrir.
3. Page Confidentialité (après un audit réseau).
4. Installateurs en français et workflow « Publication » (repris de Lusk Texte).
5. Version 1.0.0.

## Façon de travailler avec Claude

- Claude travaille dans une copie du dépôt et **copie les fichiers modifiés sur le PC**
  (`C:\LuskTab`), en gardant leurs fins de ligne. **Philippe fait lui-même `git add -A`,
  `git commit` et `git push`.**
- Les fichiers de `.github\` sont envoyés en pièce jointe et copiés à la main ; les suppressions se
  font avec `git rm`.
- Claude ne peut pas lire les journaux de GitHub Actions : copier-coller le message d'erreur.
- Pour compiler dans l'espace de Claude, Cargo doit passer par le proxy :
  `CARGO_HTTP_PROXY="$HTTPS_PROXY" NO_PROXY="" no_proxy=""`.
