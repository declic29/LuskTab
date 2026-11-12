use super::{Dialog, Key, SheetApp, json, vec2};

/// Shows enabled commands and their shortcuts from engine metadata.
pub(super) fn command_search(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool) {
    let (down, up, home, end, pgup, pgdn) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
            i.consume_key(egui::Modifiers::CTRL, Key::Home),
            i.consume_key(egui::Modifiers::CTRL, Key::End),
            i.consume_key(egui::Modifiers::NONE, Key::PageUp),
            i.consume_key(egui::Modifiers::NONE, Key::PageDown),
        )
    });
    let width = ui.available_width();
    let r = ui.add(
        egui::TextEdit::singleline(&mut d.search).desired_width(width).hint_text(crate::i18n::tr(ui, "Type a command, e.g. freeze, chart, bold…")),
    );

    if !d.focus_initialized {
        r.request_focus();
        d.focus_initialized = true;
    }

    let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
    let lang = crate::i18n::current(ui.ctx());
    // Lusk Tab : la palette affiche les commandes dans la langue de l'interface et cherche sans
    // tenir compte des accents ni des majuscules, dans le libellé traduit comme dans l'anglais.
    let q = fold(d.search.trim());
    let cmds: Vec<(String, String, String, Option<String>)> = app
        .session
        .commands()
        .into_iter()
        .filter(|c| c.enabled)
        .filter_map(|c| {
            let label = lang.tr(c.label).to_string();
            let menu = c.menu.iter().map(|m| lang.tr(m).to_string()).collect::<Vec<_>>().join(" › ");
            let hit = q.is_empty() || [label.as_str(), c.label, c.id, menu.as_str()].iter().any(|t| fold(t).contains(&q));
            hit.then(|| (c.id.to_string(), label, menu, c.shortcut_for_platform(mac).map(|k| keys_label(lang, &k))))
        })
        .take(40)
        .collect();

    let last = cmds.len().saturating_sub(1);

    let row_h = ui.spacing().interact_size.y.max(20.0);
    let row_step = row_h + ui.spacing().item_spacing.y;

    const LIST_HEIGHT: f32 = 320.0;

    let page_size = ((LIST_HEIGHT / row_step).floor() as usize).saturating_sub(1).max(1);

    if down {
        d.list_index = (d.list_index + 1).min(last);
    }
    if up {
        d.list_index = d.list_index.saturating_sub(1);
    }
    if pgdn {
        d.list_index = (d.list_index + page_size).min(last);
    }
    if pgup {
        d.list_index = d.list_index.saturating_sub(page_size);
    }
    if home {
        d.list_index = 0;
    }
    if end {
        d.list_index = last;
    }

    d.list_index = d.list_index.min(last);

    if cmds.is_empty() {
        ui.label(egui::RichText::new(crate::i18n::tr(ui, "No command matches.")).weak());
    }
    egui::ScrollArea::vertical().max_height(LIST_HEIGHT).auto_shrink([true, false]).show(ui, |ui| {
        let scroll_area_width = ui.available_width();
        for (i, (id, label, menu, shortcut)) in cmds.iter().enumerate() {
            let text = match shortcut {
                Some(keys) => format!("{label}  ({keys})    {menu}"),
                None => format!("{label}    {menu}"),
            };
            let r = ui.add(egui::Button::selectable(i == d.list_index, text.clone()).truncate().min_size(vec2(scroll_area_width, 20.0)));

            if (down || up || home || end || pgup || pgdn) && i == d.list_index && !ui.clip_rect().contains_rect(r.rect) {
                r.scroll_to_me(Some(if down || end || pgdn { egui::Align::Max } else { egui::Align::Min }));
            }

            if r.clicked() {
                d.list_index = i;
                *confirm = true;
            }

            r.on_hover_text(format!("{text}\n{id}"));
        }
    });
    if ui.input(|i| i.key_pressed(Key::Enter)) {
        *confirm = true;
    }
    if *confirm && let Some((id, ..)) = cmds.get(d.list_index) {
        let id = id.clone();
        app.dialog = None;
        app.run_or_alert(&id, json!({}));
    }
}

/// Minuscules sans accents, pour chercher « reglages » comme « Réglages ».
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ÿ' => 'y',
            other => other,
        })
        .collect()
}

/// Les noms des touches en français (Maj, Entrée, Espace…) quand l'interface est en français.
fn keys_label(lang: crate::i18n::Language, keys: &str) -> String {
    if lang != crate::i18n::Language::Fr {
        return keys.to_string();
    }
    keys.split(", ").map(|chord| chord.split('+').map(key_name).collect::<Vec<_>>().join("+")).collect::<Vec<_>>().join(", ")
}

fn key_name(k: &str) -> &str {
    match k {
        "Shift" => "Maj",
        "Enter" => "Entrée",
        "Space" => "Espace",
        "Delete" | "Del" => "Suppr",
        "Backspace" => "Retour arrière",
        "Insert" | "Ins" => "Inser",
        "Home" => "Début",
        "End" => "Fin",
        "PageUp" | "PgUp" => "Pg.Préc",
        "PageDown" | "PgDn" => "Pg.Suiv",
        "Escape" | "Esc" => "Échap",
        "Up" => "Haut",
        "Down" => "Bas",
        "Left" => "Gauche",
        "Right" => "Droite",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::fold;

    #[test]
    fn search_ignores_accents_and_case() {
        assert_eq!(fold("Réglages À Côté"), "reglages a cote");
        assert!(fold("Insérer une fonction").contains(&fold("inserer")));
    }

    #[test]
    fn shortcuts_use_french_key_names() {
        use crate::i18n::Language;
        assert_eq!(super::keys_label(Language::Fr, "Ctrl+Shift+Enter"), "Ctrl+Maj+Entrée");
        assert_eq!(super::keys_label(Language::Fr, "Ctrl++"), "Ctrl++");
        assert_eq!(super::keys_label(Language::En, "Shift+Space"), "Shift+Space");
    }
}
