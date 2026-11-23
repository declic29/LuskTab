//! Lusk Tab : saving a workbook as an OpenDocument Spreadsheet (.ods), for LibreOffice Calc.
//! Values, formulas (in OpenFormula), cell formatting, number formats, column widths, row
//! heights, merged cells, frozen panes, hidden sheets, tab colours, defined names and charts
//! are written. Format reference: ODF 1.3 parts 2 (packages), 3 (schema) and 4 (formulas).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;

use gridcraft_core::date::datetime_from_serial;
use gridcraft_core::{CellRef, MAX_COLS, RangeRef, Value, col_to_letters};
use gridcraft_formula::{BinOp, Expr, RefKind, Reference, SheetSel, UnOp};
use gridcraft_model::{
    AnchorMode, BorderLine, BorderStyle, Cell, Chart, ChartKind, Color, HAlign, LegendPos, PatternType, Sheet, Style, StyleId, Underline, VAlign,
    VertAlign, Visibility, Workbook,
};

use super::numfmt::{Kind, NumberStyles};
use super::style::INDENT_PX;
use crate::IoError;
use crate::package::ZipOut;
use crate::xml::{esc, esc_attr, num};

const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:chart="urn:oasis:names:tc:opendocument:xmlns:chart:1.0" xmlns:of="urn:oasis:names:tc:opendocument:xmlns:of:1.2" xmlns:config="urn:oasis:names:tc:opendocument:xmlns:config:1.0" xmlns:calcext="urn:org:documentfoundation:names:experimental:calc:xmlns:calcext:1.0" xmlns:loext="urn:org:documentfoundation:names:experimental:office:xmlns:loext:1.0" xmlns:tableooo="http://openoffice.org/2009/table" xmlns:ooo="http://openoffice.org/2004/office""#;
const MIME: &str = "application/vnd.oasis.opendocument.spreadsheet";
/// LibreOffice's sheets have 16384 columns; the last column group runs to its classic 1024 so
/// older versions open the file without a "too many columns" warning.
const COLUMN_PAD: u32 = 1024;

/// The workbook as an .ods package.
pub fn write_ods(wb: &Workbook) -> Result<Vec<u8>, IoError> {
    let mut w = Writer {
        wb,
        numbers: NumberStyles::new("N"),
        cell_styles: HashMap::new(),
        cell_xml: String::new(),
        col_styles: HashMap::new(),
        row_styles: HashMap::new(),
        table_styles: HashMap::new(),
        other_xml: String::new(),
        fonts: BTreeSet::new(),
        objects: vec![],
    };
    let body = w.body();
    let content = w.content(&body);
    let styles = w.styles_xml();
    let mut zip = ZipOut::new();
    zip.add_stored("mimetype", MIME.as_bytes())?;
    let mut manifest = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3"><manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="settings.xml" manifest:media-type="text/xml"/>"#,
    );
    for (dir, _) in &w.objects {
        let _ = write!(
            manifest,
            r#"<manifest:file-entry manifest:full-path="{dir}/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.chart"/><manifest:file-entry manifest:full-path="{dir}/content.xml" manifest:media-type="text/xml"/>"#
        );
    }
    manifest.push_str("</manifest:manifest>");
    zip.add("META-INF/manifest.xml", manifest.as_bytes())?;
    zip.add("content.xml", content.as_bytes())?;
    zip.add("styles.xml", styles.as_bytes())?;
    zip.add("meta.xml", w.meta().as_bytes())?;
    zip.add("settings.xml", w.settings().as_bytes())?;
    for (dir, xml) in &w.objects {
        zip.add(&format!("{dir}/content.xml"), xml.as_bytes())?;
    }
    zip.finish()
}

struct Writer<'a> {
    wb: &'a Workbook,
    numbers: NumberStyles,
    cell_styles: HashMap<StyleId, String>,
    cell_xml: String,
    col_styles: HashMap<u32, String>,
    row_styles: HashMap<(u32, bool), String>,
    table_styles: HashMap<(bool, Option<u32>, bool), String>,
    other_xml: String,
    fonts: BTreeSet<String>,
    /// Chart objects: directory (`Object 1`) and content.xml.
    objects: Vec<(String, String)>,
}

fn hex(c: Color, wb: &Workbook) -> Option<String> {
    c.resolve(&wb.theme).map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"))
}

fn length_in(px: f32) -> String {
    format!("{}in", num(((px as f64) / 96.0 * 10_000.0).round() / 10_000.0))
}

fn border_xml(line: &BorderLine, wb: &Workbook) -> String {
    let (width, kind) = match line.style {
        BorderStyle::None => return "none".into(),
        BorderStyle::Hair | BorderStyle::Thin => ("0.74pt", "solid"),
        BorderStyle::Medium => ("1.76pt", "solid"),
        BorderStyle::Thick => ("2.49pt", "solid"),
        BorderStyle::Dashed => ("0.74pt", "dashed"),
        BorderStyle::MediumDashed => ("1.76pt", "dashed"),
        BorderStyle::Dotted => ("0.74pt", "dotted"),
        BorderStyle::Double => ("2.49pt", "double"),
        BorderStyle::DashDot => ("0.74pt", "dash-dot"),
        BorderStyle::MediumDashDot | BorderStyle::SlantDashDot => ("1.76pt", "dash-dot"),
        BorderStyle::DashDotDot => ("0.74pt", "dash-dot-dot"),
        BorderStyle::MediumDashDotDot => ("1.76pt", "dash-dot-dot"),
    };
    format!("{width} {kind} {}", hex(line.color, wb).unwrap_or_else(|| "#000000".into()))
}

impl Writer<'_> {
    /// The properties of a cell style (`style:table-cell-properties` and the others) that differ
    /// from `base`, the style it inherits from, with the data style it uses. `named` is the
    /// default style (styles.xml's `Default`, based on ODF's defaults); `numbers` is where its
    /// number style goes. Only differences are written, as LibreOffice writes them: it fails to
    /// open some files whose default style states every property.
    fn props(&mut self, s: &Style, named: bool, numbers: &mut NumberStyles) -> (String, Option<String>) {
        let wb = self.wb;
        let odf_default = Style::default();
        let normal = wb.styles.get(StyleId::DEFAULT);
        let base = if named { &odf_default } else { normal };
        let mut cell = String::new();
        if s.fill != base.fill {
            let color = match s.fill.pattern {
                PatternType::None => None,
                PatternType::Solid => hex(s.fill.fg, wb),
                _ => hex(if s.fill.fg == Color::Auto { s.fill.bg } else { s.fill.fg }, wb),
            };
            let _ = write!(cell, r#" fo:background-color="{}""#, color.unwrap_or_else(|| "transparent".into()));
        }
        let (b, bb) = (&s.borders, &base.borders);
        for (key, line, was) in [
            ("fo:border-left", &b.left, &bb.left),
            ("fo:border-right", &b.right, &bb.right),
            ("fo:border-top", &b.top, &bb.top),
            ("fo:border-bottom", &b.bottom, &bb.bottom),
            ("style:diagonal-tl-br", &b.diag_down, &bb.diag_down),
            ("style:diagonal-bl-tr", &b.diag_up, &bb.diag_up),
        ] {
            if line != was {
                let _ = write!(cell, r#" {key}="{}""#, border_xml(line, wb));
            }
        }
        let (a, ab) = (&s.align, &base.align);
        if a.wrap != ab.wrap {
            let _ = write!(cell, r#" fo:wrap-option="{}""#, if a.wrap { "wrap" } else { "no-wrap" });
        }
        if a.v != ab.v {
            let v = match a.v {
                VAlign::Top | VAlign::Justify => "top",
                VAlign::Center | VAlign::Distributed => "middle",
                VAlign::Bottom => "bottom",
            };
            let _ = write!(cell, r#" style:vertical-align="{v}""#);
            if a.v == VAlign::Justify {
                cell.push_str(r#" loext:vertical-justify="justify""#);
            }
        }
        if a.shrink != ab.shrink {
            let _ = write!(cell, r#" style:shrink-to-fit="{}""#, a.shrink);
        }
        if a.rotation != ab.rotation {
            if a.rotation == 255 {
                cell.push_str(r#" style:direction="ttb""#);
            } else {
                let _ = write!(cell, r#" style:direction="ltr" style:rotation-angle="{}""#, i32::from(a.rotation).rem_euclid(360));
            }
        }
        if a.h != ab.h {
            let general = a.h == HAlign::General;
            let _ = write!(
                cell,
                r#" style:text-align-source="{}" style:repeat-content="{}""#,
                if general { "value-type" } else { "fix" },
                a.h == HAlign::Fill
            );
        }
        if s.protection != base.protection {
            let protect = match (s.protection.locked, s.protection.hidden) {
                (true, true) => "hidden-and-protected",
                (true, false) => "protected",
                (false, true) => "formula-hidden",
                (false, false) => "none",
            };
            let _ = write!(cell, r#" style:cell-protect="{protect}""#);
        }
        let mut out = String::new();
        if !cell.is_empty() {
            let _ = write!(out, "<style:table-cell-properties{cell}/>");
        }
        let mut para = String::new();
        if a.h != ab.h && a.h != HAlign::General {
            let h = match a.h {
                HAlign::Center | HAlign::CenterAcross => "center",
                HAlign::Right => "end",
                HAlign::Justify | HAlign::Distributed => "justify",
                _ => "start",
            };
            let _ = write!(para, r#" fo:text-align="{h}""#);
        }
        if a.indent != ab.indent {
            let _ = write!(para, r#" fo:margin-left="{}""#, length_in(f32::from(a.indent) * INDENT_PX));
        }
        if !para.is_empty() {
            let _ = write!(out, "<style:paragraph-properties{para}/>");
        }
        if named || s.font.name != base.font.name {
            self.fonts.insert(s.font.name.clone());
        }
        let text = text_props(&s.font, &base.font, named, wb);
        if !text.is_empty() {
            let _ = write!(out, "<style:text-properties{text}/>");
        }
        let data = numbers.name(s.num_fmt.as_str()).map(|(n, _)| n);
        (out, data)
    }

    /// The content.xml name of a workbook style (`Default` for the default style).
    fn cell_style(&mut self, id: StyleId) -> String {
        if id == StyleId::DEFAULT {
            return "Default".into();
        }
        if let Some(n) = self.cell_styles.get(&id) {
            return n.clone();
        }
        let name = format!("ce{}", self.cell_styles.len() + 1);
        let style = self.wb.styles.get(id).clone();
        let mut numbers = std::mem::take(&mut self.numbers);
        let (props, data) = self.props(&style, false, &mut numbers);
        self.numbers = numbers;
        let data = data.map(|d| format!(r#" style:data-style-name="{d}""#)).unwrap_or_default();
        let _ = write!(
            self.cell_xml,
            r#"<style:style style:name="{name}" style:family="table-cell" style:parent-style-name="Default"{data}>{props}</style:style>"#
        );
        self.cell_styles.insert(id, name.clone());
        name
    }

    fn col_style(&mut self, width: f32) -> String {
        let key = width.to_bits();
        if let Some(n) = self.col_styles.get(&key) {
            return n.clone();
        }
        let name = format!("co{}", self.col_styles.len() + 1);
        let _ = write!(
            self.other_xml,
            r#"<style:style style:name="{name}" style:family="table-column"><style:table-column-properties fo:break-before="auto" style:column-width="{}"/></style:style>"#,
            length_in(width)
        );
        self.col_styles.insert(key, name.clone());
        name
    }

    fn row_style(&mut self, height: f32, custom: bool) -> String {
        let key = (height.to_bits(), custom);
        if let Some(n) = self.row_styles.get(&key) {
            return n.clone();
        }
        let name = format!("ro{}", self.row_styles.len() + 1);
        let _ = write!(
            self.other_xml,
            r#"<style:style style:name="{name}" style:family="table-row"><style:table-row-properties style:row-height="{}" fo:break-before="auto" style:use-optimal-row-height="{}"/></style:style>"#,
            length_in(height),
            !custom
        );
        self.row_styles.insert(key, name.clone());
        name
    }

    fn table_style(&mut self, sheet: &Sheet) -> String {
        let hidden = sheet.visibility != Visibility::Visible;
        let tab = sheet.tab_color.and_then(|c| c.resolve(&self.wb.theme)).map(|[r, g, b]| (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b));
        let key = (hidden, tab, sheet.right_to_left);
        if let Some(n) = self.table_styles.get(&key) {
            return n.clone();
        }
        let name = format!("ta{}", self.table_styles.len() + 1);
        let tab_attr = tab.map(|t| format!(r##" tableooo:tab-color="#{t:06x}""##)).unwrap_or_default();
        let _ = write!(
            self.other_xml,
            r#"<style:style style:name="{name}" style:family="table" style:master-page-name="Default"><style:table-properties table:display="{}" style:writing-mode="{}"{tab_attr}/></style:style>"#,
            !hidden,
            if sheet.right_to_left { "rl-tb" } else { "lr-tb" }
        );
        self.table_styles.insert(key, name.clone());
        name
    }

    fn body(&mut self) -> String {
        let mut out = String::new();
        for sheet in &self.wb.sheets {
            self.sheet(sheet, &mut out);
        }
        self.names(&mut out);
        out
    }

    fn sheet(&mut self, sheet: &Sheet, out: &mut String) {
        let ta = self.table_style(sheet);
        let _ = write!(out, r#"<table:table table:name="{}" table:style-name="{ta}">"#, esc_attr(&sheet.name));
        // The used area: cells, merges, charts and columns or rows given a size.
        let used = sheet.cells.used_range();
        let mut last_col = used.map_or(0, |r| r.end.col);
        let mut last_row = used.map_or(0, |r| r.end.row);
        for m in &sheet.merges {
            last_col = last_col.max(m.end.col);
            last_row = last_row.max(m.end.row);
        }
        for c in &sheet.charts {
            last_col = last_col.max(c.anchor.cell.col);
            last_row = last_row.max(c.anchor.cell.row);
        }
        if let Some((&c, _)) = sheet.cols.iter().next_back() {
            last_col = last_col.max(c.min(MAX_COLS - 1));
        }
        if let Some((&r, _)) = sheet.rows.iter().next_back() {
            last_row = last_row.max(r);
        }
        // Columns, in runs of the same width, visibility and default style.
        let column = |c: u32| -> (f32, bool, Option<StyleId>) {
            let info = sheet.cols.get(&c);
            (info.and_then(|i| i.size).unwrap_or(sheet.default_col_width), info.is_some_and(|i| i.hidden), info.and_then(|i| i.style))
        };
        let pad = COLUMN_PAD.max(last_col + 1);
        let mut c = 0;
        while c < pad {
            let props = column(c);
            let mut n = 1;
            while c + n < pad && column(c + n) == props {
                n += 1;
            }
            let co = self.col_style(props.0);
            let default = match props.2 {
                Some(id) => self.cell_style(id),
                None => "Default".into(),
            };
            let _ = write!(out, r#"<table:table-column table:style-name="{co}""#);
            if n > 1 {
                let _ = write!(out, r#" table:number-columns-repeated="{n}""#);
            }
            if props.1 {
                out.push_str(r#" table:visibility="collapse""#);
            }
            let _ = write!(out, r#" table:default-cell-style-name="{default}"/>"#);
            c += n;
        }
        // Merges by first row; charts by anchor cell.
        let mut merges: Vec<RangeRef> = sheet.merges.iter().map(|m| RangeRef::new(m.start, m.end)).collect();
        merges.sort_by_key(|m| (m.start.row, m.start.col));
        // A chart anchored under a merge (not its first cell) moves to the merge's first cell.
        let mut moved: Vec<Chart> = Vec::new();
        for ch in &sheet.charts {
            if let Some(m) = merges.iter().find(|m| m.contains(ch.anchor.cell) && m.start != ch.anchor.cell) {
                let mut ch = ch.clone();
                ch.anchor.dx += (m.start.col..ch.anchor.cell.col).map(|c| super::column_width(sheet, c)).sum::<f32>();
                ch.anchor.dy += (m.start.row..ch.anchor.cell.row).map(|r| super::row_height(sheet, r)).sum::<f32>();
                ch.anchor.cell = m.start;
                moved.push(ch);
            }
        }
        let mut charts: BTreeMap<CellRef, Vec<&Chart>> = BTreeMap::new();
        for ch in sheet.charts.iter().filter(|ch| !merges.iter().any(|m| m.contains(ch.anchor.cell) && m.start != ch.anchor.cell)).chain(&moved) {
            charts.entry(ch.anchor.cell).or_default().push(ch);
        }
        let mut active: Vec<RangeRef> = Vec::new();
        let mut next_merge = 0;
        let mut r = 0u32;
        while r <= last_row {
            active.retain(|m| m.end.row >= r);
            while next_merge < merges.len() && merges[next_merge].start.row == r {
                active.push(merges[next_merge]);
                next_merge += 1;
            }
            let has_cells = sheet.cells.row(r, 0, MAX_COLS - 1).next().is_some()
                || sheet.comments.range(CellRef::new(r, 0)..=CellRef::new(r, MAX_COLS - 1)).next().is_some();
            let has_charts = charts.range(CellRef::new(r, 0)..=CellRef::new(r, MAX_COLS - 1)).next().is_some();
            let row_key = self.row_props(sheet, r);
            if !has_cells && !has_charts && active.is_empty() {
                // A run of empty rows with the same height.
                let mut n = 1;
                while r + n <= last_row
                    && self.row_props(sheet, r + n) == row_key
                    && sheet.cells.row(r + n, 0, MAX_COLS - 1).next().is_none()
                    && sheet.comments.range(CellRef::new(r + n, 0)..=CellRef::new(r + n, MAX_COLS - 1)).next().is_none()
                    && !charts.range(CellRef::new(r + n, 0)..=CellRef::new(r + n, MAX_COLS - 1)).any(|_| true)
                    && merges.get(next_merge).is_none_or(|m| m.start.row != r + n)
                {
                    n += 1;
                }
                self.row_open(out, sheet, r, n);
                let _ = write!(out, r#"<table:table-cell table:number-columns-repeated="{}"/></table:table-row>"#, last_col + 1);
                r += n;
                continue;
            }
            self.row_open(out, sheet, r, 1);
            // Covered parts of merges on this row, by first column.
            let mut covered: Vec<(u32, u32)> = active
                .iter()
                .filter_map(|m| {
                    if m.start.row == r { (m.end.col > m.start.col).then(|| (m.start.col + 1, m.end.col)) } else { Some((m.start.col, m.end.col)) }
                })
                .collect();
            covered.sort_unstable();
            let anchors: HashMap<u32, (u32, u32)> = active
                .iter()
                .filter(|m| m.start.row == r)
                .map(|m| (m.start.col, (m.end.col - m.start.col + 1, m.end.row - m.start.row + 1)))
                .collect();
            let mut cols: BTreeSet<u32> = sheet.cells.row(r, 0, MAX_COLS - 1).map(|(c, _)| c).collect();
            cols.extend(anchors.keys().copied());
            cols.extend(sheet.comments.range(CellRef::new(r, 0)..=CellRef::new(r, MAX_COLS - 1)).map(|(p, _)| p.col));
            cols.extend(charts.range(CellRef::new(r, 0)..=CellRef::new(r, MAX_COLS - 1)).map(|(p, _)| p.col));
            let mut cursor = 0u32;
            let mut spans = covered.iter().peekable();
            let mut items = cols.iter().peekable();
            loop {
                let next_span = spans.peek().map(|s| s.0);
                let next_item = items.peek().map(|c| **c);
                match (next_span, next_item) {
                    (None, None) => break,
                    (Some(s), item) if item.is_none_or(|i| s <= i) => {
                        let (c0, c1) = **spans.peek().unwrap_or(&&(s, s));
                        spans.next();
                        gap(out, cursor, c0);
                        let n = c1 - c0 + 1;
                        if n > 1 {
                            let _ = write!(out, r#"<table:covered-table-cell table:number-columns-repeated="{n}"/>"#);
                        } else {
                            out.push_str("<table:covered-table-cell/>");
                        }
                        cursor = c1 + 1;
                        // Values under a merge are hidden by it, and not written.
                        while items.peek().is_some_and(|c| **c <= c1) {
                            items.next();
                        }
                    }
                    (_, Some(c)) => {
                        items.next();
                        gap(out, cursor, c);
                        let pos = CellRef::new(r, c);
                        let cell = sheet.cells.get(pos);
                        let frames = charts.get(&pos).map(|v| v.as_slice()).unwrap_or(&[]);
                        self.cell(out, sheet, pos, cell, anchors.get(&c).copied(), frames);
                        cursor = c + 1;
                    }
                    _ => break,
                }
            }
            out.push_str("</table:table-row>");
            r += 1;
        }
        out.push_str("</table:table>");
    }

    /// A row's height style and visibility.
    fn row_props(&self, sheet: &Sheet, r: u32) -> (u32, bool, bool, Option<StyleId>) {
        let info = sheet.rows.get(&r);
        let custom = info.is_some_and(|i| i.size.is_some());
        let height = info.and_then(|i| i.size).unwrap_or(sheet.default_row_height);
        (height.to_bits(), custom, info.is_some_and(|i| i.hidden), info.and_then(|i| i.style))
    }

    fn row_open(&mut self, out: &mut String, sheet: &Sheet, r: u32, n: u32) {
        let (height, custom, hidden, style) = self.row_props(sheet, r);
        let ro = self.row_style(f32::from_bits(height), custom);
        let _ = write!(out, r#"<table:table-row table:style-name="{ro}""#);
        if n > 1 {
            let _ = write!(out, r#" table:number-rows-repeated="{n}""#);
        }
        if hidden {
            out.push_str(r#" table:visibility="collapse""#);
        }
        if let Some(id) = style {
            let name = self.cell_style(id);
            let _ = write!(out, r#" table:default-cell-style-name="{name}""#);
        }
        out.push('>');
    }

    fn cell(&mut self, out: &mut String, sheet: &Sheet, pos: CellRef, cell: Option<&Cell>, span: Option<(u32, u32)>, charts: &[&Chart]) {
        let wb = self.wb;
        out.push_str("<table:table-cell");
        let style = cell.map_or(StyleId::DEFAULT, |c| c.style);
        let column_style = sheet.cols.get(&pos.col).and_then(|i| i.style).or_else(|| sheet.rows.get(&pos.row).and_then(|i| i.style));
        if style != StyleId::DEFAULT || column_style.is_some_and(|s| s != StyleId::DEFAULT) && cell.is_some() {
            let name = self.cell_style(style);
            let _ = write!(out, r#" table:style-name="{name}""#);
        }
        if let Some((w, h)) = span {
            let _ = write!(out, r#" table:number-columns-spanned="{w}" table:number-rows-spanned="{h}""#);
        }
        let mut text: Option<String> = None;
        if let Some(cell) = cell {
            if let Some(f) = &cell.formula
                && let Some(of) = formula(f)
            {
                let _ = write!(out, r#" table:formula="of:={}""#, esc_attr(&of));
            }
            let fmt = wb.styles.get(style).num_fmt.as_str().to_string();
            let kind = self.numbers.name(&fmt).map(|(_, k)| k);
            match &cell.value {
                Value::Number(n) => {
                    let date = match kind {
                        Some(Kind::Date) => datetime_from_serial(wb.date_system, *n)
                            .filter(|d| d.year >= 1900 && !(d.month == 2 && d.day == 29 && d.year == 1900) && d.day > 0),
                        _ => None,
                    };
                    if let Some(d) = date {
                        let _ = write!(
                            out,
                            r#" office:value-type="date" office:date-value="{:04}-{:02}-{:02}T{:02}:{:02}:{:02}" calcext:value-type="date""#,
                            d.year, d.month, d.day, d.hour, d.minute, d.second
                        );
                    } else if kind == Some(Kind::Time) && n.is_finite() && *n >= 0.0 {
                        let total = (n * 86_400.0).round() as u64;
                        let _ = write!(
                            out,
                            r#" office:value-type="time" office:time-value="PT{:02}H{:02}M{:02}S" calcext:value-type="time""#,
                            total / 3600,
                            (total / 60) % 60,
                            total % 60
                        );
                    } else if kind == Some(Kind::Percent) {
                        let _ = write!(out, r#" office:value-type="percentage" office:value="{}" calcext:value-type="percentage""#, num(*n));
                    } else {
                        let _ = write!(out, r#" office:value-type="float" office:value="{}" calcext:value-type="float""#, num(*n));
                    }
                    text = Some(num(*n));
                }
                Value::Bool(b) => {
                    let _ = write!(out, r#" office:value-type="boolean" office:boolean-value="{b}" calcext:value-type="boolean""#);
                    text = Some(if *b { "TRUE" } else { "FALSE" }.into());
                }
                Value::Text(t) => {
                    out.push_str(r#" office:value-type="string" calcext:value-type="string""#);
                    text = Some(t.to_string());
                }
                Value::Error(e) => {
                    out.push_str(r#" office:value-type="string" office:string-value="" calcext:value-type="error""#);
                    text = Some(e.as_str().to_string());
                }
                Value::Array(_) | Value::Empty => {}
            }
        }
        let comment = sheet.comments.get(&pos);
        if text.is_none() && charts.is_empty() && comment.is_none() {
            out.push_str("/>");
            return;
        }
        out.push('>');
        for ch in charts {
            self.frame(out, sheet, ch);
        }
        if let Some(c) = comment {
            let _ = write!(out, r#"<office:annotation office:display="{}">"#, c.visible);
            if !c.author.is_empty() {
                let _ = write!(out, "<dc:creator>{}</dc:creator>", esc(&c.author));
            }
            let mut body = c.text.clone();
            // A threaded comment's replies follow its text, each with its author.
            for (author, reply) in &c.replies {
                let _ = write!(body, "\n{author} : {reply}");
            }
            paragraphs(out, &body);
            out.push_str("</office:annotation>");
        }
        if let Some(t) = text {
            paragraphs(out, &t);
        }
        out.push_str("</table:table-cell>");
    }

    /// A chart's frame in its anchor cell, and its object.
    fn frame(&mut self, out: &mut String, sheet: &Sheet, ch: &Chart) {
        let dir = format!("Object {}", self.objects.len() + 1);
        let Some(xml) = chart_xml(ch, self.wb) else { return };
        let a = &ch.anchor;
        let _ = write!(
            out,
            r#"<draw:frame draw:z-index="{}" draw:name="{}" svg:width="{}" svg:height="{}" svg:x="{}" svg:y="{}""#,
            self.objects.len(),
            esc_attr(&format!("Chart {}", ch.id)),
            length_in(a.width),
            length_in(a.height),
            length_in(a.dx),
            length_in(a.dy)
        );
        if a.mode == AnchorMode::MoveAndSize {
            let (end, ex, ey) = end_cell(sheet, a.cell, a.dx + a.width, a.dy + a.height);
            let _ = write!(
                out,
                r#" table:end-cell-address="{}.{}" table:end-x="{}" table:end-y="{}""#,
                esc_attr(&odf_sheet(&sheet.name)),
                end.a1(),
                length_in(ex),
                length_in(ey)
            );
        }
        let ranges = chart_ranges(ch);
        let _ = write!(
            out,
            r#"><draw:object draw:notify-on-update-of-ranges="{}" xlink:href="./{dir}" xlink:type="simple" xlink:show="embed" xlink:actuate="onLoad"><loext:p/></draw:object></draw:frame>"#,
            esc_attr(&ranges.join(" "))
        );
        self.objects.push((dir, xml));
    }

    /// Defined names, as named ranges or named expressions.
    fn names(&mut self, out: &mut String) {
        let mut xml = String::new();
        for n in &self.wb.names {
            if n.scope.is_some() || n.name.starts_with("_xlnm.") {
                continue;
            }
            let Ok(expr) = gridcraft_formula::parse(&n.formula) else { continue };
            let Some(text) = of_text(&expr) else { continue };
            let base = self.wb.sheets.first().map(|s| format!("${}.$A$1", odf_sheet(&s.name))).unwrap_or_else(|| "$Sheet1.$A$1".into());
            match &expr {
                Expr::Ref(r) if r.sheet != SheetSel::Current => {
                    let address = text.trim_start_matches('[').trim_end_matches(']');
                    let _ = write!(
                        xml,
                        r#"<table:named-range table:name="{}" table:base-cell-address="{}" table:cell-range-address="{}"/>"#,
                        esc_attr(&n.name),
                        esc_attr(&base),
                        esc_attr(address)
                    );
                }
                _ => {
                    let _ = write!(
                        xml,
                        r#"<table:named-expression table:name="{}" table:base-cell-address="{}" table:expression="of:={}"/>"#,
                        esc_attr(&n.name),
                        esc_attr(&base),
                        esc_attr(&text)
                    );
                }
            }
        }
        if !xml.is_empty() {
            let _ = write!(out, "<table:named-expressions>{xml}</table:named-expressions>");
        }
    }

    fn content(&mut self, body: &str) -> String {
        let fonts = self.font_decls();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NS} office:version="1.3">{fonts}<office:automatic-styles>{}{}{}<style:style style:name="gr1" style:family="graphic"><style:graphic-properties draw:stroke="none" draw:fill="none"/></style:style></office:automatic-styles><office:body><office:spreadsheet>{body}</office:spreadsheet></office:body></office:document-content>"#,
            self.other_xml, self.numbers.xml, self.cell_xml
        )
    }

    fn font_decls(&self) -> String {
        let mut out = String::from("<office:font-face-decls>");
        for f in &self.fonts {
            let quoted = if f.contains(' ') { format!("'{f}'") } else { f.clone() };
            let _ = write!(out, r#"<style:font-face style:name="{}" svg:font-family="{}"/>"#, esc_attr(f), esc_attr(&quoted));
        }
        out.push_str("</office:font-face-decls>");
        out
    }

    fn styles_xml(&mut self) -> String {
        let normal = self.wb.styles.get(StyleId::DEFAULT).clone();
        let mut numbers = NumberStyles::new("D");
        let (props, data) = self.props(&normal, true, &mut numbers);
        let data = data.map(|d| format!(r#" style:data-style-name="{d}""#)).unwrap_or_default();
        let fonts = self.font_decls();
        let size = num(f64::from(normal.font.size));
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NS} office:version="1.3">{fonts}<office:styles><style:default-style style:family="table-cell"><style:text-properties style:font-name="{}" fo:font-size="{size}pt" style:font-size-asian="{size}pt" style:font-size-complex="{size}pt"/></style:default-style>{}<style:style style:name="Default" style:family="table-cell"{data}>{props}</style:style></office:styles><office:automatic-styles><style:page-layout style:name="Mpm1"/></office:automatic-styles><office:master-styles><style:master-page style:name="Default" style:page-layout-name="Mpm1"/></office:master-styles></office:document-styles>"#,
            esc_attr(&normal.font.name),
            numbers.xml
        )
    }

    fn meta(&self) -> String {
        let title = &self.wb.props.title;
        let title = if title.is_empty() { String::new() } else { format!("<dc:title>{}</dc:title>", esc(title)) };
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-meta {NS} office:version="1.3"><office:meta><meta:generator>Lusk Tab</meta:generator>{title}</office:meta></office:document-meta>"#
        )
    }

    /// The view: active sheet, frozen panes, grid lines and zoom, as LibreOffice reads them (its
    /// `ooo:view-settings` name needs the `ooo` namespace declared, or the view is ignored).
    fn settings(&self) -> String {
        let mut sheets = String::new();
        for s in &self.wb.sheets {
            let mut items = String::new();
            let item = |name: &str, kind: &str, value: String| {
                format!(r#"<config:config-item config:name="{name}" config:type="{kind}">{value}</config:config-item>"#)
            };
            items.push_str(&item("CursorPositionX", "int", s.view_active.col.to_string()));
            items.push_str(&item("CursorPositionY", "int", s.view_active.row.to_string()));
            if let Some((rows, cols)) = s.freeze.filter(|(r, c)| *r > 0 || *c > 0) {
                items.push_str(&item("HorizontalSplitMode", "short", if cols > 0 { "2" } else { "0" }.into()));
                items.push_str(&item("VerticalSplitMode", "short", if rows > 0 { "2" } else { "0" }.into()));
                items.push_str(&item("HorizontalSplitPosition", "int", cols.to_string()));
                items.push_str(&item("VerticalSplitPosition", "int", rows.to_string()));
                items.push_str(&item("ActiveSplitRange", "short", if rows > 0 { "2" } else { "3" }.into()));
                items.push_str(&item("PositionLeft", "int", "0".into()));
                items.push_str(&item("PositionRight", "int", cols.to_string()));
                items.push_str(&item("PositionTop", "int", "0".into()));
                items.push_str(&item("PositionBottom", "int", rows.to_string()));
            }
            items.push_str(&item("ShowGrid", "boolean", s.show_gridlines.to_string()));
            items.push_str(&item("ZoomValue", "int", s.zoom.clamp(10, 400).to_string()));
            let _ = write!(sheets, r#"<config:config-item-map-entry config:name="{}">{items}</config:config-item-map-entry>"#, esc_attr(&s.name));
        }
        let active = self.wb.sheets.get(self.wb.active_sheet()).map(|s| s.name.clone()).unwrap_or_default();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-settings {NS} office:version="1.3"><office:settings><config:config-item-set config:name="ooo:view-settings"><config:config-item-map-indexed config:name="Views"><config:config-item-map-entry><config:config-item config:name="ViewId" config:type="string">view1</config:config-item><config:config-item-map-named config:name="Tables">{sheets}</config:config-item-map-named><config:config-item config:name="ActiveTable" config:type="string">{}</config:config-item></config:config-item-map-entry></config:config-item-map-indexed></config:config-item-set></office:settings></office:document-settings>"#,
            esc(&active)
        )
    }
}

/// The text properties of a font that differ from `fb`, the font of the style it inherits from
/// (all of the name and size for the default style).
fn text_props(f: &gridcraft_model::Font, fb: &gridcraft_model::Font, named: bool, wb: &Workbook) -> String {
    let mut text = String::new();
    if named || f.name != fb.name {
        let _ = write!(text, r#" style:font-name="{0}" style:font-name-asian="{0}" style:font-name-complex="{0}""#, esc_attr(&f.name));
    }
    if named || f.size != fb.size {
        let size = num(f64::from(f.size));
        let _ = write!(text, r#" fo:font-size="{size}pt" style:font-size-asian="{size}pt" style:font-size-complex="{size}pt""#);
    }
    if f.bold != fb.bold {
        let weight = if f.bold { "bold" } else { "normal" };
        let _ = write!(text, r#" fo:font-weight="{weight}" style:font-weight-asian="{weight}" style:font-weight-complex="{weight}""#);
    }
    if f.italic != fb.italic {
        let italic = if f.italic { "italic" } else { "normal" };
        let _ = write!(text, r#" fo:font-style="{italic}" style:font-style-asian="{italic}" style:font-style-complex="{italic}""#);
    }
    if f.underline != fb.underline {
        text.push_str(match f.underline {
            Underline::None => r#" style:text-underline-style="none""#,
            Underline::Single | Underline::SingleAccounting => r#" style:text-underline-style="solid" style:text-underline-width="auto" style:text-underline-color="font-color""#,
            Underline::Double | Underline::DoubleAccounting => {
                r#" style:text-underline-style="solid" style:text-underline-type="double" style:text-underline-width="auto" style:text-underline-color="font-color""#
            }
        });
    }
    if f.strike != fb.strike {
        let _ = write!(text, r#" style:text-line-through-style="{}""#, if f.strike { "solid" } else { "none" });
    }
    if f.color != fb.color {
        match hex(f.color, wb) {
            Some(c) => {
                let _ = write!(text, r#" fo:color="{c}""#);
            }
            None => text.push_str(r#" style:use-window-font-color="true""#),
        }
    }
    if f.vert != fb.vert {
        text.push_str(match f.vert {
            VertAlign::Baseline => r#" style:text-position="0% 100%""#,
            VertAlign::Superscript => r#" style:text-position="super 58%""#,
            VertAlign::Subscript => r#" style:text-position="sub 58%""#,
        });
    }
    text
}

/// Empty cells from `from` up to `to` (exclusive).
fn gap(out: &mut String, from: u32, to: u32) {
    match to.saturating_sub(from) {
        0 => {}
        1 => out.push_str("<table:table-cell/>"),
        n => {
            let _ = write!(out, r#"<table:table-cell table:number-columns-repeated="{n}"/>"#);
        }
    }
}

/// A cell's text as paragraphs (one per line), with runs of spaces and tabs as ODF elements.
fn paragraphs(out: &mut String, text: &str) {
    for line in text.split('\n') {
        out.push_str("<text:p>");
        let mut spaces = 0usize;
        let mut first = true;
        let flush = |out: &mut String, spaces: &mut usize, first: bool| {
            if *spaces == 0 {
                return;
            }
            // XML collapses spaces: one may stay a character, others become text:s.
            let (plain, extra) = if first { (0, *spaces) } else { (1, *spaces - 1) };
            if plain == 1 {
                out.push(' ');
            }
            match extra {
                0 => {}
                1 => out.push_str("<text:s/>"),
                n => {
                    let _ = write!(out, r#"<text:s text:c="{n}"/>"#);
                }
            }
            *spaces = 0;
        };
        for ch in line.chars() {
            match ch {
                ' ' => spaces += 1,
                '\t' => {
                    flush(out, &mut spaces, first);
                    out.push_str("<text:tab/>");
                    first = false;
                }
                '\r' => {}
                c => {
                    flush(out, &mut spaces, first);
                    first = false;
                    let mut buf = [0; 4];
                    out.push_str(&esc(c.encode_utf8(&mut buf)));
                }
            }
        }
        // Trailing spaces would collapse away as plain characters.
        if spaces > 0 {
            let _ = write!(out, r#"<text:s text:c="{spaces}"/>"#);
        }
        out.push_str("</text:p>");
    }
}

/// The cell where a chart ends: `w` and `h` pixels from the top-left of `start`.
fn end_cell(sheet: &Sheet, start: CellRef, mut w: f32, mut h: f32) -> (CellRef, f32, f32) {
    let (mut col, mut row) = (start.col, start.row);
    for _ in 0..MAX_COLS {
        let cw = super::column_width(sheet, col);
        if w <= cw || col + 1 >= MAX_COLS {
            break;
        }
        w -= cw;
        col += 1;
    }
    for _ in 0..100_000 {
        let rh = super::row_height(sheet, row);
        if h <= rh || row + 1 >= gridcraft_core::MAX_ROWS {
            break;
        }
        h -= rh;
        row += 1;
    }
    (CellRef::new(row, col), w.max(0.0), h.max(0.0))
}

// ------------------------------------------------------------------ formulas

/// A formula in OpenFormula, as `table:formula` holds it after `of:=`. `None` when it has no
/// OpenFormula spelling (a structured table reference, a spilled range `A1#`).
fn formula(f: &gridcraft_model::Formula) -> Option<String> {
    let parsed;
    let expr = match &f.expr {
        Some(e) => e.as_ref(),
        None => {
            parsed = gridcraft_formula::parse(&f.text).ok()?;
            &parsed
        }
    };
    of_text(expr)
}

pub(super) fn of_text(e: &Expr) -> Option<String> {
    let mut s = String::new();
    of_expr(&mut s, e)?;
    Some(s)
}

/// A sheet name as ODF spells it in references: bare, or quoted with `''` for a quote.
fn odf_sheet(name: &str) -> String {
    if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') && !name.starts_with(|c: char| c.is_ascii_digit()) {
        name.to_string()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

fn anchor_text(a: &gridcraft_formula::Anchor) -> String {
    format!("{}{}{}{}", if a.col_abs { "$" } else { "" }, col_to_letters(a.col), if a.row_abs { "$" } else { "" }, a.row + 1)
}

fn reference(r: &Reference) -> String {
    let (first, last) = match &r.sheet {
        SheetSel::Current => (String::new(), String::new()),
        SheetSel::Named(s) => (format!("${}", odf_sheet(s)), String::new()),
        SheetSel::Span(a, b) => (format!("${}", odf_sheet(a)), format!("${}", odf_sheet(b))),
    };
    let abs = |b: bool| if b { "$" } else { "" };
    match &r.kind {
        RefKind::Cell(a) if last.is_empty() => format!("[{first}.{}]", anchor_text(a)),
        RefKind::Cell(a) => format!("[{first}.{0}:{last}.{0}]", anchor_text(a)),
        RefKind::Range(a, b) => format!("[{first}.{}:{last}.{}]", anchor_text(a), anchor_text(b)),
        RefKind::Rows(r0, a0, r1, a1) => format!("[{first}.{}{}:{last}.{}{}]", abs(*a0), r0 + 1, abs(*a1), r1 + 1),
        RefKind::Cols(c0, a0, c1, a1) => format!("[{first}.{}{}:{last}.{}{}]", abs(*a0), col_to_letters(*c0), abs(*a1), col_to_letters(*c1)),
    }
}

/// Excel function names LibreOffice files under another name (OpenFormula's own, or
/// Microsoft's namespace for functions OpenFormula does not define).
fn odf_function(name: &str) -> String {
    match name {
        "SKEW.P" => return "SKEWP".into(),
        "F.DIST" => return "FDIST".into(),
        "F.INV" => return "FINV".into(),
        "NORMSDIST" | "NORMSINV" | "TDIST" | "CHIDIST" | "CHIINV" | "CHITEST" | "FDIST" | "FINV" => return format!("LEGACY.{name}"),
        _ => {}
    }
    const MICROSOFT: &[&str] = &[
        "CEILING",
        "CEILING.MATH",
        "CEILING.PRECISE",
        "ISO.CEILING",
        "FLOOR",
        "FLOOR.MATH",
        "FLOOR.PRECISE",
        "MAXIFS",
        "MINIFS",
        "MODE.SNGL",
        "MODE.MULT",
        "STDEV.S",
        "STDEV.P",
        "VAR.S",
        "VAR.P",
        "RANK.EQ",
        "RANK.AVG",
        "PERCENTILE.INC",
        "PERCENTILE.EXC",
        "QUARTILE.INC",
        "QUARTILE.EXC",
        "PERCENTRANK.INC",
        "PERCENTRANK.EXC",
        "FORECAST.LINEAR",
        "COVARIANCE.P",
        "COVARIANCE.S",
        "NORM.DIST",
        "NORM.INV",
        "NORM.S.DIST",
        "NORM.S.INV",
        "T.DIST",
        "T.DIST.2T",
        "T.DIST.RT",
        "T.INV",
        "T.INV.2T",
        "T.TEST",
        "CHISQ.DIST",
        "CHISQ.DIST.RT",
        "CHISQ.INV",
        "CHISQ.INV.RT",
        "CHISQ.TEST",
        "F.DIST.RT",
        "F.INV.RT",
        "F.TEST",
        "BINOM.DIST",
        "BINOM.DIST.RANGE",
        "BINOM.INV",
        "POISSON.DIST",
        "EXPON.DIST",
        "GAMMA.DIST",
        "GAMMA.INV",
        "GAMMALN.PRECISE",
        "BETA.DIST",
        "BETA.INV",
        "LOGNORM.DIST",
        "LOGNORM.INV",
        "WEIBULL.DIST",
        "HYPGEOM.DIST",
        "NEGBINOM.DIST",
        "CONFIDENCE.NORM",
        "CONFIDENCE.T",
        "Z.TEST",
        "FORECAST.ETS",
        "FORECAST.ETS.CONFINT",
        "FORECAST.ETS.SEASONALITY",
        "FORECAST.ETS.STAT",
        "CONCAT",
        "TEXTJOIN",
        "TEXTBEFORE",
        "TEXTAFTER",
        "TEXTSPLIT",
        "VALUETOTEXT",
        "ARRAYTOTEXT",
        "REGEXTEST",
        "REGEXEXTRACT",
        "REGEXREPLACE",
        "NETWORKDAYS.INTL",
        "WORKDAY.INTL",
        "XLOOKUP",
        "XMATCH",
        "FILTER",
        "SORT",
        "SORTBY",
        "UNIQUE",
        "SEQUENCE",
        "RANDARRAY",
        "VSTACK",
        "HSTACK",
        "TOROW",
        "TOCOL",
        "WRAPROWS",
        "WRAPCOLS",
        "TAKE",
        "DROP",
        "CHOOSEROWS",
        "CHOOSECOLS",
        "EXPAND",
        "GROUPBY",
        "PIVOTBY",
        "IFS",
        "SWITCH",
        "ENCODEURL",
        "WEBSERVICE",
        "FILTERXML",
        "ERF.PRECISE",
        "ERFC.PRECISE",
        "LET",
        "LAMBDA",
        "PERCENTOF",
    ];
    if MICROSOFT.contains(&name) { format!("COM.MICROSOFT.{name}") } else { name.to_string() }
}

/// The inverse of `odf_function`, for names read from an .ods file.
pub(crate) fn excel_function(name: &str) -> String {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "SKEWP" => "SKEW.P".into(),
        "FDIST" => "F.DIST".into(),
        "FINV" => "F.INV".into(),
        _ => upper.strip_prefix("COM.MICROSOFT.").or_else(|| upper.strip_prefix("LEGACY.")).map_or_else(|| name.to_string(), str::to_string),
    }
}

fn of_expr(s: &mut String, e: &Expr) -> Option<()> {
    match e {
        Expr::Number(_) | Expr::Text(_) | Expr::Error(_) => s.push_str(&gridcraft_formula::print(e)),
        Expr::Bool(b) => s.push_str(if *b { "TRUE()" } else { "FALSE()" }),
        Expr::Array(rows) => {
            s.push('{');
            for (ri, row) in rows.iter().enumerate() {
                if ri > 0 {
                    s.push('|');
                }
                for (ci, el) in row.iter().enumerate() {
                    if ci > 0 {
                        s.push(';');
                    }
                    of_expr(s, el)?;
                }
            }
            s.push('}');
        }
        Expr::Ref(r) => s.push_str(&reference(r)),
        Expr::Name(n) if !n.contains('!') => s.push_str(n),
        Expr::Name(_) | Expr::Struct(_) | Expr::Invoke(..) => return None,
        Expr::Unary(op, x) => match op {
            UnOp::Neg => {
                s.push('-');
                of_expr(s, x)?;
            }
            UnOp::Plus => {
                s.push('+');
                of_expr(s, x)?;
            }
            UnOp::Percent => {
                of_expr(s, x)?;
                s.push('%');
            }
            // Implicit intersection is how LibreOffice evaluates a plain reference anyway.
            UnOp::At => of_expr(s, x)?,
            UnOp::Spill | UnOp::Trim(_) => return None,
        },
        Expr::Binary(..) => {
            let mut rest: Vec<(&BinOp, &Expr)> = Vec::new();
            let mut first = e;
            while let Expr::Binary(op, a, b) = first {
                rest.push((op, b));
                first = a;
            }
            of_expr(s, first)?;
            for (op, right) in rest.into_iter().rev() {
                s.push_str(match op {
                    BinOp::Union => "~",
                    BinOp::Intersect => "!",
                    other => other.symbol(),
                });
                of_expr(s, right)?;
            }
        }
        Expr::Call(name, args) => {
            s.push_str(&odf_function(name));
            s.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    s.push(';');
                }
                of_expr(s, a)?;
            }
            s.push(')');
        }
        Expr::Missing => {}
        Expr::Paren(x) => {
            s.push('(');
            of_expr(s, x)?;
            s.push(')');
        }
    }
    Some(())
}

// ------------------------------------------------------------------ charts

/// `'Budget 2026'!$B$2:$B$5` → `'Budget 2026'.$B$2:'Budget 2026'.$B$5`, the spelling of chart
/// ranges; several in parentheses become several ranges.
fn chart_range(r: &str) -> Option<String> {
    let inner = r.trim().trim_start_matches('(').trim_end_matches(')');
    let mut parts = Vec::new();
    for part in split_commas(inner) {
        let (sheet, cells) = split_sheet(part.trim())?;
        let sheet = odf_sheet(&sheet);
        let cells: Vec<String> = cells.split(':').map(|c| format!("{sheet}.{c}")).collect();
        parts.push(cells.join(":"));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn split_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut quoted, mut start) = (false, 0);
    for (i, c) in s.char_indices() {
        if c == '\'' {
            quoted = !quoted;
        } else if c == ',' && !quoted {
            parts.push(&s[start..i]);
            start = i + 1;
        }
    }
    parts.push(&s[start..]);
    parts
}

/// `'My sheet'!$A$1` → (`My sheet`, `$A$1`).
fn split_sheet(r: &str) -> Option<(String, String)> {
    let (sheet, cells) = r.rsplit_once('!')?;
    let sheet = match sheet.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        Some(q) => q.replace("''", "'"),
        None => sheet.to_string(),
    };
    Some((sheet, cells.to_string()))
}

fn chart_ranges(ch: &Chart) -> Vec<String> {
    let mut out = Vec::new();
    for s in &ch.series {
        for r in [s.categories.as_deref(), s.name.as_deref(), Some(s.values.as_str()), s.bubble_sizes.as_deref()].into_iter().flatten() {
            if let Some(r) = chart_range(r)
                && !out.contains(&r)
            {
                out.push(r);
            }
        }
    }
    out
}

/// The cells in a range (`Sheet!$A$2:$A$5` → 4).
fn range_len(r: &str) -> usize {
    let cells = r.rsplit_once('!').map_or(r, |(_, c)| c);
    let point = |c: &str| {
        let c = c.replace('$', "");
        let split = c.find(|ch: char| ch.is_ascii_digit()).unwrap_or(c.len());
        let col = c[..split].bytes().fold(0usize, |n, b| n * 26 + usize::from(b.to_ascii_uppercase().saturating_sub(b'A') + 1));
        (col, c[split..].parse::<usize>().unwrap_or(1))
    };
    match cells.split_once(':') {
        Some((a, b)) => {
            let ((c0, r0), (c1, r1)) = (point(a), point(b));
            (c0.abs_diff(c1) + 1) * (r0.abs_diff(r1) + 1)
        }
        None => 1,
    }
}

fn odf_class(kind: ChartKind) -> &'static str {
    match kind {
        ChartKind::Line | ChartKind::LineMarkers | ChartKind::LineStacked => "chart:line",
        ChartKind::Pie => "chart:circle",
        ChartKind::Doughnut => "chart:ring",
        ChartKind::Area | ChartKind::AreaStacked | ChartKind::AreaStacked100 => "chart:area",
        ChartKind::Scatter | ChartKind::ScatterLines => "chart:scatter",
        ChartKind::Bubble => "chart:bubble",
        ChartKind::Radar => "chart:radar",
        ChartKind::Stock => "chart:stock",
        _ => "chart:bar",
    }
}

/// A chart as an `office:chart` document; `None` when none of its series has a range.
fn chart_xml(ch: &Chart, wb: &Workbook) -> Option<String> {
    let main = if ch.kind == ChartKind::Combo { ch.series.first().and_then(|s| s.kind).unwrap_or(ChartKind::ColumnClustered) } else { ch.kind };
    let mut styles = String::new();
    let mut style_n = 0;
    let mut style = |props: String, styles: &mut String| {
        style_n += 1;
        let _ = write!(styles, r#"<style:style style:name="ch{style_n}" style:family="chart">{props}</style:style>"#);
        format!("ch{style_n}")
    };
    let vertical = matches!(main, ChartKind::BarClustered | ChartKind::BarStacked | ChartKind::BarStacked100);
    let stacked = matches!(main, ChartKind::ColumnStacked | ChartKind::BarStacked | ChartKind::LineStacked | ChartKind::AreaStacked);
    let percent = matches!(main, ChartKind::ColumnStacked100 | ChartKind::BarStacked100 | ChartKind::AreaStacked100);
    let symbol = match main {
        ChartKind::LineMarkers | ChartKind::Scatter | ChartKind::ScatterLines | ChartKind::Bubble => "automatic",
        _ => "none",
    };
    let angle = (90 - i32::from(ch.first_slice_angle)).rem_euclid(360);
    let plot_style = style(
        format!(
            r#"<style:chart-properties chart:vertical="{vertical}" chart:stacked="{stacked}" chart:percentage="{percent}" chart:symbol-type="{symbol}" chart:angle-offset="{angle}" chart:include-hidden-cells="false" chart:treat-empty-cells="leave-gap"{}/>"#,
            if ch.series.iter().any(|s| s.smooth) { r#" chart:interpolation="cubic-spline""# } else { "" }
        ),
        &mut styles,
    );
    let mut series_xml = String::new();
    let mut any = false;
    // Series without a colour of their own take the theme's accents, as the sheet shows them.
    let accent = |i: usize| {
        let v = wb.theme.colors[4 + i % 6];
        format!("#{v:06x}")
    };
    let pie = matches!(main, ChartKind::Pie | ChartKind::Doughnut);
    for (index, s) in ch.series.iter().enumerate() {
        let Some(values) = chart_range(&s.values) else { continue };
        any = true;
        let kind = s.kind.unwrap_or(main);
        let line = matches!(kind, ChartKind::Line | ChartKind::LineMarkers | ChartKind::LineStacked | ChartKind::ScatterLines | ChartKind::Radar);
        let mut graphic = String::new();
        let c = s.color.and_then(|c| hex(c, wb)).unwrap_or_else(|| accent(index));
        if pie {
            graphic.push_str(r##" draw:stroke="solid" svg:stroke-color="#ffffff""##);
        } else if line {
            let _ = write!(graphic, r#" svg:stroke-color="{c}" svg:stroke-width="0.07cm""#);
        } else {
            let _ = write!(graphic, r#" svg:stroke-color="{c}" draw:fill-color="{c}""#);
        }
        if kind == ChartKind::Scatter {
            graphic.push_str(r#" draw:stroke="none""#);
        }
        let labels = if ch.data_labels { r#" chart:data-label-number="value""# } else { "" };
        let series_symbol = match kind {
            ChartKind::LineMarkers | ChartKind::Scatter | ChartKind::ScatterLines | ChartKind::Bubble => "automatic",
            _ => "none",
        };
        let smooth = if s.smooth { r#" chart:interpolation="cubic-spline""# } else { "" };
        let st = style(
            format!(r#"<style:chart-properties chart:symbol-type="{series_symbol}"{labels}{smooth}/><style:graphic-properties{graphic}/>"#),
            &mut styles,
        );
        let (values, domains) = match kind {
            // Bubbles: the sizes are the values, then the Y values and the X values as domains.
            ChartKind::Bubble => (
                s.bubble_sizes.as_deref().and_then(chart_range).unwrap_or_else(|| values.clone()),
                [Some(values.clone()), s.categories.as_deref().and_then(chart_range)].into_iter().flatten().collect::<Vec<_>>(),
            ),
            ChartKind::Scatter | ChartKind::ScatterLines => (values, s.categories.as_deref().and_then(chart_range).into_iter().collect()),
            _ => (values, vec![]),
        };
        let _ = write!(series_xml, r#"<chart:series chart:style-name="{st}" chart:values-cell-range-address="{}""#, esc_attr(&values));
        if let Some(name) = s.name.as_deref().and_then(chart_range) {
            let _ = write!(series_xml, r#" chart:label-cell-address="{}""#, esc_attr(&name));
        }
        let _ = write!(series_xml, r#" chart:class="{}""#, odf_class(kind));
        if s.secondary {
            series_xml.push_str(r#" chart:attached-axis="secondary-y""#);
        }
        series_xml.push('>');
        for d in domains {
            let _ = write!(series_xml, r#"<chart:domain table:cell-range-address="{}"/>"#, esc_attr(&d));
        }
        // A pie's slices each take the next accent colour.
        if pie {
            for point in 0..range_len(&s.values).min(255) {
                let st = style(
                    format!(r##"<style:graphic-properties draw:fill-color="{}" draw:stroke="solid" svg:stroke-color="#ffffff"/>"##, accent(point)),
                    &mut styles,
                );
                let _ = write!(series_xml, r#"<chart:data-point chart:style-name="{st}"/>"#);
            }
        }
        series_xml.push_str("</chart:series>");
    }
    if !any {
        return None;
    }
    let axis_style = style(
        r##"<style:chart-properties chart:display-label="true" chart:tick-marks-major-inner="false" chart:tick-marks-major-outer="true" chart:logarithmic="false" chart:reverse-direction="false" text:line-break="false"/><style:graphic-properties svg:stroke-color="#b3b3b3"/><style:text-properties fo:font-size="9pt"/>"##.into(),
        &mut styles,
    );
    let grid_style = style(r##"<style:graphic-properties svg:stroke-color="#d9d9d9"/>"##.into(), &mut styles);
    let wall_style = style(r#"<style:graphic-properties draw:stroke="none" draw:fill="none"/>"#.into(), &mut styles);
    let chart_style = style(
        r##"<style:graphic-properties draw:stroke="solid" svg:stroke-color="#d9d9d9" draw:fill="solid" draw:fill-color="#ffffff"/>"##.into(),
        &mut styles,
    );
    let title_style = style(r#"<style:text-properties fo:font-size="13pt"/>"#.into(), &mut styles);
    let mut axes = String::new();
    if pie {
        // A pie keeps its categories on a hidden axis, and runs clockwise as in Excel.
        let hidden = style(r#"<style:chart-properties chart:display-label="false" chart:visible="false"/>"#.into(), &mut styles);
        let clockwise = style(
            r#"<style:chart-properties chart:display-label="false" chart:visible="false" chart:reverse-direction="true"/>"#.into(),
            &mut styles,
        );
        let _ = write!(axes, r#"<chart:axis chart:dimension="x" chart:name="primary-x" chart:style-name="{hidden}">"#);
        if let Some(c) = ch.series.iter().find_map(|s| s.categories.as_deref()).and_then(chart_range) {
            let _ = write!(axes, r#"<chart:categories table:cell-range-address="{}"/>"#, esc_attr(&c));
        }
        let _ = write!(axes, r#"</chart:axis><chart:axis chart:dimension="y" chart:name="primary-y" chart:style-name="{clockwise}"/>"#);
    } else {
        let categories = ch
            .series
            .iter()
            .find_map(|s| s.categories.as_deref())
            .and_then(chart_range)
            .filter(|_| !matches!(main, ChartKind::Scatter | ChartKind::ScatterLines | ChartKind::Bubble));
        let title = |t: &Option<String>| t.as_ref().map(|t| format!("<chart:title><text:p>{}</text:p></chart:title>", esc(t))).unwrap_or_default();
        let _ = write!(axes, r#"<chart:axis chart:dimension="x" chart:name="primary-x" chart:style-name="{axis_style}">{}"#, title(&ch.x_title));
        if let Some(c) = categories {
            let _ = write!(axes, r#"<chart:categories table:cell-range-address="{}"/>"#, esc_attr(&c));
        }
        axes.push_str("</chart:axis>");
        let _ = write!(axes, r#"<chart:axis chart:dimension="y" chart:name="primary-y" chart:style-name="{axis_style}">{}"#, title(&ch.y_title));
        if ch.gridlines {
            let _ = write!(axes, r#"<chart:grid chart:style-name="{grid_style}" chart:class="major"/>"#);
        }
        axes.push_str("</chart:axis>");
        if ch.series.iter().any(|s| s.secondary) {
            let _ = write!(axes, r#"<chart:axis chart:dimension="y" chart:name="secondary-y" chart:style-name="{axis_style}"/>"#);
        }
    }
    let title = ch
        .title
        .as_ref()
        .map(|t| format!(r#"<chart:title chart:style-name="{title_style}"><text:p>{}</text:p></chart:title>"#, esc(t)))
        .unwrap_or_default();
    let legend = match ch.legend {
        LegendPos::None => String::new(),
        LegendPos::Bottom => r#"<chart:legend chart:legend-position="bottom" style:legend-expansion="wide"/>"#.into(),
        LegendPos::Top => r#"<chart:legend chart:legend-position="top" style:legend-expansion="wide"/>"#.into(),
        LegendPos::Left => r#"<chart:legend chart:legend-position="start"/>"#.into(),
        LegendPos::Right => r#"<chart:legend chart:legend-position="end"/>"#.into(),
    };
    let source = ch.source.as_deref().and_then(chart_range).map(|s| format!(r#" table:cell-range-address="{}""#, esc_attr(&s))).unwrap_or_default();
    Some(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NS} office:version="1.3"><office:automatic-styles>{styles}</office:automatic-styles><office:body><office:chart><chart:chart svg:width="{}" svg:height="{}" xlink:href=".." xlink:type="simple" chart:class="{}" chart:style-name="{chart_style}">{title}{legend}<chart:plot-area chart:style-name="{plot_style}"{source} chart:data-source-has-labels="none">{axes}{series_xml}<chart:wall chart:style-name="{wall_style}"/><chart:floor chart:style-name="{wall_style}"/></chart:plot-area></chart:chart></office:chart></office:body></office:document-content>"#,
        length_in(ch.anchor.width),
        length_in(ch.anchor.height),
        odf_class(main)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(text: &str) -> String {
        of_text(&gridcraft_formula::parse(text).unwrap()).unwrap()
    }

    #[test]
    fn formulas_are_written_in_openformula() {
        assert_eq!(of("SUM(A1:B3)"), "SUM([.A1:.B3])");
        assert_eq!(of("'Mon budget'!$A$1*2"), "[$'Mon budget'.$A$1]*2");
        assert_eq!(of("Feuil1!A1:B2"), "[$Feuil1.A1:.B2]");
        assert_eq!(of("IF(A1>0,\"oui\",\"non\")"), "IF([.A1]>0;\"oui\";\"non\")");
        assert_eq!(of("TRUE"), "TRUE()");
        assert_eq!(of("STDEV.S(A:A)"), "COM.MICROSOFT.STDEV.S([.A:.A])");
        assert_eq!(of("SUM({1,2;3,4})"), "SUM({1;2|3;4})");
        assert_eq!(of("-(A1+B1)%"), "-([.A1]+[.B1])%");
        // And back: what the .ods reader turns them into.
        for text in ["SUM(A1:B3)", "'Mon budget'!$A$1*2", "IF(A1>0,\"oui\",\"non\")", "STDEV.S(A1:A9)", "F.DIST(1,2,3,TRUE)", "FDIST(1,2,3)"] {
            let back = crate::ods::odf_formula(&format!("of:={}", of(text))).unwrap();
            assert_eq!(
                gridcraft_formula::print(&gridcraft_formula::parse(&back).unwrap()),
                gridcraft_formula::print(&gridcraft_formula::parse(text).unwrap()),
                "{text}"
            );
        }
    }

    #[test]
    fn text_keeps_spaces_tabs_and_lines() {
        let mut out = String::new();
        paragraphs(&mut out, "  a  b\tc \nd");
        assert_eq!(out, r#"<text:p><text:s text:c="2"/>a <text:s/>b<text:tab/>c<text:s text:c="1"/></text:p><text:p>d</text:p>"#);
    }

    #[test]
    fn chart_ranges_use_odf_spelling() {
        assert_eq!(chart_range("'Budget 2026'!$B$2:$B$5").as_deref(), Some("'Budget 2026'.$B$2:'Budget 2026'.$B$5"));
        assert_eq!(chart_range("Data!$A$1").as_deref(), Some("Data.$A$1"));
        assert_eq!(chart_range("(Data!$A$1:$A$2,Data!$C$1:$C$2)").as_deref(), Some("Data.$A$1:Data.$A$2 Data.$C$1:Data.$C$2"));
    }
}
