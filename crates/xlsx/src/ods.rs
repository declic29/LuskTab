//! OpenDocument Spreadsheet import: values, merges and (Lusk Tab) formulas, converted from
//! OpenFormula (`odf_formula`) with their saved results as values, cell formatting, number
//! formats, column widths, row heights and charts (`style`, `chart`), and export (`write`).
//! All fixtures are authored in Rust.
//! Format reference: ODF 1.3, part 3, sections 6.1 and 9.1:
//! <https://docs.oasis-open.org/office/OpenDocument/v1.3/os/part3-schema/OpenDocument-v1.3-os-part3-schema.html>.

mod chart;
mod numfmt;
mod style;
mod write;

pub use write::write_ods;

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::Arc;

use gridcraft_core::date::{days_in_month, serial_from_ymd};
use gridcraft_core::{CellError, CellRef, DateSystem, MAX_COLS, MAX_ROWS, RangeRef, Value};
use gridcraft_model::{AnchorMode, Cell, Formula, LineInfo, NumFmt, Sheet, Style, StyleId, StyleTable, Visibility, Workbook};
use quick_xml::NsReader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;

use crate::package::{MAX_ENTRIES, Package};
use crate::{IoError, ReadReport};

const MIME: &[u8] = b"application/vnd.oasis.opendocument.spreadsheet";
const OFFICE: &[u8] = b"urn:oasis:names:tc:opendocument:xmlns:office:1.0";
const TABLE: &[u8] = b"urn:oasis:names:tc:opendocument:xmlns:table:1.0";
const TEXT: &[u8] = b"urn:oasis:names:tc:opendocument:xmlns:text:1.0";
const CALC_EXT: &[u8] = b"urn:org:documentfoundation:names:experimental:calc:xmlns:calcext:1.0";
const MANIFEST: &[u8] = b"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0";
const DRAW: &[u8] = b"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0";
const SVG: &[u8] = b"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0";
const XLINK: &[u8] = b"http://www.w3.org/1999/xlink";
const DC: &[u8] = b"http://purl.org/dc/elements/1.1/";
/// Blank cells kept only for their formatting (a coloured, empty header cell), at most.
const MAX_STYLED_BLANKS: u64 = 250_000;
/// Columns a blank, formatted run keeps (LibreOffice pads rows to its last column).
const MAX_STYLED_RUN: u32 = 1024;
/// Rows given their own height or hidden state, at most.
const MAX_ROW_INFOS: usize = 200_000;
/// Charts read from one file, at most, and the size of one chart's parts.
const MAX_CHARTS: usize = 256;
const MAX_CHART_PART: u64 = 8 * 1024 * 1024;
const MAX_CELLS: u64 = 1_000_000;
const MAX_MERGES: u64 = 100_000;
const MAX_CELL_TEXT: usize = 1024 * 1024;
const MAX_TEXT: u64 = 64 * 1024 * 1024;
const MAX_SHEETS: usize = 1024;

/// Reads only the small, exact mimetype entry, including when the ZIP uses compression.
/// Looking at a filename or at arbitrary bytes inside the archive is not sufficient.
pub(crate) fn has_mimetype(bytes: &[u8]) -> bool {
    let Ok(mut zip) = zip::ZipArchive::new(Cursor::new(bytes)) else { return false };
    if zip.len() > MAX_ENTRIES {
        return false;
    }
    let Ok(mut file) = zip.by_name("mimetype") else { return false };
    if file.size() != MIME.len() as u64 {
        return false;
    }
    let mut value = Vec::new();
    (&mut file).take(MIME.len() as u64 + 1).read_to_end(&mut value).is_ok() && value == MIME
}

/// Imports ODS sheets, typed values, displayed text, merges and formulas. A formula that can't
/// be converted keeps only its saved result, as a constant (and a warning says so). Always
/// inspect the returned warnings. Saving to ODS is not supported.
pub fn read_ods(bytes: &[u8]) -> Result<(Workbook, ReadReport), IoError> {
    let mut package = Package::open(bytes)?;
    if !has_mimetype(bytes) {
        return Err(IoError::Format("missing or invalid ODS mimetype".into()));
    }
    if let Some(manifest) = package.read("META-INF/manifest.xml")? {
        check_manifest(&manifest)?;
    }
    let content = package.read("content.xml")?.ok_or_else(|| IoError::Format("ODS content.xml is missing".into()))?;
    // Lusk Tab : the formatting, from styles.xml and the automatic styles before the body.
    let mut styles = style::OdsStyles::default();
    if let Some(root) = package.read_optional_limited("styles.xml", MAX_CHART_PART * 4).ok().flatten().and_then(|b| crate::xml::parse(&b).ok()) {
        styles.add(&root, false);
    }
    if let Some(head) = style::content_head(&content) {
        styles.add(&head, true);
    }
    let mut import = Import::new(styles);
    import.parse(&content)?;
    if import.wb.sheets.is_empty() {
        return Err(IoError::Format("ODS contains no worksheets".into()));
    }
    if import.wb.sheets.iter().all(|s| s.visibility != Visibility::Visible)
        && let Some(first) = import.wb.sheets.first_mut()
    {
        Arc::make_mut(first).visibility = Visibility::Visible;
    }
    import.read_charts(&mut package)?;
    if let Some(settings) = package.read_optional_limited("settings.xml", MAX_CHART_PART).ok().flatten().and_then(|b| crate::xml::parse(&b).ok()) {
        apply_settings(&mut import.wb, &settings);
    }
    if import.partial {
        import.report.warn(ODS_PARTIAL_NOTICE);
    }
    Ok((import.wb, import.report))
}

/// What the open notice says about an .ods file (translated by the interface's catalogs).
pub(crate) const ODS_NOTICE: &str = "LibreOffice Calc file (.ods): values, formulas, formatting, merged cells, comments and charts are kept.";
/// Said when the file has what Lusk Tab does not keep; Save then asks for a new name.
pub const ODS_PARTIAL_NOTICE: &str = "This LibreOffice file also has images, drawings, conditional formatting or data validation, which Lusk Tab does not keep. Save it under a new name to keep the original file whole.";
/// Said once when a chart could not be read.
pub(crate) const ODS_CHART_NOTICE: &str = "Some charts of this LibreOffice file could not be read and were left out.";
/// Said once when a formula could not be converted and kept only its saved result.
pub(crate) const ODS_FORMULA_NOTICE: &str =
    "Some LibreOffice formulas could not be converted; they keep their last saved result and will not recalculate.";

/// Most characters of a formula converted (Excel's own limit is 8 192).
const MAX_FORMULA: usize = 16_384;

/// An OpenFormula formula (ODF 1.3 part 4, as `table:formula` holds it: `of:=SUM([.A1:.B3])`,
/// `of:=[$Feuille1.A1]*2`) in the spelling Lusk Tab reads from .xlsx files (`SUM(A1:B3)`,
/// `Feuille1!A1*2`): references lose their brackets and dots, a sheet becomes `Sheet!`, and the
/// `;` between arguments becomes `,`. `None` for what has no equivalent (a reference spanning
/// two sheets, another formula language, an unclosed bracket).
pub(crate) fn odf_formula(text: &str) -> Option<String> {
    if text.len() > MAX_FORMULA {
        return None;
    }
    let body = match text.split_once(':') {
        Some(("of" | "oooc", rest)) => rest,
        // Already in Excel's spelling.
        Some(("msoxl", rest)) => return Some(rest.strip_prefix('=').unwrap_or(rest).to_string()),
        Some((ns, _)) if !ns.is_empty() && ns.chars().all(|c| c.is_ascii_alphanumeric()) => return None,
        _ => text,
    };
    let body = body.strip_prefix('=').unwrap_or(body);
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                out.push('"');
                // A string, with "" for a quote inside.
                loop {
                    match chars.next()? {
                        '"' if chars.peek() == Some(&'"') => {
                            chars.next();
                            out.push_str("\"\"");
                        }
                        '"' => break,
                        ch => out.push(ch),
                    }
                }
                out.push('"');
            }
            '[' => {
                let mut reference = String::new();
                let mut quoted = false;
                loop {
                    match chars.next()? {
                        '\'' => {
                            quoted = !quoted;
                            reference.push('\'');
                        }
                        ']' if !quoted => break,
                        ch => reference.push(ch),
                    }
                }
                out.push_str(&odf_reference(&reference)?);
            }
            ';' => out.push(','),
            // Array rows (`{1;2|3;4}`), and the intersection operator.
            '|' => out.push(';'),
            '!' => out.push(' '),
            _ => out.push(c),
        }
    }
    // Functions LibreOffice files under another name (`COM.MICROSOFT.CONCAT`, `LEGACY.TDIST`).
    Some(excel_function_names(&out))
}

/// Function names (an identifier before `(`, outside strings) in Excel's spelling.
fn excel_function_names(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if !quoted && (c.is_alphanumeric() || c == '_' || c == '.') {
            word.push(c);
            continue;
        }
        // OpenFormula's `TRUE()` is Excel's `TRUE`.
        if c == '(' && !quoted && chars.peek() == Some(&')') && (word.eq_ignore_ascii_case("TRUE") || word.eq_ignore_ascii_case("FALSE")) {
            chars.next();
            out.push_str(&word.to_ascii_uppercase());
            word.clear();
            continue;
        }
        if c == '(' && !quoted && !word.is_empty() {
            out.push_str(&write::excel_function(&word));
        } else {
            out.push_str(&word);
        }
        word.clear();
        if c == '"' {
            quoted = !quoted;
        }
        out.push(c);
    }
    out.push_str(&word);
    out
}

/// `.A1`, `$Feuille1.$A$1`, `'Mon budget'.A1:.B2` → `A1`, `Feuille1!$A$1`, `'Mon budget'!A1:B2`.
fn odf_reference(r: &str) -> Option<String> {
    let parts = split_outside_quotes(r, ':');
    if parts.len() > 2 {
        return None;
    }
    let mut sheet: Option<String> = None;
    let mut cells = Vec::with_capacity(2);
    for (i, part) in parts.iter().enumerate() {
        let (s, cell) = match rsplit_outside_quotes(part, '.') {
            Some((s, cell)) => (s.trim_start_matches('$'), cell),
            None => ("", *part),
        };
        if cell.is_empty() || cell.contains(['.', '\'']) {
            return None;
        }
        if cell.starts_with('#') {
            // `[.#REF!]`: a reference LibreOffice already lost.
            return Some("#REF!".into());
        }
        if !s.is_empty() {
            match (&sheet, i) {
                (None, 0) => sheet = Some(s.to_string()),
                (Some(first), _) if first == s => {}
                // A range over several sheets (3-D) has no single-sheet spelling.
                _ => return None,
            }
        }
        cells.push(cell);
    }
    let cells = cells.join(":");
    Some(match sheet {
        None => cells,
        Some(s) if s.starts_with('\'') => format!("{s}!{cells}"),
        Some(s) if s.chars().all(|c| c.is_alphanumeric() || c == '_') && !s.starts_with(|c: char| c.is_ascii_digit()) => format!("{s}!{cells}"),
        Some(s) => format!("'{}'!{cells}", s.replace('\'', "''")),
    })
}

fn split_outside_quotes(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut quoted, mut start) = (false, 0);
    for (i, c) in s.char_indices() {
        if c == '\'' {
            quoted = !quoted;
        } else if c == sep && !quoted {
            parts.push(&s[start..i]);
            start = i + c.len_utf8();
        }
    }
    parts.push(&s[start..]);
    parts
}

fn rsplit_outside_quotes(s: &str, sep: char) -> Option<(&str, &str)> {
    let mut quoted = false;
    let mut at = None;
    for (i, c) in s.char_indices() {
        if c == '\'' {
            quoted = !quoted;
        } else if c == sep && !quoted {
            at = Some(i);
        }
    }
    at.map(|i| (&s[..i], &s[i + sep.len_utf8()..]))
}

fn is_ns(ns: ResolveResult<'_>, expected: &[u8]) -> bool {
    matches!(ns, ResolveResult::Bound(n) if n.as_ref() == expected)
}

fn attr(reader: &mut NsReader<&[u8]>, e: &BytesStart<'_>, ns: &[u8], name: &[u8]) -> Result<Option<String>, IoError> {
    let mut found = None;
    for a in e.attributes() {
        let a = a.map_err(|e| IoError::Xml(e.to_string()))?;
        let (resolved, local) = reader.resolver_mut().resolve_attribute(a.key);
        if is_ns(resolved, ns) && local.as_ref() == name {
            let value = a.normalized_value(XmlVersion::Explicit1_0).map_err(|e| IoError::Xml(e.to_string()))?;
            if value.len() > MAX_CELL_TEXT {
                return Err(IoError::TooLarge("ODS attribute exceeds 1 MiB".into()));
            }
            found = Some(value.into_owned());
        }
    }
    Ok(found)
}

fn positive(value: Option<String>, default: u32, limit: u32) -> Result<u32, IoError> {
    let n = match value {
        Some(s) => s.parse::<u32>().map_err(|_| IoError::Format("invalid ODS repeat or span count".into()))?,
        None => default,
    };
    if n == 0 || n > limit {
        return Err(IoError::TooLarge("ODS repeat or span exceeds worksheet bounds".into()));
    }
    Ok(n)
}

fn xml_error(e: impl std::fmt::Display) -> IoError {
    IoError::Xml(format!("ODS: {e}"))
}

fn check_manifest(bytes: &[u8]) -> Result<(), IoError> {
    let mut reader = NsReader::from_reader(bytes);
    let mut depth = 0usize;
    let mut root_seen = false;
    loop {
        let event = reader.read_event().map_err(xml_error)?;
        match &event {
            Event::Start(e) | Event::Empty(e) => {
                let (ns, name) = reader.resolver_mut().resolve_element(e.name());
                if is_ns(ns.clone(), MANIFEST) && name.as_ref() == b"encryption-data" {
                    return Err(IoError::Format("encrypted ODS files are not supported".into()));
                }
                if depth == 0 {
                    if root_seen || !is_ns(ns, MANIFEST) || name.as_ref() != b"manifest" {
                        return Err(xml_error("expected one manifest:manifest root"));
                    }
                    root_seen = true;
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                    if depth > crate::xml::MAX_DEPTH {
                        return Err(xml_error("manifest nested too deeply"));
                    }
                }
            }
            Event::End(_) => depth = depth.checked_sub(1).ok_or_else(|| xml_error("unexpected manifest closing element"))?,
            Event::DocType(_) => return Err(xml_error("DOCTYPE is not supported")),
            Event::Eof => break,
            _ => {}
        }
    }
    if !root_seen || depth != 0 {
        return Err(xml_error("incomplete manifest"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Document,
    Body,
    Spreadsheet,
    Table,
    Rows,
    Row,
    Columns,
    Column,
    Shapes,
    Frame,
    Object,
    Image,
    Drawing,
    Annotation,
    Creator,
    NamedRange,
    NamedExpression,
    /// Conditional formatting or data validation, which are not kept.
    Unkept,
    Cell,
    Covered,
    Paragraph,
    Inline,
    Space,
    Tab,
    Break,
    Other,
}

fn tag(reader: &mut NsReader<&[u8]>, e: &BytesStart<'_>) -> Tag {
    let (ns, name) = reader.resolver_mut().resolve_element(e.name());
    let local = name.as_ref();
    if is_ns(ns.clone(), OFFICE) {
        return match local {
            b"document-content" => Tag::Document,
            b"body" => Tag::Body,
            b"spreadsheet" => Tag::Spreadsheet,
            b"annotation" => Tag::Annotation,
            _ => Tag::Other,
        };
    }
    if is_ns(ns.clone(), TABLE) {
        return match local {
            b"table" => Tag::Table,
            b"table-row-group" | b"table-header-rows" | b"table-rows" => Tag::Rows,
            b"table-row" => Tag::Row,
            b"table-column-group" | b"table-header-columns" | b"table-columns" => Tag::Columns,
            b"table-column" => Tag::Column,
            b"shapes" => Tag::Shapes,
            b"named-range" => Tag::NamedRange,
            b"named-expression" => Tag::NamedExpression,
            b"content-validations" => Tag::Unkept,
            b"table-cell" => Tag::Cell,
            b"covered-table-cell" => Tag::Covered,
            _ => Tag::Other,
        };
    }
    if is_ns(ns.clone(), DRAW) {
        return match local {
            b"frame" => Tag::Frame,
            b"object" => Tag::Object,
            b"image" => Tag::Image,
            b"custom-shape" | b"rect" | b"line" | b"ellipse" | b"circle" | b"polyline" | b"polygon" | b"path" | b"connector" | b"caption"
            | b"measure" | b"g" | b"control" => Tag::Drawing,
            _ => Tag::Other,
        };
    }
    if is_ns(ns.clone(), DC) && local == b"creator" {
        return Tag::Creator;
    }
    if is_ns(ns.clone(), CALC_EXT) && local == b"conditional-formats" {
        return Tag::Unkept;
    }
    if is_ns(ns, TEXT) {
        return match local {
            b"p" | b"h" => Tag::Paragraph,
            b"span" | b"a" | b"meta" | b"meta-field" | b"ruby" | b"ruby-base" => Tag::Inline,
            b"s" => Tag::Space,
            b"tab" => Tag::Tab,
            b"line-break" => Tag::Break,
            _ => Tag::Other,
        };
    }
    Tag::Other
}

struct SheetDraft {
    sheet: Sheet,
    depth: usize,
    row: u32,
    /// `table:style-name` of the table (hidden sheet, tab colour).
    style: Option<String>,
    /// Columns as written: [start, end) with their width, hidden state and default cell style.
    cols: Vec<ColRange>,
    col: u32,
    /// Rows as written: [start, end) with their height and hidden state.
    rows: Vec<RowRange>,
    frames: Vec<FrameDraft>,
}
struct ColRange {
    start: u32,
    end: u32,
    width: Option<f32>,
    hidden: bool,
    style: StyleId,
}
struct RowRange {
    start: u32,
    end: u32,
    height: Option<f32>,
    optimal: bool,
    hidden: bool,
}
/// A `draw:frame` holding a chart object (`draw:object xlink:href="./Object 1"`).
struct FrameDraft {
    depth: usize,
    /// The cell the frame is anchored to; `None` for a frame placed on the sheet (`table:shapes`).
    cell: Option<CellRef>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    resizes: bool,
    href: Option<String>,
    /// A picture in the frame (a chart's frame also holds its preview picture).
    image: bool,
}
struct PendingChart {
    sheet: usize,
    href: String,
    anchor: gridcraft_model::Anchor,
}
struct RowDraft {
    depth: usize,
    repeat: u32,
    col: u32,
    cells: Vec<RepeatedCell>,
    style: Option<String>,
    hidden: bool,
    /// `table:default-cell-style-name` of the row.
    default_style: Option<String>,
}
struct RepeatedCell {
    col: u32,
    repeat: u32,
    width: u32,
    height: u32,
    cell: Cell,
    comment: Option<gridcraft_model::Comment>,
}
/// A comment (`office:annotation`) being read.
struct NoteDraft {
    depth: usize,
    author: String,
    text: String,
    paragraphs: usize,
    visible: bool,
}
struct CellDraft {
    depth: usize,
    repeat: u32,
    width: u32,
    height: u32,
    covered: bool,
    /// `table:style-name` of the cell.
    style: Option<String>,
    kind: Option<String>,
    raw: Option<String>,
    formula: bool,
    /// `table:formula`, in OpenFormula (`of:=SUM([.A1:.A3])`).
    formula_text: Option<String>,
    text: String,
    paragraph: Option<usize>,
    paragraphs: usize,
    paragraph_started: bool,
    pending_space: bool,
    note: Option<NoteDraft>,
}
struct Import {
    wb: Workbook,
    report: ReadReport,
    stack: Vec<Tag>,
    sheet: Option<SheetDraft>,
    row: Option<RowDraft>,
    cell: Option<CellDraft>,
    cells: u64,
    merges: u64,
    text: u64,
    formats: HashMap<(StyleId, &'static str), StyleId>,
    spreadsheet_seen: bool,
    styles: style::OdsStyles,
    /// The base every ODS cell style is resolved from (the model's default style).
    base: Style,
    style_ids: HashMap<String, StyleId>,
    styled_blanks: u64,
    charts: Vec<PendingChart>,
    /// The file has something Lusk Tab does not keep (`ODS_PARTIAL_NOTICE`).
    partial: bool,
}

impl Import {
    fn new(styles: style::OdsStyles) -> Self {
        let mut wb = Workbook::new();
        wb.sheets.clear();
        let mut report = ReadReport::default();
        report.warn(ODS_NOTICE);
        // The document's default cell style becomes the workbook's Normal style.
        let base = Style::default();
        let normal = styles.resolve(styles.has_cell_style("Default").then_some("Default"), &base);
        wb.styles = StyleTable::with_default(normal);
        Self {
            wb,
            report,
            stack: vec![],
            sheet: None,
            row: None,
            cell: None,
            cells: 0,
            merges: 0,
            text: 0,
            formats: HashMap::new(),
            spreadsheet_seen: false,
            styles,
            base,
            style_ids: HashMap::new(),
            styled_blanks: 0,
            charts: vec![],
            partial: false,
        }
    }

    /// The workbook style of a named ODS cell style (an unknown name is the default style).
    fn style_id(&mut self, name: &str) -> StyleId {
        if let Some(id) = self.style_ids.get(name) {
            return *id;
        }
        let id = if self.styles.has_cell_style(name) { self.wb.styles.intern(self.styles.resolve(Some(name), &self.base)) } else { StyleId::DEFAULT };
        if self.style_ids.len() < 100_000 {
            self.style_ids.insert(name.to_string(), id);
        }
        id
    }

    /// `id` with the number format a value type implies (a date, a percentage) when the style
    /// itself has none.
    fn with_format(&mut self, id: StyleId, code: Option<&'static str>) -> StyleId {
        let Some(code) = code else { return id };
        if self.wb.styles.get(id).num_fmt.as_str() != "General" {
            return id;
        }
        if let Some(found) = self.formats.get(&(id, code)) {
            return *found;
        }
        let derived = self.wb.styles.derive(id, |s| s.num_fmt = NumFmt::new(code));
        self.formats.insert((id, code), derived);
        derived
    }

    fn parse(&mut self, bytes: &[u8]) -> Result<(), IoError> {
        let mut reader = NsReader::from_reader(bytes);
        let mut root_seen = false;
        loop {
            match reader.read_event().map_err(xml_error)? {
                Event::Start(e) => {
                    if self.stack.len() >= crate::xml::MAX_DEPTH {
                        return Err(xml_error("XML nested too deeply"));
                    }
                    if self.stack.is_empty() {
                        if root_seen || tag(&mut reader, &e) != Tag::Document {
                            return Err(xml_error("expected one office:document-content root"));
                        }
                        root_seen = true;
                    }
                    self.start(&mut reader, &e)?;
                }
                Event::Empty(e) => {
                    if self.stack.is_empty() {
                        return Err(xml_error("expected office:document-content with a spreadsheet"));
                    }
                    self.start(&mut reader, &e)?;
                    self.end()?;
                }
                Event::End(_) => self.end()?,
                Event::Text(t) => {
                    let text = t.xml_content(XmlVersion::Explicit1_0).map_err(xml_error)?;
                    if self.stack.is_empty() && !text.trim().is_empty() {
                        return Err(xml_error("text outside the document"));
                    }
                    self.append(&text)?;
                }
                Event::CData(t) => self.append(&t.decode().map_err(xml_error)?)?,
                Event::GeneralRef(r) => {
                    let ch = match r.resolve_char_ref().map_err(xml_error)? {
                        Some(c) => c,
                        None => match r.as_ref() {
                            b"amp" => '&',
                            b"lt" => '<',
                            b"gt" => '>',
                            b"apos" => '\'',
                            b"quot" => '"',
                            _ => return Err(xml_error("unsupported XML entity")),
                        },
                    };
                    self.append(ch.encode_utf8(&mut [0; 4]))?;
                }
                Event::DocType(_) => return Err(xml_error("DOCTYPE is not supported")),
                Event::Eof => break,
                _ => {}
            }
        }
        if !root_seen || !self.stack.is_empty() || !self.spreadsheet_seen {
            return Err(xml_error("incomplete ODS document"));
        }
        Ok(())
    }

    fn start(&mut self, reader: &mut NsReader<&[u8]>, e: &BytesStart<'_>) -> Result<(), IoError> {
        let t = tag(reader, e);
        let depth = self.stack.len();
        // Lusk Tab : columns, charts, comments and names.
        if self.start_layout(t, depth, reader, e)? || self.start_extras(t, depth, reader, e)? {
            self.stack.push(t);
            return Ok(());
        }
        match t {
            Tag::Spreadsheet if self.stack == [Tag::Document, Tag::Body] => {
                if self.spreadsheet_seen {
                    return Err(xml_error("multiple spreadsheets in one document"));
                }
                self.spreadsheet_seen = true;
            }
            Tag::Table if self.stack == [Tag::Document, Tag::Body, Tag::Spreadsheet] => {
                if self.wb.sheets.len() >= MAX_SHEETS {
                    return Err(IoError::TooLarge("ODS contains more than 1024 sheets".into()));
                }
                let original = attr(reader, e, TABLE, b"name")?.unwrap_or_default();
                let mut name = crate::read::SheetNames::of(&self.wb).unique(original.trim_matches('\''), self.wb.sheets.len());
                if self.wb.check_sheet_name(&name, None).is_err() {
                    name = self.wb.next_sheet_name();
                }
                if name != original {
                    self.report.warn("Some ODS sheet names were shortened or renamed to valid, unique XLSX sheet names.");
                }
                let style = attr(reader, e, TABLE, b"style-name")?;
                self.sheet = Some(SheetDraft { sheet: Sheet::new(name), depth, row: 0, style, cols: vec![], col: 0, rows: vec![], frames: vec![] });
            }
            Tag::Row if self.sheet.as_ref().is_some_and(|s| self.stack.get(s.depth + 1..).is_some_and(|p| p.iter().all(|t| *t == Tag::Rows))) => {
                self.row = Some(RowDraft {
                    depth,
                    repeat: positive(attr(reader, e, TABLE, b"number-rows-repeated")?, 1, u32::MAX)?,
                    col: 0,
                    cells: vec![],
                    style: attr(reader, e, TABLE, b"style-name")?,
                    hidden: matches!(attr(reader, e, TABLE, b"visibility")?.as_deref(), Some("collapse" | "filter")),
                    default_style: attr(reader, e, TABLE, b"default-cell-style-name")?,
                });
            }
            Tag::Cell | Tag::Covered if self.row.as_ref().is_some_and(|r| depth == r.depth + 1) => {
                let kind = if attr(reader, e, CALC_EXT, b"value-type")?.as_deref() == Some("error") {
                    Some("error".into())
                } else {
                    attr(reader, e, OFFICE, b"value-type")?
                };
                let key: &[u8] = match kind.as_deref() {
                    Some("boolean") => b"boolean-value",
                    Some("date") => b"date-value",
                    Some("time") => b"time-value",
                    Some("string") => b"string-value",
                    _ => b"value",
                };
                let mut draft = CellDraft {
                    depth,
                    repeat: positive(attr(reader, e, TABLE, b"number-columns-repeated")?, 1, u32::MAX)?,
                    width: positive(attr(reader, e, TABLE, b"number-columns-spanned")?, 1, MAX_COLS)?,
                    height: positive(attr(reader, e, TABLE, b"number-rows-spanned")?, 1, MAX_ROWS)?,
                    covered: t == Tag::Covered,
                    style: attr(reader, e, TABLE, b"style-name")?,
                    kind,
                    raw: attr(reader, e, OFFICE, key)?,
                    formula_text: attr(reader, e, TABLE, b"formula")?,
                    formula: false,
                    text: String::new(),
                    paragraph: None,
                    paragraphs: 0,
                    paragraph_started: false,
                    pending_space: false,
                    note: None,
                };
                draft.formula = draft.formula_text.is_some();
                self.cell = Some(draft);
            }
            Tag::Paragraph if self.cell.as_ref().is_some_and(|c| depth == c.depth + 1) => {
                if let Some(cell) = self.cell.as_mut() {
                    if cell.paragraphs > 0 {
                        push_text(&mut cell.text, "\n")?;
                    }
                    cell.paragraph = Some(depth);
                    cell.paragraphs += 1;
                    cell.paragraph_started = false;
                    cell.pending_space = false;
                }
            }
            Tag::Space | Tag::Tab | Tag::Break if self.note_text() => {
                if let Some(note) = self.cell.as_mut().and_then(|c| c.note.as_mut()) {
                    push_text(&mut note.text, if t == Tag::Break { "\n" } else { " " })?;
                }
            }
            Tag::Space | Tag::Tab | Tag::Break if self.captures_text() => {
                let count = if t == Tag::Space {
                    let n = attr(reader, e, TEXT, b"c")?.map(|s| s.parse::<u32>()).transpose().map_err(xml_error)?.unwrap_or(1);
                    if n as usize > MAX_CELL_TEXT {
                        return Err(IoError::TooLarge("ODS repeated spaces exceed 1 MiB".into()));
                    }
                    n
                } else {
                    1
                };
                let value = match t {
                    Tag::Space => " ",
                    Tag::Tab => "\t",
                    _ => "\n",
                };
                if let Some(cell) = self.cell.as_mut() {
                    if cell.pending_space {
                        push_text(&mut cell.text, " ")?;
                        cell.pending_space = false;
                    }
                    cell.paragraph_started = true;
                    if cell.text.len().saturating_add(count as usize) > MAX_CELL_TEXT {
                        return Err(IoError::TooLarge("ODS cell text exceeds 1 MiB".into()));
                    }
                    for _ in 0..count {
                        cell.text.push_str(value);
                    }
                }
            }
            _ => {}
        }
        self.stack.push(t);
        Ok(())
    }

    /// Lusk Tab : a column (width, default style), a frame holding a chart, and its object. `true` when `t` was one of them.
    fn start_layout(&mut self, t: Tag, depth: usize, reader: &mut NsReader<&[u8]>, e: &BytesStart<'_>) -> Result<bool, IoError> {
        match t {
            Tag::Column
                if self.sheet.as_ref().is_some_and(|s| self.stack.get(s.depth + 1..).is_some_and(|p| p.iter().all(|t| *t == Tag::Columns))) =>
            {
                let repeat = positive(attr(reader, e, TABLE, b"number-columns-repeated")?, 1, u32::MAX)?;
                let width = attr(reader, e, TABLE, b"style-name")?.and_then(|n| self.styles.columns.get(&n).copied());
                let hidden = matches!(attr(reader, e, TABLE, b"visibility")?.as_deref(), Some("collapse" | "filter"));
                let style = match attr(reader, e, TABLE, b"default-cell-style-name")? {
                    Some(n) => self.style_id(&n),
                    None => StyleId::DEFAULT,
                };
                if let Some(sheet) = self.sheet.as_mut()
                    && sheet.col < MAX_COLS
                {
                    let end = sheet.col.saturating_add(repeat).min(MAX_COLS);
                    sheet.cols.push(ColRange { start: sheet.col, end, width, hidden, style });
                    sheet.col = end;
                }
            }
            Tag::Frame if self.sheet.is_some() => {
                let len = |v: Option<String>| v.as_deref().and_then(style::length_px).unwrap_or(0.0);
                let cell = match (&self.cell, &self.row, &self.sheet) {
                    (Some(_), Some(row), Some(sheet)) => Some(CellRef::new(sheet.row.min(MAX_ROWS - 1), row.col.min(MAX_COLS - 1))),
                    _ => None,
                };
                let frame = FrameDraft {
                    depth,
                    cell,
                    x: len(attr(reader, e, SVG, b"x")?),
                    y: len(attr(reader, e, SVG, b"y")?),
                    width: len(attr(reader, e, SVG, b"width")?),
                    height: len(attr(reader, e, SVG, b"height")?),
                    resizes: attr(reader, e, TABLE, b"end-cell-address")?.is_some(),
                    href: None,
                    image: false,
                };
                if let Some(sheet) = self.sheet.as_mut()
                    && sheet.frames.len() < MAX_CHARTS
                {
                    sheet.frames.push(frame);
                }
            }
            Tag::Object if self.stack.last() == Some(&Tag::Frame) => {
                let href = attr(reader, e, XLINK, b"href")?;
                if let Some(frame) = self.sheet.as_mut().and_then(|s| s.frames.last_mut())
                    && frame.depth + 1 == depth
                    && frame.href.is_none()
                {
                    frame.href = href;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Lusk Tab : a comment and its paragraphs, a picture or drawing, conditional formatting, a defined name. `true` when `t` was one of them.
    fn start_extras(&mut self, t: Tag, depth: usize, reader: &mut NsReader<&[u8]>, e: &BytesStart<'_>) -> Result<bool, IoError> {
        match t {
            Tag::Annotation if self.cell.as_ref().is_some_and(|c| depth == c.depth + 1 && c.note.is_none()) => {
                let visible = attr(reader, e, OFFICE, b"display")?.as_deref() == Some("true");
                if let Some(cell) = self.cell.as_mut() {
                    cell.note = Some(NoteDraft { depth, author: String::new(), text: String::new(), paragraphs: 0, visible });
                }
            }
            Tag::Paragraph if self.cell.as_ref().and_then(|c| c.note.as_ref()).is_some_and(|n| depth == n.depth + 1) => {
                if let Some(note) = self.cell.as_mut().and_then(|c| c.note.as_mut()) {
                    if note.paragraphs > 0 {
                        push_text(&mut note.text, "\n")?;
                    }
                    note.paragraphs += 1;
                }
            }
            Tag::Image if self.stack.last() == Some(&Tag::Frame) => {
                if let Some(frame) = self.sheet.as_mut().and_then(|s| s.frames.last_mut())
                    && frame.depth + 1 == depth
                {
                    frame.image = true;
                }
            }
            Tag::Drawing if matches!(self.stack.last(), Some(Tag::Cell | Tag::Shapes)) => self.partial = true,
            Tag::Unkept => self.partial = true,
            Tag::NamedRange | Tag::NamedExpression if self.spreadsheet_seen => {
                let name = attr(reader, e, TABLE, b"name")?.unwrap_or_default();
                let formula = if t == Tag::NamedRange {
                    attr(reader, e, TABLE, b"cell-range-address")?.filter(|a| chart::split_spaces(a).len() == 1).and_then(|a| odf_reference(&a))
                } else {
                    attr(reader, e, TABLE, b"expression")?.and_then(|x| odf_formula(&x))
                };
                if let Some(formula) = formula.filter(|f| !f.starts_with('#'))
                    && !name.is_empty()
                    && self.wb.names.len() < 10_000
                {
                    let scope = self.sheet.as_ref().map(|_| self.wb.sheets.len());
                    self.wb.names.push(gridcraft_model::DefinedName { name, scope, formula, comment: String::new(), hidden: false });
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn captures_text(&self) -> bool {
        self.cell
            .as_ref()
            .and_then(|c| c.paragraph)
            .is_some_and(|depth| self.stack.get(depth..).is_some_and(|p| p.iter().all(|t| matches!(t, Tag::Paragraph | Tag::Inline))))
    }

    /// Inside a comment's paragraph.
    fn note_text(&self) -> bool {
        self.cell.as_ref().and_then(|c| c.note.as_ref()).is_some_and(|n| {
            self.stack.get(n.depth + 1..).is_some_and(|p| !p.is_empty() && p.iter().all(|t| matches!(t, Tag::Paragraph | Tag::Inline)))
        })
    }

    fn append(&mut self, text: &str) -> Result<(), IoError> {
        let creator = self.stack.last() == Some(&Tag::Creator);
        if creator || self.note_text() {
            if let Some(note) = self.cell.as_mut().and_then(|c| c.note.as_mut())
                && self.stack.len() > note.depth
            {
                push_text(if creator { &mut note.author } else { &mut note.text }, text)?;
            }
            return Ok(());
        }
        if self.captures_text()
            && let Some(cell) = self.cell.as_mut()
        {
            // ODF 1.3 section 6.1.2: collapse XML whitespace across inline runs,
            // but expand explicit text:s/tab/line-break elements separately.
            for ch in text.chars() {
                if matches!(ch, ' ' | '\t' | '\n' | '\r') {
                    cell.pending_space = cell.paragraph_started;
                } else {
                    if cell.pending_space {
                        push_text(&mut cell.text, " ")?;
                        cell.pending_space = false;
                    }
                    push_text(&mut cell.text, ch.encode_utf8(&mut [0; 4]))?;
                    cell.paragraph_started = true;
                }
            }
        }
        Ok(())
    }

    fn end(&mut self) -> Result<(), IoError> {
        self.stack.pop().ok_or_else(|| xml_error("unexpected closing element"))?;
        let depth = self.stack.len();
        if let Some(cell) = self.cell.as_mut()
            && cell.paragraph == Some(depth)
        {
            cell.paragraph = None;
        }
        if let Some(sheet) = self.sheet.as_ref()
            && let Some(frame) = sheet.frames.last()
            && frame.depth == depth
            && frame.image
            && frame.href.is_none()
        {
            // A picture, not a chart's preview.
            self.partial = true;
        }
        if self.cell.as_ref().is_some_and(|c| c.depth == depth) {
            self.finish_cell()?;
        }
        if self.row.as_ref().is_some_and(|r| r.depth == depth) {
            self.finish_row()?;
        }
        if self.sheet.as_ref().is_some_and(|s| s.depth == depth)
            && let Some(sheet) = self.sheet.take()
        {
            self.finish_sheet(sheet);
        }
        Ok(())
    }

    fn finish_cell(&mut self) -> Result<(), IoError> {
        let Some(mut draft) = self.cell.take() else { return Ok(()) };
        let (cell, format) = self.convert(&draft);
        let mut comment = draft.note.take().map(|n| gridcraft_model::Comment {
            author: n.author.trim().to_string(),
            text: n.text.trim_end().to_string(),
            replies: vec![],
            threaded: false,
            resolved: false,
            visible: n.visible,
            dates: vec![],
            note_box: None,
        });
        let (width, height) = if draft.covered { (1, 1) } else { (draft.width, draft.height) };
        let Some(row) = self.row.as_ref() else { return Ok(()) };
        let (col, row_repeat) = (row.col, row.repeat);
        let row_style = row.default_style.clone();
        let merged = width > 1 || height > 1;
        let content = cell.has_content() || merged || comment.is_some();
        // Writers pad rows with blank repeats up to their own (possibly wider) sheet edge;
        // blank cells past ours carry no data, so clip them instead of rejecting the file.
        let end = match col.checked_add(draft.repeat).filter(|n| *n <= MAX_COLS) {
            Some(end) => end,
            None if !content => MAX_COLS,
            None => return Err(IoError::TooLarge("ODS row exceeds worksheet width".into())),
        };
        // Lusk Tab : the cell's own style, else the row's, else each column's default style.
        let own = draft.style.as_deref().or(row_style.as_deref()).map(|name| self.style_id(name));
        let segments = self.segments(col, end, own, merged);
        for (start, stop, style, column) in segments {
            let style = self.with_format(style, format);
            let count = u64::from(stop - start) * u64::from(row_repeat);
            let kept = content
                || (style != StyleId::DEFAULT
                    && style != column
                    && stop - start <= MAX_STYLED_RUN
                    && self.styled_blanks.saturating_add(count) <= MAX_STYLED_BLANKS);
            if !kept {
                continue;
            }
            if !content {
                self.styled_blanks += count;
            }
            let cell = Cell { style, ..cell.clone() };
            // Charge the text a row keeps before the row is done, so a row of many long cells is
            // refused as it grows; finish_row charges the row's repeats beyond the first.
            if let Value::Text(t) = &cell.value {
                self.text = self.text.saturating_add(u64::from(stop - start).saturating_mul(t.len() as u64));
                if self.text > MAX_TEXT {
                    return Err(IoError::TooLarge("ODS expands beyond the import limit (64 MiB of text)".into()));
                }
            }
            if let Some(row) = self.row.as_mut() {
                row.cells.push(RepeatedCell { col: start, repeat: stop - start, width, height, cell, comment: comment.take() });
            }
        }
        if let Some(row) = self.row.as_mut() {
            row.col = end;
        }
        Ok(())
    }

    /// The runs of [start, end) by style: the cell's own style for all of it, or each column's
    /// default style. Each run also carries its column's default style.
    fn segments(&self, start: u32, end: u32, own: Option<StyleId>, merged: bool) -> Vec<(u32, u32, StyleId, StyleId)> {
        let cols = self.sheet.as_ref().map_or(&[][..], |s| &s.cols[..]);
        let column_at = |c: u32| cols.iter().find(|r| r.start <= c && c < r.end).map_or(StyleId::DEFAULT, |r| r.style);
        if start >= end {
            return vec![];
        }
        if merged {
            let column = column_at(start);
            return vec![(start, end, own.unwrap_or(column), column)];
        }
        let mut out: Vec<(u32, u32, StyleId, StyleId)> = Vec::new();
        let mut c = start;
        while c < end {
            let range = cols.iter().find(|r| r.start <= c && c < r.end);
            let (stop, column) = match range {
                Some(r) => (r.end.min(end), r.style),
                None => (cols.iter().map(|r| r.start).filter(|s| *s > c).min().unwrap_or(end).min(end), StyleId::DEFAULT),
            };
            let style = own.unwrap_or(column);
            match out.last_mut() {
                Some(last) if last.1 == c && last.2 == style && last.3 == column => last.1 = stop,
                _ => out.push((c, stop, style, column)),
            }
            c = stop;
        }
        out
    }

    fn convert(&mut self, draft: &CellDraft) -> (Cell, Option<&'static str>) {
        // Lusk Tab : a formula LibreOffice wrote is kept, so the sheet recalculates, with its saved
        // result as the value until then. One that can't be converted stays its saved result.
        let formula = draft.formula_text.as_deref().and_then(|f| {
            let converted = odf_formula(f).map(|text| Formula::new(&crate::fmla::from_file(&text)));
            if converted.as_ref().is_none_or(|f| f.expr.is_none()) {
                self.report.warn(ODS_FORMULA_NOTICE);
                return None;
            }
            converted
        });
        let (mut cell, format) = self.convert_value(draft, formula.is_some());
        if let Some(f) = formula {
            cell.formula = Some(Arc::new(f));
        }
        (cell, format)
    }

    /// The cell's value, and the number format its value type implies (a date, a percentage).
    fn convert_value(&mut self, draft: &CellDraft, has_formula: bool) -> (Cell, Option<&'static str>) {
        let raw = draft.raw.as_deref();
        let mut format = None;
        let value = match draft.kind.as_deref() {
            Some("float" | "percentage" | "currency") => raw.and_then(|s| s.parse::<f64>().ok()).filter(|n| n.is_finite()).map(|n| {
                if draft.kind.as_deref() == Some("percentage") {
                    format = Some("0.00%");
                }
                Value::Number(n)
            }),
            Some("boolean") => match raw {
                Some("true" | "1") => Some(Value::Bool(true)),
                Some("false" | "0") => Some(Value::Bool(false)),
                _ => None,
            },
            Some("string") if raw.is_some() || draft.paragraphs > 0 || !draft.formula => Some(Value::Text(Arc::from(raw.unwrap_or(&draft.text)))),
            Some("void") if !draft.formula => Some(Value::Empty),
            Some("date") => raw.and_then(parse_date).map(|(n, f)| {
                format = Some(f);
                Value::Number(n)
            }),
            Some("time") => raw.and_then(parse_duration).map(|n| {
                format = Some("[h]:mm:ss.000");
                Value::Number(n)
            }),
            None if !draft.formula => Some(if draft.text.is_empty() { Value::Empty } else { Value::Text(Arc::from(draft.text.as_str())) }),
            _ => None,
        };
        // A kept formula's error result (`#DIV/0!` written as text) is that error, and a missing
        // result is left to the recalculation.
        let value = if has_formula {
            match value {
                Some(Value::Text(t)) => Some(CellError::parse(t.trim()).map_or(Value::Text(t), Value::Error)),
                None => Some(CellError::parse(draft.text.trim()).map_or(Value::Empty, Value::Error)),
                v => v,
            }
        } else {
            value
        };
        let value = value.unwrap_or_else(|| {
            self.report.warn("Some ODS values or formula caches were unsupported or missing; their displayed text (or raw value) was kept as text. A formula without either is marked [Unavailable ODS formula result].");
            let text = if !draft.text.is_empty() { draft.text.as_str() } else if let Some(raw) = raw { raw } else if draft.formula { "[Unavailable ODS formula result]" } else { "[Unsupported ODS value]" };
            Value::Text(Arc::from(text))
        });
        (Cell::value(value), format)
    }

    fn finish_row(&mut self) -> Result<(), IoError> {
        let Some(row) = self.row.take() else { return Ok(()) };
        let Some(sheet) = self.sheet.as_mut() else { return Ok(()) };
        // Blank trailing row repeats past the sheet edge (e.g. from larger-sheet writers) are clipped.
        let end = match sheet.row.checked_add(row.repeat).filter(|n| *n <= MAX_ROWS) {
            Some(end) => end,
            None if row.cells.is_empty() => MAX_ROWS,
            None => return Err(IoError::TooLarge("ODS exceeds worksheet height".into())),
        };
        let props = row.style.as_deref().and_then(|n| self.styles.rows.get(n)).copied().unwrap_or_default();
        if sheet.rows.len() < MAX_ROW_INFOS * 2 {
            sheet.rows.push(RowRange { start: sheet.row, end, height: props.height, optimal: props.optimal, hidden: row.hidden });
        }
        for entry in row.cells {
            let count = u64::from(entry.repeat) * u64::from(row.repeat);
            if !entry.cell.is_blank() {
                self.cells = self.cells.saturating_add(count);
                if let Value::Text(t) = &entry.cell.value {
                    let repeats = count.saturating_sub(u64::from(entry.repeat));
                    self.text = self.text.saturating_add(repeats.saturating_mul(t.len() as u64));
                }
            }
            let merged = entry.width > 1 || entry.height > 1;
            if merged {
                self.merges = self.merges.saturating_add(count);
            }
            if self.cells > MAX_CELLS || self.merges > MAX_MERGES || self.text > MAX_TEXT {
                return Err(IoError::TooLarge("ODS expands beyond the import limit (1 million cells, 100000 merges or 64 MiB of text)".into()));
            }
            let last_col = entry.col.checked_add(entry.repeat - 1).and_then(|c| c.checked_add(entry.width - 1));
            let last_row = end.checked_add(entry.height - 1);
            if last_col.is_none_or(|c| c >= MAX_COLS) || last_row.is_none_or(|r| r > MAX_ROWS) {
                return Err(IoError::TooLarge("ODS merged cell exceeds worksheet bounds".into()));
            }
            if merged && ((entry.repeat > 1 && entry.width > 1) || (row.repeat > 1 && entry.height > 1)) {
                return Err(IoError::Format("repeated ODS cells have overlapping merges".into()));
            }
            if let Some(comment) = entry.comment {
                sheet.sheet.comments.insert(CellRef::new(sheet.row, entry.col), comment);
            }
            for r in sheet.row..end {
                for c in entry.col..entry.col + entry.repeat {
                    let pos = CellRef::new(r, c);
                    if !entry.cell.is_blank() {
                        sheet.sheet.cells.set(pos, entry.cell.clone());
                    }
                    if merged {
                        sheet.sheet.merges.push(RangeRef::new(pos, CellRef::new(r + entry.height - 1, c + entry.width - 1)));
                    }
                }
            }
        }
        sheet.row = end;
        Ok(())
    }

    /// Lusk Tab : the sheet's visibility, tab colour, column widths, row heights and charts.
    fn finish_sheet(&mut self, draft: SheetDraft) {
        let SheetDraft { mut sheet, style, cols, rows, frames, .. } = draft;
        if let Some(p) = style.as_deref().and_then(|n| self.styles.tables.get(n)) {
            if p.hidden {
                sheet.visibility = Visibility::Hidden;
            }
            sheet.tab_color = p.tab_color;
        }
        let round = |v: f32| (v * 100.0).round() / 100.0;
        // The most common width and height become the sheet's defaults; the others are kept.
        let default_width = most_common(cols.iter().filter_map(|r| r.width.map(|w| (round(w), r.end - r.start))));
        if let Some(w) = default_width.filter(|w| *w > 0.0) {
            sheet.default_col_width = w;
        }
        for r in &cols {
            let size = r.width.map(round).filter(|w| Some(*w) != default_width);
            if size.is_none() && !r.hidden && r.style == StyleId::DEFAULT {
                continue;
            }
            for c in r.start..r.end {
                sheet
                    .cols
                    .insert(c, LineInfo { size, hidden: r.hidden, style: (r.style != StyleId::DEFAULT).then_some(r.style), ..Default::default() });
            }
        }
        let default_height = most_common(rows.iter().filter_map(|r| r.height.map(|h| (round(h), r.end - r.start))));
        if let Some(h) = default_height.filter(|h| *h > 0.0) {
            sheet.default_row_height = h;
        }
        let mut infos = 0usize;
        'rows: for r in &rows {
            let size = r.height.map(round).filter(|h| !r.optimal && Some(*h) != default_height);
            if size.is_none() && !r.hidden {
                continue;
            }
            for row in r.start..r.end {
                if infos >= MAX_ROW_INFOS {
                    break 'rows;
                }
                sheet.rows.insert(row, LineInfo { size, hidden: r.hidden, custom: size.is_some(), ..Default::default() });
                infos += 1;
            }
        }
        let index = self.wb.sheets.len();
        for f in frames {
            let Some(href) = f.href else { continue };
            let relative = f.cell.filter(|c| f.x <= column_width(&sheet, c.col) + 1.0 && f.y <= row_height(&sheet, c.row) + 1.0);
            let (cell, dx, dy) = match relative {
                Some(cell) => (cell, f.x.max(0.0), f.y.max(0.0)),
                None => locate(&sheet, f.x.max(0.0), f.y.max(0.0)),
            };
            let mode = if f.resizes { AnchorMode::MoveAndSize } else { AnchorMode::MoveOnly };
            let anchor = gridcraft_model::Anchor { cell, dx, dy, width: f.width.max(1.0), height: f.height.max(1.0), mode };
            self.charts.push(PendingChart { sheet: index, href, anchor });
        }
        self.wb.sheets.push(Arc::new(sheet));
    }

    /// Lusk Tab : the charts the sheets' frames point to (`Object 1/content.xml`).
    fn read_charts(&mut self, package: &mut Package<'_>) -> Result<(), IoError> {
        let mut failed = false;
        for p in std::mem::take(&mut self.charts) {
            let dir = p.href.trim_start_matches("./").trim_end_matches('/');
            if dir.is_empty() || dir.contains("..") || dir.starts_with('/') || dir.contains(':') {
                continue;
            }
            let Some(doc) =
                package.read_optional_limited(&format!("{dir}/content.xml"), MAX_CHART_PART).ok().flatten().and_then(|b| crate::xml::parse(&b).ok())
            else {
                failed = true;
                continue;
            };
            // Other embedded objects (a formula, a text frame) are not charts.
            if doc.path(&["body", "chart"]).is_none() {
                continue;
            }
            let styles =
                package.read_optional_limited(&format!("{dir}/styles.xml"), MAX_CHART_PART).ok().flatten().and_then(|b| crate::xml::parse(&b).ok());
            let Some(sheet) = self.wb.sheets.get_mut(p.sheet) else { continue };
            let sheet = Arc::make_mut(sheet);
            let id = sheet.charts.iter().map(|c| c.id).max().unwrap_or(0) + 1;
            match chart::read(&doc, styles.as_ref(), p.anchor, id) {
                Some(c) => sheet.charts.push(c),
                None => failed = true,
            }
        }
        if failed {
            self.report.warn(ODS_CHART_NOTICE);
        }
        Ok(())
    }
}

/// Lusk Tab : the view LibreOffice keeps in settings.xml: the active sheet, frozen rows and
/// columns, grid lines.
fn apply_settings(wb: &mut Workbook, settings: &crate::xml::El) {
    fn find<'a>(el: &'a crate::xml::El, name: &str, out: &mut Vec<&'a crate::xml::El>) {
        for k in &el.children {
            if k.attr("name") == Some(name) {
                out.push(k);
            }
            find(k, name, out);
        }
    }
    let mut views = Vec::new();
    find(settings, "Tables", &mut views);
    let Some(tables) = views.first() else { return };
    let item =
        |entry: &crate::xml::El, name: &str| entry.kids("config-item").find(|i| i.attr("name") == Some(name)).map(|i| i.text.trim().to_string());
    for entry in tables.kids("config-item-map-entry") {
        let Some(name) = entry.attr("name") else { continue };
        let Some(sheet) = wb.sheets.iter_mut().find(|s| s.name == name) else { continue };
        let number = |key: &str| item(entry, key).and_then(|v| v.parse::<u32>().ok());
        let frozen_cols = (number("HorizontalSplitMode") == Some(2)).then(|| number("HorizontalSplitPosition")).flatten().unwrap_or(0);
        let frozen_rows = (number("VerticalSplitMode") == Some(2)).then(|| number("VerticalSplitPosition")).flatten().unwrap_or(0);
        let sheet = Arc::make_mut(sheet);
        if (frozen_rows > 0 || frozen_cols > 0) && frozen_rows < MAX_ROWS && frozen_cols < MAX_COLS {
            sheet.freeze = Some((frozen_rows, frozen_cols));
        }
        if item(entry, "ShowGrid").as_deref() == Some("false") {
            sheet.show_gridlines = false;
        }
    }
    let mut active = Vec::new();
    find(settings, "ActiveTable", &mut active);
    if let Some(i) = active.first().and_then(|a| wb.sheets.iter().position(|s| s.name == a.text.trim())) {
        wb.set_active_sheet(i);
    }
}

/// The value seen over the most columns or rows (ties go to the smaller value).
fn most_common(values: impl Iterator<Item = (f32, u32)>) -> Option<f32> {
    let mut counts: Vec<(f32, u64)> = Vec::new();
    for (v, n) in values {
        match counts.iter().position(|(w, _)| *w == v) {
            Some(i) => counts[i].1 += u64::from(n),
            None if counts.len() < 4096 => counts.push((v, u64::from(n))),
            None => {}
        }
    }
    counts.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.total_cmp(&a.0))).map(|(v, _)| v)
}

fn column_width(sheet: &Sheet, col: u32) -> f32 {
    match sheet.cols.get(&col) {
        Some(info) if info.hidden => 0.0,
        Some(LineInfo { size: Some(w), .. }) => *w,
        _ => sheet.default_col_width,
    }
}

fn row_height(sheet: &Sheet, row: u32) -> f32 {
    match sheet.rows.get(&row) {
        Some(info) if info.hidden => 0.0,
        Some(LineInfo { size: Some(h), .. }) => *h,
        _ => sheet.default_row_height,
    }
}

/// The cell at a position on the sheet (pixels from its top-left corner), and the offset in it.
fn locate(sheet: &Sheet, mut x: f32, mut y: f32) -> (CellRef, f32, f32) {
    let (mut col, mut row) = (0u32, 0u32);
    for _ in 0..MAX_COLS {
        let w = column_width(sheet, col);
        if x < w || col + 1 >= MAX_COLS {
            break;
        }
        x -= w;
        col += 1;
    }
    for _ in 0..100_000 {
        let h = row_height(sheet, row);
        if y < h || row + 1 >= MAX_ROWS {
            break;
        }
        y -= h;
        row += 1;
    }
    (CellRef::new(row, col), x, y)
}

fn push_text(out: &mut String, text: &str) -> Result<(), IoError> {
    if out.len().saturating_add(text.len()) > MAX_CELL_TEXT {
        return Err(IoError::TooLarge("ODS cell text exceeds 1 MiB".into()));
    }
    out.push_str(text);
    Ok(())
}

/// Support unzoned ISO calendar dates and times. A timezone, invalid date or out-of-range
/// calendar year remains text; never silently discard an offset or roll an invalid date.
fn parse_date(s: &str) -> Option<(f64, &'static str)> {
    let (date, time) = s.split_once('T').map_or((s, None), |(d, t)| (d, Some(t)));
    if date.len() != 10 || date.get(4..5)? != "-" || date.get(7..8)? != "-" {
        return None;
    }
    let year: i32 = digits(date.get(..4)?)?.parse().ok()?;
    let month: u32 = digits(date.get(5..7)?)?.parse().ok()?;
    let day: u32 = digits(date.get(8..10)?)?.parse().ok()?;
    if !(1900..=9999).contains(&year) || !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let day = serial_from_ymd(DateSystem::D1900, year.into(), month.into(), day.into())?;
    let Some(time) = time else { return Some((day, "yyyy-mm-dd")) };
    let mut parts = time.split(':');
    let h = parts.next()?;
    let m = parts.next()?;
    let sec = parts.next()?;
    if parts.next().is_some() || h.len() != 2 || m.len() != 2 || sec.split('.').next()?.len() != 2 {
        return None;
    }
    let h: u32 = digits(h)?.parse().ok()?;
    let m: u32 = digits(m)?.parse().ok()?;
    let sec = decimal(sec)?;
    if h >= 24 || m >= 60 || sec >= 60.0 {
        return None;
    }
    Some((day + (h as f64 * 3600.0 + m as f64 * 60.0 + sec) / 86400.0, "yyyy-mm-dd hh:mm:ss.000"))
}

fn digits(s: &str) -> Option<&str> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then_some(s)
}
fn decimal(s: &str) -> Option<f64> {
    match s.split_once('.') {
        Some((a, b)) => {
            digits(a)?;
            digits(b)?;
        }
        None => {
            digits(s)?;
        }
    }
    s.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// A bounded ISO day/time duration. Calendar years/months cannot be represented as days.
fn parse_duration(s: &str) -> Option<f64> {
    let (negative, s) = s.strip_prefix('-').map_or((false, s), |s| (true, s));
    let mut rest = s.strip_prefix('P')?;
    let mut total = 0.0;
    let mut any = false;
    if let Some((days, tail)) = rest.split_once('D') {
        total += digits(days)?.parse::<f64>().ok()? * 86400.0;
        rest = tail;
        any = true;
    }
    if let Some(mut time) = rest.strip_prefix('T') {
        let mut time_any = false;
        for (unit, factor) in [('H', 3600.0), ('M', 60.0), ('S', 1.0)] {
            if let Some((n, tail)) = time.split_once(unit) {
                let value = if unit == 'S' { decimal(n)? } else { digits(n)?.parse::<f64>().ok()? };
                total += value * factor;
                time = tail;
                time_any = true;
            }
        }
        if !time.is_empty() || !time_any {
            return None;
        }
        any = true;
    } else if !rest.is_empty() {
        return None;
    }
    let days = total / 86400.0;
    (any && days.is_finite() && days <= 2_958_465.0).then_some(if negative { -days } else { days })
}
