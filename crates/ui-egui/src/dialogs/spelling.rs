use super::{Dialog, Json, SheetApp, json};

pub(super) fn spelling(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let r = d.result.clone().unwrap_or(Json::Null);
    if let Some(e) = r.get("error").and_then(Json::as_str) {
        ui.label(app.ui.language.tr_error(e));
        return;
    }
    let issues = r["issues"].as_array().cloned().unwrap_or_default();
    let Some(it) = issues.get(d.list_index) else {
        ui.label(crate::i18n::tr(ui, "The spelling check is complete for the entire sheet."));
        if ui.button(crate::i18n::tr(ui, "OK")).clicked() {
            *open = false;
        }
        return;
    };
    let word = it["word"].as_str().unwrap_or("").to_string();
    let cell = it["cell"].as_str().unwrap_or("").to_string();
    ui.label(crate::i18n::tr(ui, "Not in Dictionary ({cell}):").replace("{cell}", &cell));
    ui.label(egui::RichText::new(&word).strong().color(crate::theme::Tokens::get(ui.ctx()).danger));
    let sugg: Vec<String> =
        it["suggestions"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    ui.label(crate::i18n::tr(ui, "Suggestions:"));
    let mut pick = d.values.get("pick").and_then(Json::as_str).unwrap_or("").to_string();
    for s in &sugg {
        if ui.selectable_label(pick == *s, s).clicked() {
            pick = s.clone();
        }
    }
    if sugg.is_empty() {
        ui.label(egui::RichText::new(crate::i18n::tr(ui, "(No suggestions)")).italics());
    }
    d.values.insert("pick".into(), json!(pick.clone()));
    let _ = app.session.run("selection.set", json!({"cell": cell}));
    ui.horizontal(|ui| {
        if ui.button(crate::i18n::tr(ui, "Ignore Once")).clicked() {
            d.list_index += 1;
        }
        if ui.button(crate::i18n::tr(ui, "Add to Dictionary")).clicked() {
            let _ = app.session.run("review.addToDictionary", json!({"word": word}));
            d.list_index += 1;
        }
        if !pick.is_empty() {
            if ui.button(crate::i18n::tr(ui, "Change")).clicked() {
                app.run_or_alert("review.changeSpelling", json!({"cell": cell, "word": word, "to": pick}));
                d.list_index += 1;
            }
            if ui.button(crate::i18n::tr(ui, "Change All")).clicked() {
                app.run_or_alert("review.changeSpelling", json!({"word": word, "to": pick, "all": true}));
                d.list_index += 1;
            }
        }
    });
}

/// Review › Thesaurus (Shift+F7): the meanings of a word with their synonyms, in the dictionary
/// language. Clicking a synonym picks it, double-clicking looks it up, and Insert puts it in the
/// active cell in place of the word looked up.
pub(super) fn thesaurus(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let mut word = d.values.get("word").and_then(Json::as_str).unwrap_or("").to_string();
    let mut look_up = None;
    ui.horizontal(|ui| {
        let label = ui.label(crate::i18n::tr(ui, "Search:"));
        let field = ui.add(egui::TextEdit::singleline(&mut word).desired_width(220.0)).labelled_by(label.id);
        if (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button(crate::i18n::tr(ui, "Search")).clicked() {
            look_up = Some(word.clone());
        }
    });
    d.values.insert("word".into(), json!(word));
    let r = d.result.clone().unwrap_or(Json::Null);
    let mut pick = d.values.get("pick").and_then(Json::as_str).unwrap_or("").to_string();
    ui.add_space(4.0);
    egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
        if r["available"] == false {
            ui.label(egui::RichText::new(crate::i18n::tr(ui, "There is no thesaurus for this dictionary language.")).italics());
            return;
        }
        let meanings = r["meanings"].as_array().cloned().unwrap_or_default();
        if meanings.is_empty() {
            ui.label(egui::RichText::new(crate::i18n::tr(ui, "No synonyms found.")).italics());
        }
        for m in &meanings {
            ui.label(egui::RichText::new(m["part"].as_str().unwrap_or("")).strong());
            ui.horizontal_wrapped(|ui| {
                for s in m["synonyms"].as_array().into_iter().flatten().filter_map(Json::as_str) {
                    let resp = ui.selectable_label(pick == s, s);
                    if resp.double_clicked() {
                        look_up = Some(s.to_string());
                    } else if resp.clicked() {
                        pick = s.to_string();
                    }
                }
            });
            ui.add_space(4.0);
        }
    });
    if let Some(w) = look_up {
        d.values.insert("word".into(), json!(w));
        d.values.insert("pick".into(), json!(""));
        d.values.insert("looked".into(), json!(w));
        d.result = Some(app.session.run("review.thesaurus", json!({"word": w})).unwrap_or_else(|e| json!({"error": e})));
        return;
    }
    d.values.insert("pick".into(), json!(pick.clone()));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(!pick.is_empty(), egui::Button::new(crate::i18n::tr(ui, "Thesaurus|Insert"))).clicked() {
            let looked = d.values.get("looked").and_then(Json::as_str).unwrap_or("").to_string();
            let cell = app.session.active().map(|doc| doc.selection.active.a1()).unwrap_or_default();
            let text = app
                .session
                .active()
                .and_then(|doc| doc.wb.active().and_then(|sh| sh.cell(doc.selection.active)).map(|c| c.input_text()))
                .unwrap_or_default();
            let input = if !looked.is_empty() && text.contains(&looked) { text.replacen(&looked, &pick, 1) } else { pick.clone() };
            app.run_or_alert("cell.set", json!({"cell": cell, "input": input}));
            *open = false;
        }
        if ui.button(crate::i18n::tr(ui, "Close")).clicked() {
            *open = false;
        }
    });
}
