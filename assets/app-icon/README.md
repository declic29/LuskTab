# Icône de Lusk Tab

L'icône reprend le logo de Lusk Tab fourni par l'association Lusk : des crochets verts, deux
barres « // » et le mot « TAB » en dégradé. Le logo (`lusk-tab-logo.png`, fond transparent) est
centré sur un carré transparent ; toutes les tailles en sont tirées.

Pour changer d'icône : remplacer `lusk-tab-logo.png`, lancer `packaging/icons.sh`, puis mettre à
jour les lignes de `ATTRIBUTION.md` si la provenance change.

| Fichier | Usage |
|---|---|
| `lusk-tab-logo.png` | Logo d'origine, image source |
| `gridcraft-1024.png` | Logo centré sur un carré de 1024 px |
| `gridcraft-macos-512.png` | Icône de fenêtre et du Dock sous macOS (grille d'Apple, marge transparente) |
| `gridcraft.icns` | Icône de l'application macOS |
| `gridcraft.ico` | Icône du programme et de l'installateur Windows (16 à 256 px) |
| `hicolor/<taille>/apps/bzh.lusk.tab.png` | Thème d'icônes Linux, 16 à 512 px |

L'identifiant de l'application est `bzh.lusk.tab` : noms des icônes Linux, identifiant Wayland et
`packaging/linux/bzh.lusk.tab.desktop`. Les fichiers gardent le nom interne `gridcraft`, comme le
programme lui-même.
