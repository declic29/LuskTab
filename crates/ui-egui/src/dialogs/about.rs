use super::Dialog;

/// Help ▸ About Lusk Tab: the app, its publisher (the Lusk association) and its GridCraft origin.
pub(super) fn about(ui: &mut egui::Ui, _d: &mut Dialog) {
    ui.heading("Lusk Tab");
    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
    ui.add_space(4.0);
    ui.label("Tableur libre et gratuit, porté par l'association Lusk (loi 1901).");
    ui.label("© 2026 Lusk. Logiciel libre sous licence MIT ou Apache 2.0.");
    ui.label("Dérivé de GridCraft, © 2026 ArtCraft Team and the GridCraft contributors.");
    ui.add_space(4.0);
    ui.hyperlink_to("lusk.bzh", "https://www.lusk.bzh");
    ui.hyperlink_to("github.com/declic29/LuskTab", "https://github.com/declic29/LuskTab");
}
