//! Lusk Tab : the formatting of an .ods file. Cell styles (fonts, colours, borders, alignment),
//! number styles, column widths, row heights and sheet properties are read from styles.xml and
//! from content.xml's automatic styles, then turned into the workbook's own styles.
//! Format reference: ODF 1.3 part 3, sections 16 (styles) and 20 (formatting properties).

use std::collections::HashMap;

use gridcraft_model::{BorderLine, BorderStyle, Color, Fill, HAlign, Style, Underline, VAlign, VertAlign};

use crate::xml::El;

/// How deep `style:parent-style-name` chains are followed (a loop stops there too).
const MAX_PARENTS: usize = 32;
/// Longest number format kept (Excel's own limit is 255 characters).
const MAX_CODE: usize = 255;

/// Properties one cell style states; `None` inherits from the parent style.
#[derive(Clone, Debug, Default)]
pub(super) struct CellProps {
    parent: Option<String>,
    data_style: Option<String>,
    font_name: Option<String>,
    font_family: Option<u8>,
    size: Option<f32>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<Underline>,
    strike: Option<bool>,
    color: Option<Color>,
    vert: Option<VertAlign>,
    fill: Option<Fill>,
    /// Left, right, top, bottom, diagonal down, diagonal up.
    borders: [Option<BorderLine>; 6],
    h: Option<HAlign>,
    /// `style:text-align-source="value-type"`: the alignment follows the value (General).
    align_by_value: Option<bool>,
    v: Option<VAlign>,
    wrap: Option<bool>,
    shrink: Option<bool>,
    indent: Option<u8>,
    rotation: Option<i16>,
    locked: Option<bool>,
    hidden: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct RowProps {
    /// Pixels at 96 dpi.
    pub height: Option<f32>,
    /// `style:use-optimal-row-height`: the height follows the content.
    pub optimal: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TableProps {
    pub hidden: bool,
    pub tab_color: Option<Color>,
}

/// Every style of one .ods file, by name.
#[derive(Debug, Default)]
pub(super) struct OdsStyles {
    /// `style:font-face` name → (family name, Excel family class).
    fonts: HashMap<String, (String, Option<u8>)>,
    numbers: HashMap<String, El>,
    cells: HashMap<String, CellProps>,
    default_cell: CellProps,
    /// Column style → width in pixels.
    pub columns: HashMap<String, f32>,
    pub rows: HashMap<String, RowProps>,
    pub tables: HashMap<String, TableProps>,
}

impl OdsStyles {
    /// Adds the styles of one part: styles.xml (`automatic` false: its automatic styles only
    /// serve page layouts) or content.xml (`automatic` true).
    pub(super) fn add(&mut self, root: &El, automatic: bool) {
        for part in &root.children {
            match part.name.as_str() {
                "font-face-decls" => {
                    for face in part.kids("font-face") {
                        if let (Some(name), Some(family)) = (face.attr("name"), face.attr("font-family")) {
                            let generic = face.attr("font-family-generic").and_then(generic_family);
                            self.fonts.insert(name.to_string(), (unquote(family), generic));
                        }
                    }
                }
                "styles" => self.add_styles(part),
                "automatic-styles" if automatic => self.add_styles(part),
                _ => {}
            }
        }
    }

    fn add_styles(&mut self, list: &El) {
        for el in &list.children {
            let name = el.attr("name").map(str::to_string);
            match (el.name.as_str(), el.attr("family")) {
                ("default-style", Some("table-cell")) => self.default_cell = cell_props(el),
                ("style", Some("table-cell")) => {
                    if let Some(name) = name {
                        self.cells.insert(name, cell_props(el));
                    }
                }
                ("style", Some("table-column")) => {
                    let width = el.child("table-column-properties").and_then(|p| p.attr("column-width")).and_then(length_px);
                    if let (Some(name), Some(width)) = (name, width) {
                        self.columns.insert(name, width);
                    }
                }
                ("style", Some("table-row")) => {
                    if let (Some(name), Some(p)) = (name, el.child("table-row-properties")) {
                        let height = p.attr("row-height").and_then(length_px);
                        self.rows.insert(name, RowProps { height, optimal: p.flag("use-optimal-row-height", height.is_none()) });
                    }
                }
                ("style", Some("table")) => {
                    if let (Some(name), Some(p)) = (name, el.child("table-properties")) {
                        let tab_color = p.attr("tab-color").and_then(Color::from_hex);
                        self.tables.insert(name, TableProps { hidden: p.attr("display") == Some("false"), tab_color });
                    }
                }
                ("number-style" | "currency-style" | "percentage-style" | "date-style" | "time-style" | "boolean-style" | "text-style", _) => {
                    if let Some(name) = name {
                        self.numbers.insert(name, el.clone());
                    }
                }
                _ => {}
            }
        }
    }

    /// The full style of the named cell style (`None`: the document's default cell style),
    /// starting from `base` (the workbook's default style).
    pub(super) fn resolve(&self, name: Option<&str>, base: &Style) -> Style {
        let mut chain: Vec<&CellProps> = Vec::new();
        let mut next = name;
        while let Some(n) = next {
            if chain.len() >= MAX_PARENTS {
                break;
            }
            let Some(props) = self.cells.get(n) else { break };
            if chain.iter().any(|p| std::ptr::eq(*p, props)) {
                break;
            }
            chain.push(props);
            next = props.parent.as_deref();
        }
        let mut style = base.clone();
        self.apply(&self.default_cell, &mut style);
        for props in chain.iter().rev() {
            self.apply(props, &mut style);
        }
        style
    }

    pub(super) fn has_cell_style(&self, name: &str) -> bool {
        self.cells.contains_key(name)
    }

    fn apply(&self, p: &CellProps, s: &mut Style) {
        if let Some(name) = &p.font_name {
            let (family, generic) = self.fonts.get(name).cloned().unwrap_or_else(|| (name.clone(), None));
            if !family.is_empty() {
                s.font.name = family;
                s.font.minor = false;
                s.font.family = generic.or(s.font.family);
            }
        }
        if let Some(f) = p.font_family {
            s.font.family = Some(f);
        }
        if let Some(v) = p.size {
            s.font.size = v;
        }
        if let Some(v) = p.bold {
            s.font.bold = v;
        }
        if let Some(v) = p.italic {
            s.font.italic = v;
        }
        if let Some(v) = p.underline {
            s.font.underline = v;
        }
        if let Some(v) = p.strike {
            s.font.strike = v;
        }
        if let Some(v) = p.color {
            s.font.color = v;
        }
        if let Some(v) = p.vert {
            s.font.vert = v;
        }
        if let Some(v) = p.fill {
            s.fill = v;
        }
        let [left, right, top, bottom, down, up] = p.borders;
        let b = &mut s.borders;
        for (line, slot) in
            [(left, &mut b.left), (right, &mut b.right), (top, &mut b.top), (bottom, &mut b.bottom), (down, &mut b.diag_down), (up, &mut b.diag_up)]
        {
            if let Some(line) = line {
                *slot = line;
            }
        }
        if let Some(v) = p.h {
            s.align.h = v;
        }
        if p.align_by_value == Some(true) {
            s.align.h = HAlign::General;
        }
        if let Some(v) = p.v {
            s.align.v = v;
        }
        if let Some(v) = p.wrap {
            s.align.wrap = v;
        }
        if let Some(v) = p.shrink {
            s.align.shrink = v;
        }
        if let Some(v) = p.indent {
            s.align.indent = v;
        }
        if let Some(v) = p.rotation {
            s.align.rotation = v;
        }
        if let Some(v) = p.locked {
            s.protection.locked = v;
        }
        if let Some(v) = p.hidden {
            s.protection.hidden = v;
        }
        if let Some(code) = p.data_style.as_deref().and_then(|n| self.number_code(n)) {
            s.num_fmt = gridcraft_model::NumFmt::new(&code);
        }
    }

    /// The Excel number format code of an ODF number style, with its `style:map` conditions
    /// as format sections (`#,##0.00;[Red]-#,##0.00`).
    pub(super) fn number_code(&self, name: &str) -> Option<String> {
        let el = self.numbers.get(name)?;
        let main = section(el);
        let mut maps = Vec::new();
        for m in el.kids("map") {
            let (Some(cond), Some(apply)) = (m.attr("condition"), m.attr("apply-style-name")) else { continue };
            let Some(sub) = self.numbers.get(apply) else { continue };
            let Some((op, n)) = condition(cond) else { continue };
            maps.push((op, n, section(sub)));
        }
        let code = combine(main, maps, el.name == "text-style");
        (code.len() <= MAX_CODE).then_some(code)
    }
}

fn combine(main: String, maps: Vec<(&'static str, f64, String)>, text_main: bool) -> String {
    if maps.is_empty() {
        return main;
    }
    let zero = maps.iter().all(|(_, n, _)| *n == 0.0);
    let find = |ops: &[&str]| maps.iter().find(|(op, n, _)| *n == 0.0 && ops.contains(op)).map(|(_, _, s)| s.clone());
    if zero {
        let (pos, pos_inclusive) = match (find(&[">"]), find(&[">="])) {
            (Some(p), _) => (Some(p), false),
            (None, Some(p)) => (Some(p), true),
            _ => (None, false),
        };
        let neg = find(&["<"]);
        let zero_part = find(&["="]);
        let text = if text_main { main.clone() } else { String::new() };
        let tail = |mut out: String| {
            if text_main {
                out.push(';');
                out.push_str(&text);
            }
            out
        };
        match (pos, neg, zero_part) {
            // `value()>=0` → positive, the main style is the negative section.
            (Some(p), None, None) if pos_inclusive => return format!("{p};{main}"),
            (Some(p), Some(n), Some(z)) if !pos_inclusive => return tail(format!("{p};{n};{z}")),
            (Some(p), Some(n), None) if !pos_inclusive && !text_main => return format!("{p};{n};{main}"),
            (None, Some(n), None) if !text_main => return format!("{main};{n}"),
            _ => {}
        }
    }
    // Other conditions: Excel's own `[>100]` sections (two at most), then the main style.
    let mut out = String::new();
    for (op, n, s) in maps.iter().take(2) {
        out.push_str(&format!("[{op}{}]{s};", crate::xml::num(*n)));
    }
    out.push_str(&main);
    out
}

/// `value()>=0` → (">=", 0).
fn condition(cond: &str) -> Option<(&'static str, f64)> {
    let rest = cond.trim().strip_prefix("value()")?.trim_start();
    let (op, n) = [(">=", ">="), ("<=", "<="), ("!=", "<>"), ("<>", "<>"), (">", ">"), ("<", "<"), ("==", "="), ("=", "=")]
        .iter()
        .find_map(|(prefix, op)| rest.strip_prefix(prefix).map(|n| (*op, n)))?;
    let n = n.trim().parse::<f64>().ok().filter(|n| n.is_finite())?;
    Some((op, n))
}

/// One number style as an Excel format section.
fn section(el: &El) -> String {
    let percent = el.name == "percentage-style";
    let date_like = matches!(el.name.as_str(), "date-style" | "time-style");
    let elapsed = el.name == "time-style" && el.attr("truncate-on-overflow") == Some("false");
    let mut out = String::new();
    if let Some(name) = el.child("text-properties").and_then(|t| t.attr("color")).and_then(color_name) {
        out.push('[');
        out.push_str(name);
        out.push(']');
    }
    let head = out.len();
    let mut elapsed_done = false;
    let mut seen = false;
    for k in &el.children {
        let long = k.attr("style") == Some("long");
        seen |= !matches!(k.name.as_str(), "text-properties" | "map");
        match k.name.as_str() {
            "number" => number_part(k, &mut out),
            "scientific-number" => {
                let int = k.attr_u32("min-integer-digits").unwrap_or(1).clamp(1, 15) as usize;
                let dec = k.attr_u32("decimal-places").unwrap_or(2).min(15) as usize;
                out.push_str(&"0".repeat(int));
                if dec > 0 {
                    out.push('.');
                    out.push_str(&"0".repeat(dec));
                }
                out.push_str(if k.attr("forced-exponent-sign") == Some("false") { "E-" } else { "E+" });
                out.push_str(&"0".repeat(k.attr_u32("min-exponent-digits").unwrap_or(2).clamp(1, 5) as usize));
            }
            "fraction" => {
                if k.attr("min-integer-digits").is_some() {
                    out.push_str("# ");
                }
                out.push_str(&"?".repeat(k.attr_u32("min-numerator-digits").unwrap_or(1).clamp(1, 5) as usize));
                out.push('/');
                match k.attr_u32("denominator-value") {
                    Some(d) if d > 0 => out.push_str(&d.to_string()),
                    _ => out.push_str(&"?".repeat(k.attr_u32("min-denominator-digits").unwrap_or(1).clamp(1, 5) as usize)),
                }
            }
            "text" => literal(&k.text, percent, date_like, &mut out),
            "currency-symbol" => literal(&k.text, false, false, &mut out),
            "text-content" => out.push('@'),
            "fill-character" if !k.text.is_empty() => {
                out.push('*');
                out.push_str(k.text.chars().next().map(String::from).as_deref().unwrap_or(" "));
            }
            "day" => out.push_str(if long { "dd" } else { "d" }),
            "month" => out.push_str(match (k.attr("textual") == Some("true"), long) {
                (true, true) => "mmmm",
                (true, false) => "mmm",
                (false, true) => "mm",
                (false, false) => "m",
            }),
            "year" => out.push_str(if long { "yyyy" } else { "yy" }),
            "day-of-week" => out.push_str(if long { "dddd" } else { "ddd" }),
            "hours" => {
                let h = if long { "hh" } else { "h" };
                if elapsed && !elapsed_done {
                    out.push('[');
                    out.push_str(h);
                    out.push(']');
                    elapsed_done = true;
                } else {
                    out.push_str(h);
                }
            }
            "minutes" => out.push_str(if long { "mm" } else { "m" }),
            "seconds" => {
                out.push_str(if long { "ss" } else { "s" });
                let dec = k.attr_u32("decimal-places").unwrap_or(0).min(3) as usize;
                if dec > 0 {
                    out.push('.');
                    out.push_str(&"0".repeat(dec));
                }
            }
            "am-pm" => out.push_str("AM/PM"),
            _ => {}
        }
    }
    if out.len() == head && (!seen || el.name == "boolean-style") {
        // A style with nothing to show (a boolean style): Excel's General. One with only an
        // empty text shows nothing (`0;-0;;@`).
        out.push_str("General");
    }
    out
}

fn number_part(k: &El, out: &mut String) {
    let decimals = k.attr_u32("decimal-places");
    let grouping = k.attr("grouping") == Some("true");
    let min_int = k.attr_u32("min-integer-digits").unwrap_or(1).min(30) as usize;
    if decimals.is_none() && !grouping {
        // No decimal count: as many as needed, which is Excel's General.
        out.push_str(if min_int == 0 { "#" } else { "General" });
        return;
    }
    if grouping {
        let positions = min_int.max(4);
        let mut digits = Vec::with_capacity(positions + positions / 3);
        for i in 0..positions {
            if i > 0 && i % 3 == 0 {
                digits.push(',');
            }
            digits.push(if i < min_int { '0' } else { '#' });
        }
        out.extend(digits.iter().rev());
    } else if min_int == 0 {
        out.push('#');
    } else {
        out.push_str(&"0".repeat(min_int));
    }
    let decimals = decimals.unwrap_or(0).min(30) as usize;
    let min_dec = k.attr_u32("min-decimal-places").map_or(decimals, |m| (m as usize).min(decimals));
    if decimals > 0 {
        out.push('.');
        out.push_str(&"0".repeat(min_dec));
        out.push_str(&"#".repeat(decimals - min_dec));
    }
    let factor = k.attr("display-factor").and_then(|f| f.trim().parse::<f64>().ok());
    if factor == Some(1000.0) {
        out.push(',');
    } else if factor == Some(1_000_000.0) {
        out.push_str(",,");
    }
}

/// Literal text in a number format: text Excel shows as it is stays bare (`-`, ` `, `/` in a
/// date), other text goes in quotes (`" €"`). A percentage style's `%` stays the percent sign.
fn literal(text: &str, percent: bool, date_like: bool, out: &mut String) {
    let bare = |c: char| matches!(c, ' ' | '-' | ':' | '(' | ')' | '+' | '$') || (c == '/' && date_like);
    let mut first = true;
    for segment in text.split(if percent { '%' } else { '\u{0}' }) {
        if !first {
            out.push('%');
        }
        first = false;
        if segment.chars().all(bare) {
            out.push_str(segment);
        } else {
            out.push('"');
            out.push_str(&segment.replace('"', "\"\\\"\""));
            out.push('"');
        }
    }
}

/// The Excel colour name nearest to an ODF colour (`#ff0000` → `Red`).
fn color_name(hex: &str) -> Option<&'static str> {
    let Color::Rgb(v) = Color::from_hex(hex)? else { return None };
    let [r, g, b] = [(v >> 16) as i32 & 255, (v >> 8) as i32 & 255, v as i32 & 255];
    const NAMES: [(&str, [i32; 3]); 8] = [
        ("Black", [0, 0, 0]),
        ("White", [255, 255, 255]),
        ("Red", [255, 0, 0]),
        ("Green", [0, 255, 0]),
        ("Blue", [0, 0, 255]),
        ("Yellow", [255, 255, 0]),
        ("Magenta", [255, 0, 255]),
        ("Cyan", [0, 255, 255]),
    ];
    NAMES.iter().min_by_key(|(_, [cr, cg, cb])| (r - cr).pow(2) + (g - cg).pow(2) + (b - cb).pow(2)).map(|(n, _)| *n)
}

fn cell_props(el: &El) -> CellProps {
    let mut p = CellProps {
        parent: el.attr("parent-style-name").map(str::to_string),
        data_style: el.attr("data-style-name").map(str::to_string),
        ..Default::default()
    };
    if let Some(c) = el.child("table-cell-properties") {
        match c.attr("background-color") {
            Some("transparent") => p.fill = Some(Fill::default()),
            Some(hex) => p.fill = Color::from_hex(hex).map(Fill::solid),
            None => {}
        }
        if let Some(all) = c.attr("border").and_then(border) {
            p.borders[..4].fill(Some(all));
        }
        for (i, key) in ["border-left", "border-right", "border-top", "border-bottom", "diagonal-tl-br", "diagonal-bl-tr"].iter().enumerate() {
            if let Some(line) = c.attr(key).and_then(border) {
                p.borders[i] = Some(line);
            }
        }
        match c.attr("wrap-option") {
            Some("wrap") => p.wrap = Some(true),
            Some("no-wrap") => p.wrap = Some(false),
            _ => {}
        }
        p.shrink = c.attr_bool("shrink-to-fit");
        p.v = match c.attr("vertical-align") {
            Some("top") => Some(VAlign::Top),
            Some("middle") => Some(VAlign::Center),
            Some("bottom" | "automatic") => Some(VAlign::Bottom),
            _ => None,
        };
        if c.attr("vertical-justify") == Some("justify") {
            p.v = Some(VAlign::Justify);
        }
        match c.attr("text-align-source") {
            Some("value-type") => p.align_by_value = Some(true),
            Some("fix") => p.align_by_value = Some(false),
            _ => {}
        }
        if c.attr("repeat-content") == Some("true") {
            p.h = Some(HAlign::Fill);
        }
        if c.attr("direction") == Some("ttb") {
            p.rotation = Some(255);
        } else if let Some(angle) =
            c.attr("rotation-angle").and_then(|a| a.trim().trim_end_matches("deg").parse::<f64>().ok()).filter(|a| a.is_finite())
        {
            let a = angle.rem_euclid(360.0).round() as i16;
            p.rotation = Some(match a {
                0..=90 => a,
                270..=359 => a - 360,
                _ => 0,
            });
        }
        if let Some(protect) = c.attr("cell-protect") {
            let words: Vec<&str> = protect.split_whitespace().collect();
            p.locked = Some(words.iter().any(|w| matches!(*w, "protected" | "hidden-and-protected")));
            p.hidden = Some(words.iter().any(|w| matches!(*w, "formula-hidden" | "hidden-and-protected")));
        }
    }
    if let Some(par) = el.child("paragraph-properties") {
        if let Some(h) = par.attr("text-align") {
            let h = match h {
                "start" | "left" => HAlign::Left,
                "end" | "right" => HAlign::Right,
                "center" => HAlign::Center,
                "justify" => HAlign::Justify,
                _ => HAlign::General,
            };
            if p.h != Some(HAlign::Fill) {
                p.h = Some(h);
            }
        }
        if let Some(px) = par.attr("margin-left").and_then(length_px) {
            p.indent = Some((px / INDENT_PX).round().clamp(0.0, 250.0) as u8);
        }
    }
    if let Some(t) = el.child("text-properties") {
        p.font_name = t.attr("font-name").map(str::to_string).or_else(|| t.attr("font-family").map(unquote));
        p.font_family = t.attr("font-family-generic").and_then(generic_family);
        p.size = t.attr("font-size").and_then(length_px).map(|px| (px * 0.75 * 2.0).round() / 2.0).filter(|s| *s > 0.0 && *s <= 409.0);
        p.bold = t.attr("font-weight").map(|w| w == "bold" || w.parse::<u32>().is_ok_and(|n| n >= 600));
        p.italic = t.attr("font-style").map(|s| s == "italic" || s == "oblique");
        p.underline = t.attr("text-underline-style").map(|u| match (u, t.attr("text-underline-type")) {
            ("none", _) => Underline::None,
            (_, Some("double")) => Underline::Double,
            _ => Underline::Single,
        });
        p.strike = t.attr("text-line-through-style").map(|s| s != "none");
        if t.attr("use-window-font-color") == Some("true") {
            p.color = Some(Color::Auto);
        } else if let Some(c) = t.attr("color").and_then(Color::from_hex) {
            p.color = Some(c);
        }
        p.vert = t.attr("text-position").map(|pos| {
            let first = pos.split_whitespace().next().unwrap_or("");
            if first == "super" || first.trim_end_matches('%').parse::<f64>().is_ok_and(|n| n > 0.0) {
                VertAlign::Superscript
            } else if first == "sub" || first.trim_end_matches('%').parse::<f64>().is_ok_and(|n| n < 0.0) {
                VertAlign::Subscript
            } else {
                VertAlign::Baseline
            }
        });
    }
    p
}

/// One indent step (Excel's indent level) in pixels: 10 points, as LibreOffice converts them.
pub(super) const INDENT_PX: f32 = 10.0 / 0.75;

/// `0.74pt solid #000000` → a thin black line; `none` → no line.
fn border(s: &str) -> Option<BorderLine> {
    let mut width_pt = 0.75f32;
    let mut kind = None;
    let mut color = Color::Auto;
    for tok in s.split_whitespace() {
        if tok.starts_with('#') {
            color = Color::from_hex(tok).unwrap_or(Color::Auto);
        } else if let Some(px) = length_px(tok) {
            width_pt = px * 0.75;
        } else {
            match tok {
                "none" | "hidden" => return Some(BorderLine::default()),
                "thin" => width_pt = 0.75,
                "medium" => width_pt = 1.76,
                "thick" => width_pt = 2.5,
                other => kind = Some(other.to_string()),
            }
        }
    }
    let heavy = width_pt > 1.25;
    let style = match kind.as_deref()? {
        "solid" | "groove" | "ridge" | "inset" | "outset" if width_pt > 2.0 => BorderStyle::Thick,
        "solid" | "groove" | "ridge" | "inset" | "outset" if heavy => BorderStyle::Medium,
        "double" | "double-thin" => BorderStyle::Double,
        "dotted" => BorderStyle::Dotted,
        "dashed" | "fine-dashed" | "dash" if heavy => BorderStyle::MediumDashed,
        "dashed" | "fine-dashed" | "dash" => BorderStyle::Dashed,
        "dash-dot" if heavy => BorderStyle::MediumDashDot,
        "dash-dot" => BorderStyle::DashDot,
        "dash-dot-dot" if heavy => BorderStyle::MediumDashDotDot,
        "dash-dot-dot" => BorderStyle::DashDotDot,
        _ => BorderStyle::Thin,
    };
    Some(BorderLine { style, color })
}

/// A CSS length (`2.258cm`, `0.8in`, `10pt`) in pixels at 96 dpi.
pub(super) fn length_px(s: &str) -> Option<f32> {
    let s = s.trim();
    let split = s.find(|c: char| c.is_ascii_alphabetic() || c == '%')?;
    let (n, unit) = s.split_at(split);
    let n = n.trim().parse::<f64>().ok().filter(|n| n.is_finite() && n.abs() < 1e7)?;
    let px = match unit {
        "cm" => n * 96.0 / 2.54,
        "mm" => n * 96.0 / 25.4,
        "in" | "inch" => n * 96.0,
        "pt" => n * 96.0 / 72.0,
        "pc" => n * 16.0,
        "px" => n,
        _ => return None,
    };
    Some(px as f32)
}

fn generic_family(g: &str) -> Option<u8> {
    match g {
        "roman" => Some(1),
        "swiss" => Some(2),
        "modern" => Some(3),
        "script" => Some(4),
        "decorative" => Some(5),
        _ => None,
    }
}

/// `'Liberation Sans', sans-serif` → `Liberation Sans`.
fn unquote(family: &str) -> String {
    let first = family.split(',').next().unwrap_or("").trim();
    first.trim_matches(|c| c == '\'' || c == '"').trim().to_string()
}

/// Text of an element and its descendants (`text:p` with spans), with `text:s` and
/// `text:line-break` expanded.
pub(super) fn el_text(el: &El) -> String {
    let mut out = el.text.clone();
    for k in &el.children {
        match k.name.as_str() {
            "s" => out.push_str(&" ".repeat(k.attr_u32("c").unwrap_or(1).min(1000) as usize)),
            "tab" => out.push('\t'),
            "line-break" => out.push('\n'),
            "p" | "h" => {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&el_text(k));
            }
            _ => out.push_str(&el_text(k)),
        }
    }
    out
}

/// The part of content.xml before `office:body` (fonts and automatic styles), as a small tree:
/// the body itself can be large, so it is not built into one.
pub(super) fn content_head(bytes: &[u8]) -> Option<El> {
    let mut reader = quick_xml::Reader::from_reader(bytes);
    let mut depth = 0usize;
    let mut start = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event().ok()? {
            quick_xml::events::Event::Start(e) => {
                if depth == 0 {
                    start = Some(reader.buffer_position() as usize);
                } else if depth == 1 && local(e.name().as_ref()) == b"body" {
                    let mut out = b"<r>".to_vec();
                    out.extend_from_slice(bytes.get(start?..before)?);
                    out.extend_from_slice(b"</r>");
                    return crate::xml::parse(&out).ok();
                }
                depth += 1;
            }
            quick_xml::events::Event::End(_) => depth = depth.checked_sub(1)?,
            quick_xml::events::Event::Eof => return None,
            _ => {}
        }
    }
}

fn local(name: &[u8]) -> &[u8] {
    name.iter().position(|&b| b == b':').map_or(name, |i| &name[i + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number_style(xml: &str) -> String {
        let root = crate::xml::parse(format!("<r><office:styles>{xml}</office:styles></r>").as_bytes()).unwrap();
        let mut styles = OdsStyles::default();
        styles.add(&root, false);
        styles.number_code("N1").unwrap()
    }

    #[test]
    fn number_styles_become_excel_codes() {
        assert_eq!(
            number_style(
                r##"<number:number-style style:name="N1"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/><number:text> €</number:text></number:number-style>"##
            ),
            "#,##0.00\" €\""
        );
        assert_eq!(
            number_style(
                r##"<number:number-style style:name="N1P0" style:volatile="true"><number:number number:decimal-places="2" number:min-integer-digits="1" number:grouping="true"/></number:number-style><number:number-style style:name="N1"><style:text-properties fo:color="#ff0000"/><number:text>-</number:text><number:number number:decimal-places="2" number:min-integer-digits="1" number:grouping="true"/><style:map style:condition="value()&gt;=0" style:apply-style-name="N1P0"/></number:number-style>"##
            ),
            "#,##0.00;[Red]-#,##0.00"
        );
        assert_eq!(
            number_style(
                r##"<number:percentage-style style:name="N1"><number:number number:decimal-places="1" number:min-integer-digits="1"/><number:text>%</number:text></number:percentage-style>"##
            ),
            "0.0%"
        );
        assert_eq!(
            number_style(
                r##"<number:date-style style:name="N1"><number:day number:style="long"/><number:text>/</number:text><number:month number:style="long"/><number:text>/</number:text><number:year number:style="long"/></number:date-style>"##
            ),
            "dd/mm/yyyy"
        );
        assert_eq!(
            number_style(
                r##"<number:time-style style:name="N1" number:truncate-on-overflow="false"><number:hours number:style="long"/><number:text>:</number:text><number:minutes number:style="long"/></number:time-style>"##
            ),
            "[hh]:mm"
        );
        assert_eq!(
            number_style(
                r##"<number:number-style style:name="N1"><number:scientific-number number:decimal-places="2" number:min-integer-digits="1" number:min-exponent-digits="2"/></number:number-style>"##
            ),
            "0.00E+00"
        );
        assert_eq!(
            number_style(r##"<number:number-style style:name="N1"><number:number number:min-integer-digits="1"/></number:number-style>"##),
            "General"
        );
        assert_eq!(
            number_style(
                r##"<number:currency-style style:name="N1"><number:number number:decimal-places="2" number:min-integer-digits="1" number:grouping="true"/><number:text> </number:text><number:currency-symbol number:language="fr" number:country="FR">€</number:currency-symbol></number:currency-style>"##
            ),
            "#,##0.00 \"€\""
        );
    }

    #[test]
    fn lengths_and_borders() {
        assert_eq!(length_px("1in"), Some(96.0));
        assert!((length_px("2.54cm").unwrap() - 96.0).abs() < 0.01);
        assert_eq!(length_px("12pt"), Some(16.0));
        assert_eq!(length_px("50%"), None);
        assert_eq!(border("0.74pt solid #000000"), Some(BorderLine { style: BorderStyle::Thin, color: Color::Rgb(0) }));
        assert_eq!(border("1.76pt solid #ff0000").map(|b| b.style), Some(BorderStyle::Medium));
        assert_eq!(border("2.49pt solid #ff0000").map(|b| b.style), Some(BorderStyle::Thick));
        assert_eq!(border("none"), Some(BorderLine::default()));
        assert_eq!(border("0.74pt dashed #000000").map(|b| b.style), Some(BorderStyle::Dashed));
    }
}
