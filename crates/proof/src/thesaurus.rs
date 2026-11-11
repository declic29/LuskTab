//! Thesaurus (Lusk Tab): synonyms by meaning, from the myThes files LibreOffice uses.
//!
//! - **French**: the Dicollecte thesaurus 2.3 (`assets/thesaurus/fr/thes_fr.dat`, LGPL 2.1 or
//!   later), built into the program as it is.
//! - **English**: the WordNet thesaurus (`assets/thesaurus/en/th_en_US_v2.dat.gz`, WordNet
//!   licence), built in compressed with gzip.
//!
//! A file with the same name (`thes_fr.dat`, `th_en_US_v2.dat`) in a dictionary folder (a
//! `dictionnaires` folder next to the program, the folder set with
//! [`crate::set_dictionaries_folder`], …) replaces the built-in one, so the thesaurus can be
//! updated or changed without rebuilding Lusk Tab.
//!
//! The myThes format: a first line naming the encoding (UTF-8 here), then for each entry a line
//! `word|count` followed by `count` lines `(part of speech)|synonym|synonym…`.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::ProofLang;

const EMBEDDED_FR: &str = include_str!("../../../assets/thesaurus/fr/thes_fr.dat");
static EMBEDDED_EN_GZ: &[u8] = include_bytes!("../../../assets/thesaurus/en/th_en_US_v2.dat.gz");

/// One meaning of a word: its part of speech as the thesaurus writes it (`Nom`, `Adjectif`,
/// `noun`…) and the words with that meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meaning {
    pub part: String,
    pub synonyms: Vec<String>,
}

/// A loaded thesaurus: its text and, for each lowercase headword, the byte offset of its entry.
struct Thesaurus {
    text: String,
    index: HashMap<String, usize>,
}

impl Thesaurus {
    fn new(text: String) -> Thesaurus {
        let mut index = HashMap::new();
        let mut offset = 0;
        let mut lines = text.split_inclusive('\n');
        // The encoding line.
        if let Some(first) = lines.next() {
            offset += first.len();
        }
        for line in lines {
            let start = offset;
            offset += line.len();
            if line.starts_with('(') || line.starts_with('-') {
                continue;
            }
            if let Some((word, count)) = line.trim_end().rsplit_once('|')
                && count.parse::<usize>().is_ok()
            {
                index.entry(word.to_lowercase()).or_insert(start);
            }
        }
        Thesaurus { text, index }
    }

    fn meanings(&self, word: &str) -> Option<Vec<Meaning>> {
        let start = *self.index.get(word)?;
        let mut lines = self.text.get(start..)?.lines();
        let count = lines.next()?.rsplit_once('|')?.1.parse::<usize>().ok()?;
        let mut out = Vec::new();
        for line in lines.take(count) {
            let mut parts = line.split('|');
            let part = parts.next().unwrap_or("").trim().trim_start_matches('(').trim_end_matches(')').to_string();
            let mut synonyms: Vec<String> = Vec::new();
            for s in parts {
                let Some(s) = clean(s) else { continue };
                if !s.eq_ignore_ascii_case(word) && !synonyms.iter().any(|x| x == &s) {
                    synonyms.push(s);
                }
            }
            if !synonyms.is_empty() {
                out.push(Meaning { part, synonyms });
            }
        }
        Some(out)
    }
}

/// A synonym as shown, or `None` for an entry that isn't one: WordNet marks broader words
/// `(generic term)` and opposites `(antonym)`; a `(similar term)` or `(related term)` is kept
/// without its mark.
fn clean(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() || s.ends_with("(generic term)") || s.ends_with("(antonym)") {
        return None;
    }
    Some(s.trim_end_matches("(similar term)").trim_end_matches("(related term)").trim().to_string())
}

/// A replacement file named `name` in a dictionary folder, if there is one.
fn replacement(name: &str) -> Option<String> {
    crate::hunspell::search_dirs(None).into_iter().map(|d| d.join(name)).find(|p| p.is_file()).and_then(|p| match std::fs::read_to_string(&p) {
        Ok(text) => Some(text),
        Err(e) => {
            log::warn!("thesaurus {}: {e}", p.display());
            None
        }
    })
}

/// The data of a gzip file (RFC 1952), `None` if it isn't one.
fn gunzip(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 18 || data[0] != 0x1f || data[1] != 0x8b || data[2] != 8 {
        return None;
    }
    let flags = data[3];
    let mut at = 10;
    if flags & 0x04 != 0 {
        let extra = usize::from(u16::from_le_bytes([*data.get(at)?, *data.get(at + 1)?]));
        at += 2 + extra;
    }
    for bit in [0x08, 0x10] {
        if flags & bit != 0 {
            at += data.get(at..)?.iter().position(|b| *b == 0)? + 1;
        }
    }
    if flags & 0x02 != 0 {
        at += 2;
    }
    miniz_oxide::inflate::decompress_to_vec(data.get(at..data.len() - 8)?).ok()
}

fn french() -> &'static Thesaurus {
    static T: OnceLock<Thesaurus> = OnceLock::new();
    T.get_or_init(|| Thesaurus::new(replacement("thes_fr.dat").unwrap_or_else(|| EMBEDDED_FR.to_string())))
}

fn english() -> &'static Thesaurus {
    static T: OnceLock<Thesaurus> = OnceLock::new();
    T.get_or_init(|| {
        let text =
            replacement("th_en_US_v2.dat").or_else(|| gunzip(EMBEDDED_EN_GZ).and_then(|bytes| String::from_utf8(bytes).ok())).unwrap_or_default();
        Thesaurus::new(text)
    })
}

fn thesaurus(lang: ProofLang) -> Option<&'static Thesaurus> {
    match lang {
        ProofLang::Fr => Some(french()),
        l if l.is_english() => Some(english()),
        _ => None,
    }
}

/// Is there a thesaurus for `lang`? French and English have one; Breton has none yet.
pub fn has_thesaurus(lang: ProofLang) -> bool {
    matches!(lang, ProofLang::Fr) || lang.is_english()
}

/// The forms of `word` to look up, best first: as written in lowercase, then without a plural or
/// a common ending (`chevaux` → `cheval`, `maisons` → `maison`, `running` → `run`).
fn forms(word: &str, lang: ProofLang) -> Vec<String> {
    let w = word.trim().to_lowercase();
    let mut out = vec![w.clone()];
    let mut push = |s: String| {
        if s.chars().count() > 1 && !out.contains(&s) {
            out.push(s);
        }
    };
    let ends: &[(&str, &str)] = if lang == ProofLang::Fr {
        &[("aux", "al"), ("eaux", "eau"), ("s", ""), ("x", ""), ("es", ""), ("e", ""), ("ée", "é"), ("ées", "é"), ("és", "é")]
    } else {
        &[("ies", "y"), ("es", ""), ("s", ""), ("ed", ""), ("ed", "e"), ("ing", ""), ("ing", "e"), ("ly", "")]
    };
    for (end, by) in ends {
        if let Some(stem) = w.strip_suffix(end) {
            push(format!("{stem}{by}"));
        }
    }
    out
}

/// The meanings of `word` in `lang`'s thesaurus, with their synonyms; empty when the word (or a
/// simple form of it) isn't in it, or `lang` has no thesaurus.
pub fn synonyms(word: &str, lang: ProofLang) -> Vec<Meaning> {
    let Some(t) = thesaurus(lang) else { return Vec::new() };
    forms(word, lang).iter().find_map(|w| t.meanings(w).filter(|m| !m.is_empty())).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn french_synonyms_by_meaning() {
        let m = synonyms("Maison", ProofLang::Fr);
        assert!(!m.is_empty());
        assert!(m.iter().any(|x| x.part.contains("Nom") && x.synonyms.iter().any(|s| s == "demeure")), "{m:?}");
        // Plurals and feminine forms find the entry.
        assert!(!synonyms("maisons", ProofLang::Fr).is_empty());
        assert!(!synonyms("chevaux", ProofLang::Fr).is_empty());
        assert!(synonyms("zzqxw", ProofLang::Fr).is_empty());
    }

    #[test]
    fn english_synonyms_without_broader_terms() {
        let m = synonyms("happy", ProofLang::En);
        assert!(m.iter().any(|x| x.synonyms.iter().any(|s| s == "glad")), "{m:?}");
        assert!(m.iter().all(|x| x.synonyms.iter().all(|s| !s.contains("term)"))), "{m:?}");
        assert!(!synonyms("houses", ProofLang::En).is_empty());
        assert!(!m.iter().flat_map(|x| &x.synonyms).any(|s| s == "unhappy"), "antonyms are left out");
    }

    #[test]
    fn breton_has_no_thesaurus_yet() {
        assert!(!has_thesaurus(ProofLang::Br));
        assert!(synonyms("ti", ProofLang::Br).is_empty());
    }

    #[test]
    fn gunzip_reads_the_built_in_english_file() {
        let text = gunzip(EMBEDDED_EN_GZ).and_then(|b| String::from_utf8(b).ok()).unwrap_or_default();
        assert!(text.starts_with("UTF-8"));
        assert!(gunzip(b"not gzip at all, really").is_none());
    }
}
