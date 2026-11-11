//! Spelling (Lusk Tab): checks text cells with the dictionaries built into the program (French,
//! Breton and English, from Lusk Texte's `gridcraft-proof`), in the language chosen in
//! File › Options (`spellingLanguage`, French by default), plus a user dictionary kept in the
//! engine preferences. The thesaurus still reads the system word list (`/usr/share/dict/words`).

use std::collections::HashSet;
use std::sync::OnceLock;

use gridcraft_core::{RangeRef, Value};
use gridcraft_proof::ProofLang;
use serde_json::{Value as Json, json};

use super::{
    CommandSpec, EngineError, Result, Session, always, bad, bool_param, cell_param, cmd, has_doc, ok, str_param, target_range, target_sheet,
};

/// The dictionary languages offered in File › Options: tag and name.
pub const SPELLING_LANGUAGES: [(&str, &str); 4] =
    [("fr", "Français"), ("br", "Brezhoneg"), ("en-US", "English (United States)"), ("en-GB", "English (United Kingdom)")];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "review.spelling", "Spelling", ["Review", "Proofing"], Some("F7"), "{range?, sheet?} → misspelled words with cells and suggestions", has_doc, spelling),
        cmd!(noundo "review.addToDictionary", "Add to Dictionary", [], None, "{word}", has_doc, add_word),
        cmd!("review.changeSpelling", "Change", [], None, "{cell, word, to, all?: bool}", has_doc, change),
        cmd!(query "review.thesaurus", "Thesaurus", ["Review", "Proofing"], None, "{word} → related words (words sharing the stem in the dictionary)", has_doc, thesaurus),
        cmd!(query "review.spellingLanguage", "Dictionary Language", [], None, "{lang?: \"fr\" | \"br\" | \"en-US\" | \"en-GB\"} → the language spelling is checked in", always, spelling_language),
    ]
}

/// The system word list, for the thesaurus only.
fn dictionary() -> &'static HashSet<String> {
    static DICT: OnceLock<HashSet<String>> = OnceLock::new();
    DICT.get_or_init(|| {
        #[cfg(target_arch = "wasm32")]
        {
            HashSet::new()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut set = HashSet::new();
            for path in ["/usr/share/dict/words", "/usr/share/dict/american-english", "/usr/share/dict/british-english"] {
                if let Ok(text) = std::fs::read_to_string(path) {
                    for w in text.lines().take(2_000_000) {
                        set.insert(w.trim().to_lowercase());
                    }
                    break;
                }
            }
            set
        }
    })
}

/// The language spelling is checked in.
fn proof_lang(s: &Session) -> ProofLang {
    ProofLang::from_tag(Some(&s.prefs.spelling_language))
}

fn in_user_dictionary(s: &Session, w: &str) -> bool {
    s.prefs.user_dictionary.iter().any(|u| u.to_lowercase() == w.to_lowercase())
}

/// Suggestions for a misspelled word, best first.
fn suggestions(w: &str, lang: ProofLang) -> Vec<String> {
    gridcraft_proof::suggest_in(w, 6, lang)
}

fn spelling(s: &mut Session, p: &Json) -> Result<Json> {
    let lang = proof_lang(s);
    if !lang.is_proofed() {
        return Err(EngineError::Other(format!("No dictionary for the language “{}”.", s.prefs.spelling_language)));
    }
    let si = target_sheet(s, p)?;
    let d = s.doc()?;
    let sh = d.wb.sheet(si).ok_or(EngineError::NoDocument)?;
    let scope = match str_param(p, "range") {
        Some(_) => target_range(s, p)?,
        None => sh.used_range().unwrap_or(RangeRef::cell(d.selection.active)),
    };
    let mut issues = Vec::new();
    for (c, cell) in sh.cells.iter_range(scope) {
        if cell.formula.is_some() {
            continue;
        }
        let Value::Text(t) = &cell.value else { continue };
        for issue in gridcraft_proof::check_spelling_in(t, lang) {
            let Some(w) = t.get(issue.start..issue.end) else { continue };
            // Single letters and words in capitals (acronyms, codes) are left alone.
            if w.chars().count() < 2 || w.chars().all(|ch| !ch.is_lowercase()) || in_user_dictionary(s, w) {
                continue;
            }
            issues.push(json!({"cell": c.a1(), "word": w, "suggestions": suggestions(w, lang)}));
        }
        if issues.len() >= 500 {
            break;
        }
    }
    Ok(json!({"issues": issues, "done": issues.is_empty()}))
}

fn spelling_language(s: &mut Session, p: &Json) -> Result<Json> {
    if let Some(tag) = str_param(p, "lang") {
        let Some((tag, _)) = SPELLING_LANGUAGES.iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)) else {
            return Err(bad("review.spellingLanguage", "`lang` must be fr, br, en-US or en-GB"));
        };
        s.prefs.spelling_language = (*tag).to_string();
    }
    Ok(json!({"lang": s.prefs.spelling_language}))
}

fn add_word(s: &mut Session, p: &Json) -> Result<Json> {
    let w = str_param(p, "word").ok_or_else(|| bad("review.addToDictionary", "missing `word`"))?.to_string();
    if !s.prefs.user_dictionary.iter().any(|u| u.eq_ignore_ascii_case(&w)) {
        s.prefs.user_dictionary.push(w);
    }
    ok()
}

fn change(s: &mut Session, p: &Json) -> Result<Json> {
    let word = str_param(p, "word").ok_or_else(|| bad("review.changeSpelling", "missing `word`"))?.to_string();
    let to = str_param(p, "to").ok_or_else(|| bad("review.changeSpelling", "missing `to`"))?.to_string();
    if bool_param(p, "all").unwrap_or(false) {
        return s.execute("edit.replace", json!({"what": word, "with": to, "matchCase": true, "all": true}));
    }
    let c = cell_param(s, p, "cell")?.ok_or_else(|| bad("review.changeSpelling", "missing `cell`"))?;
    let text = s.doc()?.wb.active().and_then(|sh| sh.cell(c)).map(|x| x.input_text()).unwrap_or_default();
    s.execute("cell.set", json!({"cell": c.a1(), "input": text.replacen(&word, &to, 1)}))
}

fn thesaurus(_: &mut Session, p: &Json) -> Result<Json> {
    let w = str_param(p, "word").unwrap_or("").to_lowercase();
    if w.len() < 3 {
        return Ok(json!([]));
    }
    let stem: String = w.chars().take(w.chars().count().saturating_sub(2).max(3)).collect();
    let mut related: Vec<&String> = dictionary().iter().filter(|d| d.starts_with(&stem) && **d != w && d.len() <= w.len() + 4).collect();
    related.sort();
    related.truncate(20);
    Ok(json!(related))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &mut Session) -> Vec<String> {
        let out = s.execute("review.spelling", json!({})).unwrap();
        out["issues"].as_array().unwrap().iter().map(|i| i["word"].as_str().unwrap().to_string()).collect()
    }

    fn session(texts: &[&str]) -> Session {
        let mut s = Session::new();
        s.new_workbook();
        for (i, t) in texts.iter().enumerate() {
            s.execute("cell.set", json!({"cell": format!("A{}", i + 1), "input": t})).unwrap();
        }
        s
    }

    #[test]
    fn french_is_checked_by_default_with_accents() {
        let mut s = session(&["Facture de l’été", "Réunion à Brest le lundi", "Dépenses mensuelles", "Ceci est une fôte", "TVA"]);
        assert_eq!(s.prefs.spelling_language, "fr");
        assert_eq!(words(&mut s), ["fôte"]);
        let out = s.execute("review.spelling", json!({})).unwrap();
        assert!(out["issues"][0]["suggestions"].as_array().unwrap().iter().any(|w| w == "fête"), "{out}");
    }

    #[test]
    fn breton_and_english_dictionaries_are_built_in() {
        let mut s = session(&["Demat d'an holl", "Kenavo", "The monthly budgte"]);
        s.execute("review.spellingLanguage", json!({"lang": "br"})).unwrap();
        let br = words(&mut s);
        assert!(!br.iter().any(|w| w == "Demat" || w == "Kenavo"), "{br:?}");
        s.execute("review.spellingLanguage", json!({"lang": "en-US"})).unwrap();
        assert_eq!(words(&mut s).last().map(String::as_str), Some("budgte"));
        assert!(s.execute("review.spellingLanguage", json!({"lang": "xx"})).is_err());
        assert_eq!(s.execute("review.spellingLanguage", json!({})).unwrap()["lang"], "en-US");
    }

    #[test]
    fn added_words_are_no_longer_flagged() {
        let mut s = session(&["Le projet Kerlusk"]);
        assert_eq!(words(&mut s), ["Kerlusk"]);
        s.execute("review.addToDictionary", json!({"word": "kerlusk"})).unwrap();
        assert!(words(&mut s).is_empty());
    }
}
