//! Lusk Tab : Excel number format codes (`#,##0.00 "€"`, `dd/mm/yyyy`, `0.0%`) as ODF number
//! styles, for saving as .ods. Each section of a code (`positive;negative;zero;text`) becomes a
//! style, joined by `style:map` conditions as LibreOffice writes them.

use std::collections::HashMap;
use std::fmt::Write;

use crate::xml::esc;

/// What a format shows, which decides a cell's `office:value-type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Number,
    Percent,
    Date,
    Time,
    Text,
}

#[derive(Debug, Default)]
pub(super) struct NumberStyles {
    prefix: &'static str,
    by_code: HashMap<String, Option<(String, Kind)>>,
    /// The `number:*-style` elements written so far.
    pub xml: String,
}

impl NumberStyles {
    /// Styles named `{prefix}1`, `{prefix}2`…
    pub(super) fn new(prefix: &'static str) -> Self {
        Self { prefix, ..Default::default() }
    }

    /// The style name for a format code (`None` for General, or a code that can't be written),
    /// and what the format shows.
    pub(super) fn name(&mut self, code: &str) -> Option<(String, Kind)> {
        if code.eq_ignore_ascii_case("general") || code.is_empty() {
            return None;
        }
        if let Some(found) = self.by_code.get(code) {
            return found.clone();
        }
        let name = format!("{}{}", self.prefix, self.by_code.len() + 1);
        let written = self.write(code, &name).map(|kind| (name, kind));
        self.by_code.insert(code.to_string(), written.clone());
        written
    }

    fn write(&mut self, code: &str, name: &str) -> Option<Kind> {
        let sections: Vec<Section> = split_sections(code).iter().take(4).map(|s| parse(s)).collect();
        let first = sections.first()?;
        let kind = first.kind();
        let conditional = sections.iter().any(|s| s.condition.is_some());
        // Which section is the main style, and the conditions that pick the others.
        let (main, maps): (usize, Vec<(usize, String)>) = if conditional {
            let last = sections.len() - 1;
            let maps = sections[..last].iter().enumerate().filter_map(|(i, s)| s.condition.clone().map(|c| (i, c))).collect();
            (last, maps)
        } else {
            match sections.len() {
                1 => (0, vec![]),
                2 => (1, vec![(0, "value()>=0".into())]),
                3 => (2, vec![(0, "value()>0".into()), (1, "value()<0".into())]),
                _ => (3, vec![(0, "value()>0".into()), (1, "value()<0".into()), (2, "value()=0".into())]),
            }
        };
        let mut maps_xml = String::new();
        for (i, cond) in &maps {
            let sub = format!("{name}P{i}");
            sections[*i].write(&sub, true, &mut self.xml);
            let _ = write!(maps_xml, r#"<style:map style:condition="{}" style:apply-style-name="{sub}"/>"#, esc(cond));
        }
        sections[main].write_with(name, false, &maps_xml, &mut self.xml);
        Some(kind)
    }
}

/// Splits on `;` outside quotes, brackets and escapes.
fn split_sections(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = code.chars();
    let (mut quoted, mut bracket) = (false, false);
    while let Some(c) = chars.next() {
        match c {
            '"' if !bracket => quoted = !quoted,
            '[' if !quoted => bracket = true,
            ']' if !quoted => bracket = false,
            '\\' if !quoted && !bracket => {
                current.push(c);
                if let Some(n) = chars.next() {
                    current.push(n);
                }
                continue;
            }
            ';' if !quoted && !bracket => {
                out.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    out.push(current);
    out
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Lit(String),
    Currency(String),
    /// A run of `0 # ? , . E+ /` digits.
    Digits(String),
    Percent,
    Text,
    General,
    Year(usize),
    Month(usize),
    Minute(usize),
    Day(usize),
    Hour(usize, bool),
    Second(usize, bool),
    AmPm,
}

#[derive(Debug, Default)]
struct Section {
    toks: Vec<Tok>,
    color: Option<&'static str>,
    condition: Option<String>,
}

fn color_hex(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "black" | "color1" => "#000000",
        "white" | "color2" => "#ffffff",
        "red" | "color3" => "#ff0000",
        "green" | "color4" => "#00ff00",
        "blue" | "color5" => "#0000ff",
        "yellow" | "color6" => "#ffff00",
        "magenta" | "color7" => "#ff00ff",
        "cyan" | "color8" => "#00ffff",
        _ => return None,
    })
}

fn parse(s: &str) -> Section {
    let mut sec = Section::default();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let lit = |toks: &mut Vec<Tok>, s: &str| match toks.last_mut() {
        Some(Tok::Lit(prev)) => prev.push_str(s),
        _ => toks.push(Tok::Lit(s.to_string())),
    };
    let digit = |c: char| matches!(c, '0' | '#' | '?');
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                let end = chars[i + 1..].iter().position(|&c| c == '"').map_or(chars.len(), |p| i + 1 + p);
                let text: String = chars[i + 1..end].iter().collect();
                lit(&mut sec.toks, &text);
                i = end + 1;
            }
            '\\' => {
                if let Some(n) = chars.get(i + 1) {
                    lit(&mut sec.toks, &n.to_string());
                }
                i += 2;
            }
            '_' => {
                lit(&mut sec.toks, " ");
                i += 2;
            }
            '*' => i += 2,
            '[' => {
                let end = chars[i + 1..].iter().position(|&c| c == ']').map_or(chars.len(), |p| i + 1 + p);
                let inner: String = chars[i + 1..end].iter().collect();
                i = end + 1;
                let lower = inner.to_ascii_lowercase();
                if let Some(cur) = inner.strip_prefix('$') {
                    let symbol = cur.split('-').next().unwrap_or("");
                    if !symbol.is_empty() {
                        sec.toks.push(Tok::Currency(symbol.to_string()));
                    }
                } else if let Some(hex) = color_hex(&inner) {
                    sec.color = Some(hex);
                } else if inner.starts_with(['<', '>', '=']) {
                    let op_len = inner.find(|c: char| !matches!(c, '<' | '>' | '=')).unwrap_or(inner.len());
                    let (op, n) = inner.split_at(op_len);
                    let op = if op == "<>" { "!=" } else { op };
                    if n.trim().parse::<f64>().is_ok() {
                        sec.condition = Some(format!("value(){op}{}", n.trim()));
                    }
                } else if !lower.is_empty() && lower.chars().all(|c| c == 'h') {
                    sec.toks.push(Tok::Hour(lower.len().min(2), true));
                } else if !lower.is_empty() && lower.chars().all(|c| c == 'm') {
                    sec.toks.push(Tok::Minute(lower.len().min(2)));
                } else if !lower.is_empty() && lower.chars().all(|c| c == 's') {
                    sec.toks.push(Tok::Second(lower.len().min(2), true));
                }
            }
            '@' => {
                sec.toks.push(Tok::Text);
                i += 1;
            }
            '%' => {
                sec.toks.push(Tok::Percent);
                i += 1;
            }
            _ if digit(c) || (c == '.' && chars.get(i + 1).is_some_and(|&n| digit(n))) => {
                let start = i;
                while i < chars.len() {
                    let ch = chars[i];
                    let next = chars.get(i + 1).copied();
                    let ok = digit(ch)
                        || ch == '.'
                        || (ch == ','
                            && (next.is_some_and(|n| digit(n) || n == ',' || n == '.')
                                || i + 1 == chars.len()
                                || !next.is_some_and(char::is_alphanumeric)))
                        || (matches!(ch, 'E' | 'e') && matches!(next, Some('+' | '-')))
                        || (matches!(ch, '+' | '-') && i > start && matches!(chars[i - 1], 'E' | 'e'))
                        || (ch == '/' && next.is_some_and(|n| digit(n) || n.is_ascii_digit()))
                        || (ch.is_ascii_digit() && i > start && chars[start..i].contains(&'/'))
                        || (ch == ' '
                            && chars[i + 1..].iter().take_while(|c| digit(**c)).count() > 0
                            && chars[i + 1..].iter().find(|c| !digit(**c)) == Some(&'/'));
                    if !ok {
                        break;
                    }
                    i += 1;
                }
                let run: String = chars[start..i].iter().collect();
                sec.toks.push(Tok::Digits(run));
            }
            _ => {
                let rest: String = chars[i..].iter().take(7).collect::<String>().to_ascii_lowercase();
                if rest.starts_with("general") {
                    sec.toks.push(Tok::General);
                    i += 7;
                    continue;
                }
                if rest.starts_with("am/pm") {
                    sec.toks.push(Tok::AmPm);
                    i += 5;
                    continue;
                }
                if rest.starts_with("a/p") {
                    sec.toks.push(Tok::AmPm);
                    i += 3;
                    continue;
                }
                let lower = c.to_ascii_lowercase();
                if matches!(lower, 'y' | 'm' | 'd' | 'h' | 's' | 'e') {
                    let n = chars[i..].iter().take_while(|ch| ch.to_ascii_lowercase() == lower).count();
                    i += n;
                    sec.toks.push(match lower {
                        'y' | 'e' => Tok::Year(n),
                        'm' => Tok::Month(n),
                        'd' => Tok::Day(n),
                        'h' => Tok::Hour(n, false),
                        _ => Tok::Second(n, false),
                    });
                    continue;
                }
                lit(&mut sec.toks, &c.to_string());
                i += 1;
            }
        }
    }
    // `m` right after hours, or right before seconds, is minutes.
    let dates: Vec<usize> = (0..sec.toks.len())
        .filter(|&i| matches!(sec.toks[i], Tok::Year(_) | Tok::Month(_) | Tok::Day(_) | Tok::Hour(..) | Tok::Second(..) | Tok::Minute(_)))
        .collect();
    for (k, &i) in dates.iter().enumerate() {
        if let Tok::Month(n) = sec.toks[i]
            && n <= 2
        {
            let after_hour = k > 0 && matches!(sec.toks[dates[k - 1]], Tok::Hour(..));
            let before_second = dates.get(k + 1).is_some_and(|&j| matches!(sec.toks[j], Tok::Second(..)));
            if after_hour || before_second {
                sec.toks[i] = Tok::Minute(n);
            }
        }
    }
    sec
}

impl Section {
    fn kind(&self) -> Kind {
        let date = self.toks.iter().any(|t| matches!(t, Tok::Year(_) | Tok::Month(_) | Tok::Day(_)));
        let time = self.toks.iter().any(|t| matches!(t, Tok::Hour(..) | Tok::Minute(_) | Tok::Second(..) | Tok::AmPm));
        if date {
            Kind::Date
        } else if time {
            Kind::Time
        } else if self.toks.contains(&Tok::Percent) {
            Kind::Percent
        } else if self.toks.contains(&Tok::Text) && !self.toks.iter().any(|t| matches!(t, Tok::Digits(_) | Tok::General)) {
            Kind::Text
        } else {
            Kind::Number
        }
    }

    fn write(&self, name: &str, volatile: bool, out: &mut String) {
        self.write_with(name, volatile, "", out);
    }

    fn write_with(&self, name: &str, volatile: bool, maps: &str, out: &mut String) {
        let kind = self.kind();
        let currency = self.toks.iter().any(|t| matches!(t, Tok::Currency(_)));
        let element = match kind {
            Kind::Date => "date-style",
            Kind::Time => "time-style",
            Kind::Percent => "percentage-style",
            Kind::Text => "text-style",
            Kind::Number if currency => "currency-style",
            Kind::Number => "number-style",
        };
        let _ = write!(out, r#"<number:{element} style:name="{name}""#);
        if volatile {
            out.push_str(r#" style:volatile="true""#);
        }
        if self.toks.iter().any(|t| matches!(t, Tok::Hour(_, true) | Tok::Second(_, true))) {
            out.push_str(r#" number:truncate-on-overflow="false""#);
        }
        out.push('>');
        if let Some(hex) = self.color {
            let _ = write!(out, r#"<style:text-properties fo:color="{hex}"/>"#);
        }
        let body = out.len();
        let mut number_done = false;
        let mut pending_text = String::new();
        let flush = |text: &mut String, out: &mut String| {
            if !text.is_empty() {
                let _ = write!(out, "<number:text>{}</number:text>", esc(text));
                text.clear();
            }
        };
        let mut prev_second = false;
        for t in &self.toks {
            let second = matches!(t, Tok::Second(..));
            match t {
                Tok::Lit(s) => pending_text.push_str(s),
                Tok::Percent => pending_text.push('%'),
                Tok::Currency(s) if element == "currency-style" => {
                    flush(&mut pending_text, out);
                    let _ = write!(out, "<number:currency-symbol>{}</number:currency-symbol>", esc(s));
                }
                Tok::Currency(s) => pending_text.push_str(s),
                Tok::Text if kind == Kind::Text => {
                    flush(&mut pending_text, out);
                    out.push_str("<number:text-content/>");
                }
                Tok::Text => {}
                Tok::General if !number_done && matches!(kind, Kind::Number | Kind::Percent) => {
                    flush(&mut pending_text, out);
                    out.push_str(r#"<number:number number:min-integer-digits="1"/>"#);
                    number_done = true;
                }
                Tok::Digits(d) if matches!(kind, Kind::Date | Kind::Time) => {
                    // `ss.00`: tenths and hundredths of seconds, written on the seconds element.
                    if !(prev_second && d.starts_with('.')) {
                        pending_text.push_str(d);
                    }
                }
                Tok::Digits(d) if !number_done && matches!(kind, Kind::Number | Kind::Percent) => {
                    flush(&mut pending_text, out);
                    out.push_str(&number_element(d));
                    number_done = true;
                }
                Tok::Year(n) => date_part(out, &mut pending_text, "year", *n >= 3, ""),
                Tok::Month(n) => {
                    let textual = if *n >= 3 { r#" number:textual="true""# } else { "" };
                    date_part(out, &mut pending_text, "month", *n == 2 || *n == 4, textual);
                }
                Tok::Day(n) if *n >= 3 => date_part(out, &mut pending_text, "day-of-week", *n >= 4, ""),
                Tok::Day(n) => date_part(out, &mut pending_text, "day", *n >= 2, ""),
                Tok::Hour(n, _) => date_part(out, &mut pending_text, "hours", *n >= 2, ""),
                Tok::Minute(n) => date_part(out, &mut pending_text, "minutes", *n >= 2, ""),
                Tok::Second(n, _) => {
                    let decimals = self.second_decimals();
                    let extra = if decimals > 0 { format!(r#" number:decimal-places="{decimals}""#) } else { String::new() };
                    date_part(out, &mut pending_text, "seconds", *n >= 2, &extra);
                }
                Tok::AmPm => {
                    flush(&mut pending_text, out);
                    out.push_str("<number:am-pm/>");
                }
                _ => {}
            }
            prev_second = second;
        }
        if element == "percentage-style" && !number_done {
            out.push_str(r#"<number:number number:decimal-places="0" number:min-integer-digits="1"/>"#);
        }
        if !pending_text.is_empty() || out.len() == body {
            // An empty section (`0;-0;;@` shows nothing for zero) stays an empty text.
            let _ = write!(out, "<number:text>{}</number:text>", esc(&pending_text));
        }
        out.push_str(maps);
        let _ = write!(out, "</number:{element}>");
    }

    fn second_decimals(&self) -> usize {
        let mut after = false;
        for t in &self.toks {
            match t {
                Tok::Second(..) => after = true,
                Tok::Digits(d) if after && d.starts_with('.') => return d.chars().filter(|c| *c == '0').count().min(3),
                _ => after = false,
            }
        }
        0
    }
}

fn date_part(out: &mut String, pending: &mut String, element: &str, long: bool, extra: &str) {
    if !pending.is_empty() {
        let _ = write!(out, "<number:text>{}</number:text>", esc(pending));
        pending.clear();
    }
    let style = if long { r#" number:style="long""# } else { "" };
    let _ = write!(out, "<number:{element}{style}{extra}/>");
}

/// A digit run (`#,##0.00`, `0.00E+00`, `# ?/?`) as its `number:*` element.
fn number_element(d: &str) -> String {
    let count = |s: &str, set: &[char]| s.chars().filter(|c| set.contains(c)).count();
    if let Some(e) = d.find(['E', 'e']) {
        let (mantissa, exponent) = d.split_at(e);
        let (int, dec) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        return format!(
            r#"<number:scientific-number number:decimal-places="{}" number:min-decimal-places="{}" number:min-integer-digits="{}" number:min-exponent-digits="{}"{}/>"#,
            count(dec, &['0', '#', '?']),
            count(dec, &['0']),
            count(int, &['0']).max(1),
            count(exponent, &['0', '#', '?']).max(1),
            if exponent.contains('-') { r#" number:forced-exponent-sign="false""# } else { "" },
        );
    }
    if let Some((left, denominator)) = d.split_once('/') {
        let (int, numerator) = left.rsplit_once(' ').map_or((None, left), |(i, n)| (Some(i), n));
        let int_attr = int.map(|i| format!(r#" number:min-integer-digits="{}""#, count(i, &['0']))).unwrap_or_default();
        let denominator_attr = if denominator.chars().all(|c| c.is_ascii_digit()) && !denominator.is_empty() {
            format!(r#" number:denominator-value="{denominator}""#)
        } else {
            format!(r#" number:min-denominator-digits="{}""#, count(denominator, &['0', '#', '?']).max(1))
        };
        return format!(
            r#"<number:fraction{int_attr} number:min-numerator-digits="{}"{denominator_attr}/>"#,
            count(numerator, &['0', '#', '?']).max(1)
        );
    }
    let (int, dec) = d.split_once('.').unwrap_or((d, ""));
    let trailing = int.len() - int.trim_end_matches(',').len() + (dec.len() - dec.trim_end_matches(',').len());
    let int_core = int.trim_end_matches(',');
    let grouping = int_core.contains(',');
    let decimals = count(dec, &['0', '#', '?']);
    let mut out = format!(
        r#"<number:number number:decimal-places="{decimals}" number:min-decimal-places="{}" number:min-integer-digits="{}""#,
        count(dec, &['0']),
        count(int_core, &['0'])
    );
    if grouping {
        out.push_str(r#" number:grouping="true""#);
    }
    if trailing > 0 {
        let _ = write!(out, r#" number:display-factor="{}""#, 1000u64.pow(trailing.min(3) as u32));
    }
    out.push_str("/>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ods::style::OdsStyles;

    /// A code written as ODF styles and read back gives the same code.
    fn round_trip(code: &str) -> (String, Kind) {
        let mut styles = NumberStyles::new("N");
        let (name, kind) = styles.name(code).unwrap();
        let root = crate::xml::parse(format!("<r><office:styles>{}</office:styles></r>", styles.xml).as_bytes()).unwrap();
        let mut read = OdsStyles::default();
        read.add(&root, false);
        (read.number_code(&name).unwrap(), kind)
    }

    #[test]
    fn excel_codes_survive_odf_number_styles() {
        for code in [
            "0",
            "0.00",
            "#,##0",
            "#,##0.00",
            "0%",
            "0.0%",
            "#,##0.00\" €\"",
            "#,##0.00;[Red]-#,##0.00",
            "dd/mm/yyyy",
            "d mmm yyyy",
            "dddd d mmmm yyyy",
            "hh:mm",
            "hh:mm:ss",
            "[hh]:mm:ss",
            "h:mm AM/PM",
            "0.00E+00",
            "# ?/?",
            "@",
            "#,##0;-#,##0;\"nul\"",
            "0;-0;;@",
        ] {
            assert_eq!(round_trip(code).0, code, "{code}");
        }
        assert_eq!(round_trip("dd/mm/yyyy").1, Kind::Date);
        assert_eq!(round_trip("hh:mm").1, Kind::Time);
        assert_eq!(round_trip("0.0%").1, Kind::Percent);
        assert_eq!(round_trip("@").1, Kind::Text);
        // Excel's own currency brackets become LibreOffice's currency symbol.
        assert_eq!(round_trip("#,##0.00 [$€-40C]").0, "#,##0.00 \"€\"");
        assert_eq!(NumberStyles::default().name("General"), None);
    }
}
