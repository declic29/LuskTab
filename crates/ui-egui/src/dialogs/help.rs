//! Lusk Tab : Aide › Aide. L'essentiel pour bien démarrer, en français : les formules, les
//! raccourcis utiles, les fichiers, et où trouver de l'aide (le site de l'association, le
//! signalement d'un problème).

use egui::RichText;

use super::Dialog;
use crate::SheetApp;
use crate::theme::{self, Tokens};

/// Le site de l'association.
pub const SITE: &str = "https://www.lusk.bzh";
/// Signaler un problème : les tickets du dépôt de Lusk Tab.
pub const ISSUES: &str = "https://github.com/declic29/LuskTab/issues";
/// L'adresse de contact de l'association.
pub const CONTACT: &str = "contact@lusk.bzh";

/// Les raccourcis présentés : (touches sous Windows et Linux, action).
const SHORTCUTS: [(&str, &str); 14] = [
    ("Ctrl+S", "Enregistrer"),
    ("F12", "Enregistrer sous"),
    ("Ctrl+Z / Ctrl+Y", "Annuler / Rétablir"),
    ("Ctrl+C, Ctrl+X, Ctrl+V", "Copier, couper, coller"),
    ("Ctrl+F / Ctrl+H", "Rechercher / Remplacer"),
    ("Ctrl+G ou F5", "Atteindre une cellule"),
    ("Ctrl+D", "Recopier vers le bas"),
    ("Alt+=", "Somme automatique"),
    ("Maj+F3", "Insérer une fonction"),
    ("F9", "Calculer maintenant"),
    ("F7", "Orthographe"),
    ("Maj+F7", "Dictionnaire des synonymes"),
    ("Ctrl+T", "Mettre sous forme de tableau"),
    ("Alt", "Afficher les lettres d'accès au ruban"),
];

fn heading(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(10.0);
    ui.label(RichText::new(text).font(theme::ui_bold(14.0)).color(t.text));
    ui.add_space(2.0);
}

fn para(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).font(theme::ui_font(12.5))).wrap());
}

/// La fenêtre d'aide.
pub(super) fn help(app: &mut SheetApp, ui: &mut egui::Ui, _d: &mut Dialog, open: &mut bool) {
    ui.set_width(560.0);
    egui::ScrollArea::vertical().max_height(440.0).auto_shrink([false, true]).show(ui, |ui| {
        para(
            ui,
            "Lusk Tab est un tableur libre et gratuit, porté par l'association Lusk. Il ouvre et enregistre les \
             classeurs Excel (.xlsx), les fichiers CSV et exporte en PDF. Tout reste sur votre ordinateur.",
        );
        heading(ui, "Écrire une formule");
        para(
            ui,
            "Une formule commence par =, par exemple =SOMME(B2:B10) ou =SI(C2>0;\"Oui\";\"Non\"). Les noms des \
             fonctions sont en français, comme dans Excel en français (SOMME, MOYENNE, SI, RECHERCHEV, NB.SI…), \
             les arguments se séparent par un point-virgule et les décimales s'écrivent avec une virgule (1,5). \
             Les noms anglais (SUM, IF…) sont aussi acceptés.",
        );
        para(ui, "Formules › Insérer une fonction (Maj+F3) liste toutes les fonctions, avec une aide pour chacune.");
        heading(ui, "Raccourcis utiles");
        egui::Grid::new("lusk_help_shortcuts").num_columns(2).spacing([18.0, 4.0]).striped(true).show(ui, |ui| {
            for (keys, what) in SHORTCUTS {
                ui.label(RichText::new(keys).font(theme::ui_bold(12.0)));
                ui.label(RichText::new(what).font(theme::ui_font(12.0)));
                ui.end_row();
            }
        });
        heading(ui, "Vos fichiers");
        para(
            ui,
            "Fichier › Nouveau propose un classeur vide et des modèles (budget familial, comptes de \
             l'association, suivi des adhérents). Fichier › Enregistrer sous enregistre aussi dans OneDrive ou \
             Google Drive quand leur application est installée sur l'ordinateur.",
        );
        heading(ui, "Obtenir de l'aide");
        para(
            ui,
            &format!(
                "Le site de l'association : {SITE}. Pour une question : {CONTACT}. Pour signaler un problème, joignez si \
                 possible le journal de Lusk Tab (dossier %APPDATA%\\LuskTab\\logs sous Windows)."
            ),
        );
    });
    ui.add_space(10.0);
    let mut url = None;
    ui.horizontal(|ui| {
        if ui.button("Site de l'association").clicked() {
            url = Some(SITE.to_string());
        }
        if ui.button("Signaler un problème").clicked() {
            url = Some(ISSUES.to_string());
        }
        if ui.button("Écrire à l'association").clicked() {
            url = Some(format!("mailto:{CONTACT}"));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(crate::i18n::tr(ui, "Close")).clicked() {
                *open = false;
            }
        });
    });
    if let Some(url) = url
        && let Some(open_url) = &app.services.open_url
    {
        open_url(&url);
    }
}
