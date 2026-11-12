//! One background check at startup (and on Help ▸ Check for Updates…) and the modal prompt that
//! offers the new release: Update and Restart (direct self-update), Download or Later. The flow
//! itself is `craft_update::updater::Updater`.

use craft_update::CheckResultResponse;
use craft_update::updater::{Action, Status, Updater};

use crate::SheetApp;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) struct Runtime {
    updater: Updater,
    /// The check at start was asked for; `tick` starts it with the context.
    startup: bool,
    /// Help ▸ Check for Updates… was chosen; the next `tick` starts it.
    manual: bool,
}

impl Default for Runtime {
    fn default() -> Self {
        Self { updater: Updater::new(&crate::UPDATE_APP, VERSION).with_translator(translate), startup: false, manual: false }
    }
}

/// Lusk Tab : les fenêtres de mise à jour (la proposition au démarrage et le résultat d'une
/// recherche) dans la langue de l'interface. Leurs textes anglais sont les clés du catalogue,
/// leurs `{valeurs}` remplies après la traduction.
fn translate(text: &str, args: &[(&str, &str)]) -> String {
    craft_update::translate::fill(crate::i18n::tr_now(text), args)
}

/// Ask for a check; the next frame's `tick` starts it. A manual one reports every outcome.
pub(crate) fn start(app: &mut SheetApp, manual: bool) {
    if manual {
        app.updates_rt.manual = true;
    } else {
        app.updates_rt.startup = true;
    }
}

pub(crate) fn tick(app: &mut SheetApp, ctx: &egui::Context) {
    let rt = &mut app.updates_rt;
    if std::mem::take(&mut rt.manual) {
        rt.updater.check_now(ctx);
    } else if rt.startup {
        rt.updater.check_at_start(ctx);
    }
    rt.updater.tick(ctx);
}

/// Starts the helper of a ready update; it installs once the process has exited.
pub(crate) fn start_pending_update(app: &mut SheetApp) {
    app.updates_rt.updater.start_pending_update();
}

/// Whether the check result window shows: for every status but idle (a requested check, and the
/// first start after an update, which reports that it installed or why it failed), and never
/// over the startup prompt.
fn result_window_shows(status: &Status, prompt_open: bool) -> bool {
    !prompt_open && !matches!(status, Status::Idle)
}

pub(crate) fn show(app: &mut SheetApp, ctx: &egui::Context) {
    // The close for a ready update waits until no workbook holds unsaved work.
    let clean = !app.session.documents().iter().any(|d| d.is_dirty());
    match app.updates_rt.updater.show_if(ctx, clean) {
        Action::OpenUrl(url) => {
            if let Some(open) = &app.services.open_url {
                open(&url);
            }
        }
        // Quit closes every workbook window. The install helper starts once the app exits.
        Action::Close => app.request_quit(ctx),
        Action::None => {}
    }
    // Lusk Tab : Fichier › Mises à jour montre le résultat sur la page, pas dans une fenêtre.
    let on_page = app.backstage.as_ref().is_some_and(|b| b.page == crate::backstage::Page::Updates);
    let rt = &mut app.updates_rt;
    if !on_page && result_window_shows(rt.updater.status(), rt.updater.prompt_open()) {
        match craft_update::CheckResultDialog::new(&crate::UPDATE_APP, VERSION).with_translator(translate).show(ctx, rt.updater.status()) {
            CheckResultResponse::Close => rt.updater.dismiss_status(),
            CheckResultResponse::CheckAgain => rt.updater.check_now(ctx),
            // The next `show_if` asks the window to close once nothing is unsaved.
            CheckResultResponse::RestartToUpdate => rt.updater.restart_to_update(),
            CheckResultResponse::None => {}
        }
    }
}

/// Lusk Tab (repris de Lusk Texte) : Fichier › Mises à jour. La version installée, un bouton
/// qui cherche une version plus récente sur les publications de l'association (GitHub), et la
/// recherche au démarrage. Une version plus récente ouvre la fenêtre de mise à jour, qui la
/// télécharge, vérifie son empreinte et l'installe quand tous les classeurs sont enregistrés.
pub(crate) fn page(app: &mut SheetApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let text = |ui: &mut egui::Ui, s: &str| {
        ui.add(egui::Label::new(egui::RichText::new(s).font(crate::theme::ui_font(13.0)).color(t.text)).wrap());
    };
    ui.label(egui::RichText::new(format!("Lusk Tab {VERSION}")).font(crate::theme::ui_bold(16.0)).color(t.text));
    text(ui, "Version installée sur cet ordinateur.");
    ui.add_space(20.0);
    let status = app.updates_rt.updater.status().clone();
    let checking = matches!(status, Status::Checking);
    ui.horizontal(|ui| {
        if ui.add_enabled(!checking, egui::Button::new("Rechercher des mises à jour").min_size(egui::vec2(0.0, 30.0))).clicked() {
            start(app, true);
        }
        if checking {
            ui.add(egui::Spinner::new());
            ui.label("Recherche en cours…");
        }
    });
    // Le résultat, avec les mots de la page plutôt que ceux de la fenêtre.
    let outcome = match &status {
        Status::UpToDate => Some(format!("Vous avez la dernière version de Lusk Tab ({VERSION}).")),
        Status::Failed(e) if e == "no releases yet" => Some("Aucune version n'a encore été publiée sur GitHub.".to_string()),
        Status::Failed(e) => Some(format!("La recherche n'a pas abouti. Vérifiez la connexion à Internet, puis réessayez.\n(Détail : {e})")),
        Status::Installing => Some("Une nouvelle version est en cours de téléchargement et d'installation.".to_string()),
        Status::Ready => Some("Une nouvelle version est prête : elle s'installera à la fermeture de Lusk Tab.".to_string()),
        Status::Managed => Some("Les mises à jour de Lusk Tab sont gérées par un autre programme sur cet ordinateur.".to_string()),
        Status::Updated(v) => Some(format!("Lusk Tab a été mis à jour en version {v}.")),
        Status::InstallFailed(e) => Some(format!("La dernière mise à jour n'a pas pu être installée : {e}")),
        Status::Idle | Status::Checking => None,
    };
    if let Some(outcome) = outcome {
        ui.add_space(8.0);
        text(ui, &outcome);
    }
    if matches!(status, Status::Ready) && ui.button("Redémarrer pour mettre à jour").clicked() {
        app.updates_rt.updater.restart_to_update();
    }
    ui.add_space(20.0);
    let mut at_start = app.ui.check_updates_at_start;
    if ui.checkbox(&mut at_start, "Rechercher automatiquement les mises à jour au démarrage").changed() {
        // Enregistré avec les autres préférences à la fermeture ; vaut au prochain démarrage.
        app.ui.check_updates_at_start = at_start;
    }
    ui.add_space(20.0);
    text(
        ui,
        "Les nouvelles versions sont publiées par l'association Lusk. Quand une version plus récente existe, Lusk Tab \
         propose de la télécharger et vérifie son empreinte avant de l'installer. L'installation attend que tous vos \
         classeurs soient enregistrés.",
    );
    ui.add_space(8.0);
    text(ui, "Seul le numéro de la dernière version est demandé : aucun classeur ni aucune donnée personnelle n'est envoyé.");
    if ui.link("Vos données et votre vie privée").clicked() {
        app.open_dialog("privacy", serde_json::json!({}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_window_shows_every_status_but_idle() {
        let shown = [
            Status::Checking,
            Status::UpToDate,
            Status::Failed("no network".into()),
            Status::Installing,
            Status::Ready,
            Status::Managed,
            Status::Updated("0.2.0".into()),
            Status::InstallFailed("disk full".into()),
        ];
        for status in &shown {
            assert!(result_window_shows(status, false), "{status:?}");
            assert!(!result_window_shows(status, true), "{status:?} over the prompt");
        }
        assert!(!result_window_shows(&Status::Idle, false));
    }

    #[test]
    fn update_windows_speak_french() {
        let ctx = egui::Context::default();
        crate::i18n::set_current(&ctx, crate::i18n::Language::Fr);
        assert_eq!(super::translate("Later", &[]), "Plus tard");
        assert_eq!(
            super::translate("{app} {version} is the newest version available.", &[("app", "Lusk Tab"), ("version", "1.0.0")]),
            "Lusk Tab 1.0.0 est la version la plus récente."
        );
        assert!(super::translate(craft_update::updater::READY_TEXT, &[("app", "Lusk Tab")]).contains("Fermez Lusk Tab"));
        crate::i18n::set_current(&ctx, crate::i18n::Language::En);
    }

    /// Les fenêtres de mise à jour en français, dessinées en PNG pour les relire :
    /// `LUSK_UPDATE_SHOTS=<dossier> cargo test -p gridcraft-ui-egui update_windows_pictures`.
    /// Sans la variable, le test ne fait rien.
    #[test]
    fn update_windows_pictures() {
        use craft_update::dialog::{CheckResultDialog, StartupPrompt};
        use craft_update::release::Release;
        let Some(dir) = std::env::var_os("LUSK_UPDATE_SHOTS").map(std::path::PathBuf::from) else { return };
        std::fs::create_dir_all(&dir).unwrap();
        let states: [(&str, Option<Status>); 5] = [
            ("recherche", Some(Status::Checking)),
            ("a-jour", Some(Status::UpToDate)),
            ("echec", Some(Status::Failed("délai dépassé".into()))),
            ("prete", Some(Status::Ready)),
            ("proposition", None),
        ];
        for (name, status) in states {
            let dialog = CheckResultDialog::new(&crate::UPDATE_APP, "0.19.1").with_translator(super::translate);
            let mut prompt = StartupPrompt::new(
                &crate::UPDATE_APP,
                Release { version: craft_update::version::parse("1.0.0"), tag: "v1.0.0".into(), ..Release::default() },
                "0.19.1",
            )
            .with_translator(super::translate);
            let mut h = egui_kittest::Harness::builder()
                .with_size(egui::vec2(620.0, 380.0))
                .with_context_setup(|ctx| crate::SheetApp::setup_context_for_language(ctx, false, crate::i18n::Language::Fr))
                .wgpu()
                .build_ui(|ui| {
                    crate::i18n::set_current(ui.ctx(), crate::i18n::Language::Fr);
                    match &status {
                        Some(status) => {
                            dialog.show(ui.ctx(), status);
                        }
                        None => {
                            prompt.show(ui.ctx());
                        }
                    }
                });
            h.run_steps(4);
            h.render().unwrap().save(dir.join(format!("maj-{name}.png"))).unwrap();
        }
    }

    /// Lusk Tab : la mise à jour reconnaît les fichiers que publie le workflow « Publication »
    /// (`Lusk-Tab-<version>-…`), mais pas leurs copies sans numéro de version (les liens
    /// « dernière version » du README) ni l'archive de l'outil en ligne de commande.
    #[test]
    fn release_files_named_lusk_tab_are_found() {
        use craft_update::platform::parse_asset_name;
        let stems: Vec<&str> = crate::UPDATE_APP.stems().collect();
        let a = parse_asset_name("Lusk-Tab-1.0.0-windows-x64.msi", &stems).unwrap();
        assert_eq!((a.version.to_string().as_str(), a.token.as_str()), ("1.0.0", "windows-x64"));
        assert!(parse_asset_name("Lusk-Tab-1.0.0-macos-universal.dmg", &stems).is_some());
        assert!(parse_asset_name("Lusk-Tab-1.0.0-linux-x86_64.AppImage", &stems).is_some());
        assert!(parse_asset_name("Lusk-Tab-1.0.0-linux-x86_64.deb", &stems).is_some());
        assert!(parse_asset_name("Lusk-Tab-Windows.msi", &stems).is_none());
        assert!(parse_asset_name("Lusk-Tab-cli-1.0.0-macos-universal.zip", &stems).is_none());
    }
}
