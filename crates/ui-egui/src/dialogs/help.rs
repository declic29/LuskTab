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
        if ui.button("Confidentialité").clicked() {
            app.open_dialog("privacy", serde_json::json!({}));
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

/// Lusk Tab : Aide › Confidentialité. Tout ce qui peut passer par Internet (relevé dans le code
/// le 12 novembre 2026) et ce qui reste sur l'ordinateur.
pub(super) fn privacy(_app: &mut SheetApp, ui: &mut egui::Ui, _d: &mut Dialog, open: &mut bool) {
    ui.set_width(560.0);
    egui::ScrollArea::vertical().max_height(440.0).auto_shrink([false, true]).show(ui, |ui| {
        para(
            ui,
            "Ce que vous saisissez vous appartient. Lusk Tab fonctionne entièrement sur votre ordinateur : pas de \
             compte à créer, pas de publicité, pas de mesure d'audience, et aucun classeur n'est jamais envoyé.",
        );
        heading(ui, "Ce qui reste sur votre ordinateur");
        para(
            ui,
            "Vos classeurs et leurs enregistrements automatiques, vos préférences, votre dictionnaire personnel et le \
             journal de Lusk Tab (dossier %APPDATA%\\LuskTab sous Windows). Les calculs, la vérification de \
             l'orthographe, le dictionnaire des synonymes et les exports (PDF, CSV, page web) se font sur place, sans \
             connexion à Internet.",
        );
        heading(ui, "Les seuls moments où Lusk Tab utilise Internet");
        para(
            ui,
            "• Mises à jour : Lusk Tab demande à GitHub, où l'association Lusk publie ses versions (dépôt \
             declic29/LuskTab), le numéro de la dernière version. Il le fait au démarrage si la case est cochée, et \
             quand vous cliquez sur « Rechercher des mises à jour ». Une nouvelle version n'est téléchargée que si vous \
             l'acceptez. Aucun classeur ni aucune donnée personnelle n'est envoyé ; comme tout site web, GitHub voit \
             l'adresse de votre connexion. Pour ne plus rechercher au démarrage, décochez la case dans \
             Fichier › Mises à jour.",
        );
        para(
            ui,
            "• Liens : un lien hypertexte d'un classeur s'ouvre dans votre navigateur seulement après que vous avez \
             confirmé son adresse.",
        );
        para(
            ui,
            "• Aide : les boutons « Site de l'association », « Signaler un problème » et « Écrire à l'association » \
             ouvrent votre navigateur ou votre messagerie.",
        );
        heading(ui, "Ce qui n'utilise jamais Internet");
        para(
            ui,
            "Les fonctions SERVICEWEB et FILTRE.XML ne sont pas disponibles : elles renvoient #VALEUR! au lieu \
             d'interroger un site. Les cellules Python s'exécutent dans un espace isolé, sans réseau (sous Linux et \
             macOS) ; sous Windows, elles ne s'exécutent pas. Le pilotage par des scripts ou des assistants est \
             désactivé, sauf si Lusk Tab est lancé avec l'option --control : même alors, il n'écoute que cet \
             ordinateur, et seuls les programmes qui connaissent sa clé peuvent s'en servir.",
        );
        heading(ui, "Avant de partager un classeur");
        para(
            ui,
            "Un classeur peut contenir plus que ses chiffres : le nom de son auteur (Fichier › Informations), celui de \
             l'auteur de chaque commentaire et de chaque note. Le nom inscrit est celui de Fichier › Options › Général \
             › Nom d'utilisateur (« Utilisateur » par défaut).",
        );
    });
    ui.add_space(10.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.button(crate::i18n::tr(ui, "Close")).clicked() {
            *open = false;
        }
    });
}
