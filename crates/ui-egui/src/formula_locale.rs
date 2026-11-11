//! UI formula syntax boundary. Engine commands, control/API callers and XLSX keep English.
use crate::i18n::Language;
use gridcraft_engine::formula::{self, FormulaLocale, ParseError};
use gridcraft_engine::model::Workbook;

pub(crate) fn canonical(
    text: &str,
    locale: FormulaLocale,
    wb: &Workbook,
    sheet: usize,
    cell: gridcraft_engine::core::CellRef,
) -> Result<String, ParseError> {
    // The editor hid the `_xludf.` prefixes of this cell's formula; confirming it unchanged keeps
    // the user-defined calls instead of binding them to the built-ins of the same name.
    if let Some(stored) = wb.sheet(sheet).and_then(|sh| sh.cell(cell)).filter(|c| c.formula.is_some()).map(|c| c.input_text())
        && has_udf_prefix(&stored)
        && text == display(&stored, locale, wb, sheet)
    {
        return Ok(stored);
    }
    if wb.sheet(sheet).is_some_and(|sh| wb.styles.get(sh.style_id(cell)).num_fmt.as_str() == "@") {
        return Ok(text.to_string());
    }
    // Lusk Tab: numbers and dates typed the way the display region writes them.
    let region = gridcraft_engine::numfmt::display_region();
    let regional = region.tag != gridcraft_engine::core::region::default_region().tag;
    if locale == FormulaLocale::En && !regional {
        return Ok(text.to_string());
    }
    let is_formula = text.starts_with('=')
        || ((text.starts_with('+') || text.starts_with('-')) && text.chars().nth(1).is_some_and(|c| c.is_ascii_alphabetic() || c == '('));
    if locale == FormulaLocale::En && is_formula {
        return Ok(text.to_string());
    }
    if regional
        && !is_formula
        && let Some(date) = regional_date(text, region)
    {
        return Ok(date);
    }
    // An error value typed in the formula language (`#VALEUR!`) is stored as Excel stores it.
    if !is_formula
        && let Some(error) = gridcraft_engine::core::CellError::ALL.iter().find(|e| text.trim().eq_ignore_ascii_case(locale.error_name(**e)))
    {
        return Ok(error.as_str().to_string());
    }
    // Same formula detection as the engine's `input_to_cell` (no trimming: " =1" is text).
    let body = text;
    if body.starts_with('=')
        || ((body.starts_with('+') || body.starts_with('-')) && body.chars().nth(1).is_some_and(|c| c.is_ascii_alphabetic() || c == '('))
    {
        let known = |name: &str| wb.name(name, sheet).is_some() || wb.table(name).is_some();
        // Match the engine's existing convenience for missing closing parentheses.
        let expr = formula::parse_closing_parens(body, |text| formula::parse_input(text, locale, known));
        return Ok(format!("={}", formula::print(&expr?)));
    }
    // Decimal-comma number entry follows the region (else the formula language), without
    // rewriting text or dates. Digit groups (`1 234,50`) are dropped.
    let decimal = if regional { region.decimal } else { locale.decimal_separator() };
    let trimmed = body.trim();
    let ungrouped: String = if regional && (region.is_group(' ') || region.group == '\u{a0}') {
        trimmed.chars().filter(|c| !matches!(c, ' ' | '\u{a0}' | '\u{202f}')).collect()
    } else {
        trimmed.to_string()
    };
    let normalized = if decimal == '.' { ungrouped } else { ungrouped.replace(decimal, ".") };
    let number = normalized.strip_suffix('%').unwrap_or(&normalized);
    if number.parse::<f64>().is_ok_and(f64::is_finite) {
        return Ok(normalized);
    }
    Ok(text.to_string())
}

/// A date typed day first (`25/12/2026`, `25/12`, `25/12/2026 14:30`) in a day-month-year region,
/// rewritten month first as the engine reads dates (`12/25/2026`). `None` when `text` isn't one.
fn regional_date(text: &str, region: &gridcraft_engine::core::Region) -> Option<String> {
    if let Some(date) = named_month_date(text, region) {
        return Some(date);
    }
    if region.date_order != gridcraft_engine::core::region::DateOrder::Dmy {
        return None;
    }
    let sep = region.date_sep;
    let trimmed = text.trim();
    let (date, time) = match trimmed.split_once(' ') {
        Some((d, t)) => (d, Some(t.trim())),
        None => (trimmed, None),
    };
    let parts: Vec<&str> = date.split(sep).collect();
    let numeric = |s: &&str| !s.is_empty() && s.len() <= 4 && s.chars().all(|c| c.is_ascii_digit());
    if !(2..=3).contains(&parts.len()) || !parts.iter().all(numeric) {
        return None;
    }
    let day: u32 = parts.first()?.parse().ok()?;
    let month: u32 = parts.get(1)?.parse().ok()?;
    if !(1..=31).contains(&day) || !(1..=12).contains(&month) || parts.first()?.len() > 2 || parts.get(1)?.len() > 2 {
        return None;
    }
    let mut out = format!("{month}/{day}");
    if let Some(year) = parts.get(2) {
        out.push('/');
        out.push_str(year);
    }
    if let Some(t) = time {
        if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || c == ':') {
            return None;
        }
        out.push(' ');
        out.push_str(t);
    }
    Some(out)
}

/// A date typed with the region's month name, day first: `7 oct. 2026`, `7 octobre 2026`,
/// `7 octobre`, `1er janvier 2027`. Rewritten month first in digits (`10/7/2026`, `10/7`).
fn named_month_date(text: &str, region: &gridcraft_engine::core::Region) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if !(2..=3).contains(&words.len()) {
        return None;
    }
    let day_word = words.first()?;
    // French writes the first of the month `1er`.
    let day_digits = day_word.strip_suffix("er").filter(|d| *d == "1").unwrap_or(day_word);
    if day_digits.is_empty() || day_digits.len() > 2 || !day_digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let day: u32 = day_digits.parse().ok()?;
    let month = region.month_from_name(words.get(1)?)?;
    if !(1..=31).contains(&day) {
        return None;
    }
    match words.get(2) {
        None => Some(format!("{month}/{day}")),
        Some(year) if (2..=4).contains(&year.len()) && year.chars().all(|c| c.is_ascii_digit()) => Some(format!("{month}/{day}/{year}")),
        Some(_) => None,
    }
}

pub(crate) fn display(text: &str, locale: FormulaLocale, wb: &Workbook, sheet: usize) -> String {
    // English (and every language without a formula table) shows the stored text, `_xludf.`
    // prefixes aside, and skips a parse per frame.
    if locale == FormulaLocale::En {
        return hide_udf_prefix(text, locale);
    }
    let Some(body) = text.strip_prefix('=') else { return text.to_string() };
    let shown = formula::parse(body)
        .and_then(|expr| formula::print_input(&expr, locale, |name| wb.name(name, sheet).is_some() || wb.table(name).is_some()))
        .map(|body| format!("={body}"))
        .unwrap_or_else(|_| text.to_string());
    hide_udf_prefix(&shown, locale)
}

const UDF_PREFIX: &str = "_xludf.";

fn has_udf_prefix(text: &str) -> bool {
    text.to_ascii_lowercase().contains(UDF_PREFIX)
}

/// Excel shows a user-defined call such as `_xludf.EOMONTH()` as `=EOMONTH()`. Only function
/// names lose the prefix: text literals and names keep it.
fn hide_udf_prefix(text: &str, locale: FormulaLocale) -> String {
    let Some(body) = text.strip_prefix('=').filter(|b| has_udf_prefix(b)) else { return text.to_string() };
    let Ok(tokens) = formula::lexer::tokenize_locale(body, locale) else { return text.to_string() };
    let mut out = String::from("=");
    let mut from = 0;
    for t in tokens {
        if matches!(t.tok, formula::lexer::Tok::Func(_))
            && body.get(t.start..t.start + UDF_PREFIX.len()).is_some_and(|p| p.eq_ignore_ascii_case(UDF_PREFIX))
        {
            out.push_str(body.get(from..t.start).unwrap_or(""));
            from = t.start + UDF_PREFIX.len();
        }
    }
    out.push_str(body.get(from..).unwrap_or(""));
    out
}

/// A function's argument hint. The name and separators follow the formula syntax; the argument
/// names follow the interface language, all of them or none, so a hint never mixes two languages.
pub(crate) fn signature(name: &str, locale: FormulaLocale, language: Language) -> Option<String> {
    let english = crate::formula_bar::function_signature(locale.canonical_name(name))?;
    let Some((head, args)) = split_signature(&english) else { return Some(english) };
    let separator = format!("{} ", locale.list_separator());
    let args = match args.iter().map(|arg| translate_argument(arg, language)).collect::<Option<Vec<_>>>() {
        Some(translated) => translated.join(&separator),
        None => args.join(&separator),
    };
    Some(format!("{}({args})", locale.function_name(head)))
}

/// A function's description in the interface language (English while untranslated).
pub(crate) fn description(name: &str, locale: FormulaLocale, language: Language) -> Option<String> {
    let english = crate::formula_bar::function_description(locale.canonical_name(name))?;
    Some(language.function_description(&english).map_or(english, str::to_string))
}

/// `SUM(number1, [number2], ...)` -> `SUM` and its arguments as written.
fn split_signature(signature: &str) -> Option<(&str, Vec<&str>)> {
    let (head, rest) = signature.split_once('(')?;
    let inner = rest.strip_suffix(')')?;
    Some((head, if inner.is_empty() { Vec::new() } else { inner.split(", ").collect() }))
}

/// `[number2]` -> `number`. A bracket can open or close on its own (SWITCH's
/// `[default_or_value2, result2]`, LAMBDA's `[parameter1, ...]`).
fn argument_name(arg: &str) -> &str {
    let arg = arg.strip_prefix('[').unwrap_or(arg);
    let arg = arg.strip_suffix(']').unwrap_or(arg);
    arg.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// `[number2]` -> `[número2]`: brackets, index and `...` stay.
fn translate_argument(arg: &str, language: Language) -> Option<String> {
    let name = argument_name(arg);
    if name == "..." {
        return Some(arg.to_string());
    }
    // `arg` is `[`? + `name` + index + `]`?, so the first match is the name itself.
    Some(arg.replacen(name, language.function_argument(name)?, 1))
}

/// Avoid suggesting an alias that would call a workbook/table name instead of the function.
pub(crate) fn completion_name<'a>(name: &'a str, locale: FormulaLocale, wb: &Workbook, sheet: usize) -> &'a str {
    let alias = locale.function_name(name);
    if wb.name(alias, sheet).is_some() || wb.table(alias).is_some() { name } else { alias }
}

/// Engine-generated edit templates use canonical syntax. The only incomplete template is
/// the bare function opening emitted by Insert Function; typed user input never enters here.
pub(crate) fn display_template(text: &str, locale: FormulaLocale, wb: &Workbook, sheet: usize) -> String {
    if let Some(name) = text.strip_prefix('=').and_then(|s| s.strip_suffix('('))
        && !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.'))
    {
        return format!("={}(", completion_name(name, locale, wb, sheet));
    }
    display(text, locale, wb, sheet)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hint for `name`, empty when there is none.
    fn sig(name: &str, locale: FormulaLocale, language: Language) -> String {
        signature(name, locale, language).unwrap_or_default()
    }

    /// Hints keep the formula syntax's name and separators and translate the argument names
    /// only when the interface language has them all.
    #[test]
    fn function_help_follows_the_interface_language_and_the_formula_syntax() {
        use FormulaLocale::{En, Es};
        // Spanish help over English formulas: the name and `,` stay as typed; brackets, indexes
        // and `...` survive.
        assert_eq!(sig("IF", En, Language::Es), "IF(prueba_lógica, [valor_si_verdadero], [valor_si_falso])");
        assert_eq!(sig("SUM", En, Language::Es), "SUM(número1, [número2], ...)");
        assert_eq!(description("IF", En, Language::Es).as_deref(), Some("Devuelve un valor según una condición."));
        // Spanish names and separators, from a localized name.
        assert_eq!(sig("suma", Es, Language::Es), "SUMA(número1; [número2]; ...)");
        assert_eq!(description("SUMA", Es, Language::Es).as_deref(), Some("Suma los números."));
        // An argument list Spanish covers only in part stays English instead of mixing (the name
        // is whatever the Spanish formula table calls SUMIFS).
        let sumifs = sig("SUMIFS", Es, Language::Es);
        assert!(sumifs.ends_with("(sum_range; criteria_range1; criteria1; ...)"), "{sumifs}");
        assert_eq!(description("SUMIFS", Es, Language::Es).as_deref(), Some("Adds the cells that meet every condition."));
        assert_eq!(sig("IF", En, Language::En), "IF(logical_test, [value_if_true], [value_if_false])");
    }

    /// Lusk Tab: in a French region, numbers and dates are typed the French way and reach the
    /// engine in its canonical spelling; formulas still follow the formula language.
    #[test]
    fn french_region_input() {
        let fr = gridcraft_engine::core::region::find("fr-FR").unwrap();
        let wb = Workbook::new();
        let cell = gridcraft_engine::core::CellRef { row: 0, col: 0 };
        let typed =
            |text: &str, locale: FormulaLocale| gridcraft_engine::numfmt::with_display_region(fr, || canonical(text, locale, &wb, 0, cell).unwrap());
        assert_eq!(typed("25/12/2026", FormulaLocale::Fr), "12/25/2026");
        assert_eq!(typed("7/10/2026 18:30", FormulaLocale::Fr), "10/7/2026 18:30");
        assert_eq!(typed("25/12", FormulaLocale::Fr), "12/25");
        assert_eq!(typed("1 234,5", FormulaLocale::Fr), "1234.5");
        assert_eq!(typed("1\u{a0}234,5", FormulaLocale::Fr), "1234.5");
        assert_eq!(typed("12,5%", FormulaLocale::Fr), "12.5%");
        assert_eq!(typed("#VALEUR!", FormulaLocale::Fr), "#VALUE!");
        assert_eq!(typed("#n/a", FormulaLocale::Fr), "#N/A");
        // With the month's name.
        assert_eq!(typed("7 oct. 2026", FormulaLocale::Fr), "10/7/2026");
        assert_eq!(typed("7 octobre 2026", FormulaLocale::Fr), "10/7/2026");
        assert_eq!(typed("25 déc.", FormulaLocale::Fr), "12/25");
        assert_eq!(typed("1er janvier 2027", FormulaLocale::Fr), "1/1/2027");
        assert_eq!(typed("14 juillet 26", FormulaLocale::Fr), "7/14/26");
        // The region applies even with English formulas (a French computer, English interface).
        assert_eq!(typed("1,5", FormulaLocale::En), "1.5");
        assert_eq!(typed("25/12/2026", FormulaLocale::En), "12/25/2026");
        assert_eq!(typed("=SUM(1,2)", FormulaLocale::En), "=SUM(1,2)");
        assert_eq!(typed("=SOMME(1,5;2)", FormulaLocale::Fr), "=SUM(1.5,2)");
        // Not dates or numbers: left as typed.
        for text in ["32/12/2026", "25/13/2026", "Brest", "1/2/3/4", "25/12/2026 midi", "7 rue de Brest", "2er mai"] {
            assert_eq!(typed(text, FormulaLocale::Fr), text);
        }
        // Without a region, nothing changes for English formulas.
        assert_eq!(canonical("25/12/2026", FormulaLocale::En, &wb, 0, cell).unwrap(), "25/12/2026");
    }

    /// French, as in Excel in French: French names, `;` between arguments, French help.
    #[test]
    fn french_function_help() {
        use FormulaLocale::Fr;
        assert_eq!(sig("somme", Fr, Language::Fr), "SOMME(nombre1; [nombre2]; ...)");
        assert_eq!(sig("SI", Fr, Language::Fr), "SI(test_logique; [valeur_si_vrai]; [valeur_si_faux])");
        assert_eq!(description("SOMME", Fr, Language::Fr).as_deref(), Some("Additionne tous les nombres."));
        let vlookup = sig("RECHERCHEV", Fr, Language::Fr);
        assert_eq!(vlookup, "RECHERCHEV(valeur_cherchée; table_matrice; no_index_col; [valeur_proche])");
        assert_eq!(description("RECHERCHEV", Fr, Language::Fr).as_deref().map(|d| d.starts_with("Cherche une valeur")), Some(true));
    }
}
