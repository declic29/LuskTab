//! Lusk Tab : the formatting, charts, comments and view of .ods files, read and written. The
//! packages are assembled in Rust, not files from another spreadsheet application.

use std::sync::Arc;

use gridcraft_core::{CellRef, RangeRef, Value};
use gridcraft_model::{
    Anchor, AnchorMode, BorderLine, BorderStyle, Cell, Chart, ChartKind, Color, Comment, DefinedName, Fill, Formula, HAlign, LegendPos, LineInfo,
    NumFmt, Series, Sheet, StyleId, Underline, VAlign, Visibility, Workbook,
};

use super::make_zip;
use crate::{ODS_PARTIAL_NOTICE, read_ods, read_xlsx, write_ods, write_xlsx};

const MIME: &[u8] = b"application/vnd.oasis.opendocument.spreadsheet";
const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:chart="urn:oasis:names:tc:opendocument:xmlns:chart:1.0" xmlns:config="urn:oasis:names:tc:opendocument:xmlns:config:1.0" xmlns:calcext="urn:org:documentfoundation:names:experimental:calc:xmlns:calcext:1.0" xmlns:tableooo="http://openoffice.org/2009/table""#;

const STYLES: &str = r##"<style:default-style style:family="table-cell"><style:text-properties style:font-name="Liberation Sans" fo:font-size="10pt"/></style:default-style>
<number:number-style style:name="N2"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/><number:text> €</number:text></number:number-style>
<style:style style:name="Default" style:family="table-cell"><style:text-properties fo:color="#000000"/></style:style>
<style:style style:name="Titre" style:family="table-cell" style:parent-style-name="Default"><style:text-properties fo:font-size="14pt" fo:font-weight="bold"/></style:style>"##;

const AUTOMATIC: &str = r##"<style:style style:name="co1" style:family="table-column"><style:table-column-properties style:column-width="1in"/></style:style>
<style:style style:name="co2" style:family="table-column"><style:table-column-properties style:column-width="0.75in"/></style:style>
<style:style style:name="ro1" style:family="table-row"><style:table-row-properties style:row-height="0.1875in" style:use-optimal-row-height="true"/></style:style>
<style:style style:name="ro2" style:family="table-row"><style:table-row-properties style:row-height="0.5in" style:use-optimal-row-height="false"/></style:style>
<style:style style:name="ta1" style:family="table"><style:table-properties table:display="true"/></style:style>
<style:style style:name="ta2" style:family="table"><style:table-properties table:display="false" tableooo:tab-color="#00ff00"/></style:style>
<number:percentage-style style:name="N3"><number:number number:decimal-places="1" number:min-integer-digits="1"/><number:text>%</number:text></number:percentage-style>
<number:date-style style:name="N4"><number:day number:style="long"/><number:text>/</number:text><number:month number:style="long"/><number:text>/</number:text><number:year number:style="long"/></number:date-style>
<number:number-style style:name="N5P0" style:volatile="true"><number:number number:decimal-places="2" number:min-integer-digits="1"/></number:number-style>
<number:number-style style:name="N5"><style:text-properties fo:color="#ff0000"/><number:text>-</number:text><number:number number:decimal-places="2" number:min-integer-digits="1"/><style:map style:condition="value()&gt;=0" style:apply-style-name="N5P0"/></number:number-style>
<style:style style:name="ce1" style:family="table-cell" style:parent-style-name="Titre"><style:table-cell-properties fo:background-color="#1f4e79" fo:border="0.74pt solid #000000" fo:border-bottom="1.76pt solid #ff0000" fo:wrap-option="wrap" style:vertical-align="middle" style:text-align-source="fix"/><style:paragraph-properties fo:text-align="center"/><style:text-properties fo:color="#ffffff" fo:font-style="italic" style:text-underline-style="solid" style:font-name="Liberation Serif"/></style:style>
<style:style style:name="ce2" style:family="table-cell" style:parent-style-name="Default" style:data-style-name="N2"/>
<style:style style:name="ce3" style:family="table-cell" style:parent-style-name="Default" style:data-style-name="N3"/>
<style:style style:name="ce4" style:family="table-cell" style:parent-style-name="Default" style:data-style-name="N4"/>
<style:style style:name="ce5" style:family="table-cell" style:parent-style-name="Default" style:data-style-name="N5"/>
<style:style style:name="ce6" style:family="table-cell" style:parent-style-name="Default"><style:table-cell-properties fo:background-color="#ffff00"/></style:style>"##;

fn package(body: &str, objects: &[(&str, String)], settings: Option<&str>) -> Vec<u8> {
    let content = format!(
        r#"<?xml version="1.0"?><office:document-content {NS}><office:font-face-decls><style:font-face style:name="Liberation Sans" svg:font-family="'Liberation Sans'" style:font-family-generic="swiss"/><style:font-face style:name="Liberation Serif" svg:font-family="'Liberation Serif'" style:font-family-generic="roman"/></office:font-face-decls><office:automatic-styles>{AUTOMATIC}</office:automatic-styles><office:body><office:spreadsheet>{body}</office:spreadsheet></office:body></office:document-content>"#
    );
    let styles = format!(r#"<?xml version="1.0"?><office:document-styles {NS}><office:styles>{STYLES}</office:styles></office:document-styles>"#);
    let manifest = r#"<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/></manifest:manifest>"#;
    let mut parts: Vec<(String, Vec<u8>)> = vec![
        ("mimetype".into(), MIME.to_vec()),
        ("META-INF/manifest.xml".into(), manifest.as_bytes().to_vec()),
        ("content.xml".into(), content.into_bytes()),
        ("styles.xml".into(), styles.into_bytes()),
    ];
    for (name, xml) in objects {
        parts.push((format!("{name}/content.xml"), xml.as_bytes().to_vec()));
    }
    if let Some(s) = settings {
        parts.push((
            "settings.xml".into(),
            format!(r#"<?xml version="1.0"?><office:document-settings {NS}><office:settings>{s}</office:settings></office:document-settings>"#)
                .into_bytes(),
        ));
    }
    let refs: Vec<(&str, &[u8])> = parts.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
    make_zip(&refs)
}

#[test]
fn ods_cell_formatting_number_formats_widths_and_heights_are_read() {
    let bytes = package(
        r#"<table:table table:name="Budget" table:style-name="ta1">
            <table:table-column table:style-name="co1" table:default-cell-style-name="Default"/>
            <table:table-column table:style-name="co2" table:number-columns-repeated="2" table:default-cell-style-name="Default"/>
            <table:table-column table:style-name="co2" table:default-cell-style-name="ce6"/>
            <table:table-column table:style-name="co2" table:visibility="collapse" table:default-cell-style-name="Default"/>
            <table:table-column table:style-name="co2" table:number-columns-repeated="1019" table:default-cell-style-name="Default"/>
            <table:table-row table:style-name="ro1">
              <table:table-cell table:style-name="ce1" office:value-type="string"><text:p>Recettes</text:p></table:table-cell>
              <table:table-cell table:style-name="ce2" office:value-type="float" office:value="1200.5"/>
              <table:table-cell table:style-name="ce3" office:value-type="percentage" office:value="0.125"/>
              <table:table-cell office:value-type="float" office:value="7"/>
              <table:table-cell table:number-columns-repeated="1020"/>
            </table:table-row>
            <table:table-row table:style-name="ro2">
              <table:table-cell table:style-name="ce4" office:value-type="date" office:date-value="2026-03-15"/>
              <table:table-cell table:style-name="ce5" office:value-type="float" office:value="-3"/>
              <table:table-cell table:style-name="ce6" table:number-columns-repeated="2"/>
            </table:table-row>
            <table:table-row table:style-name="ro1" table:number-rows-repeated="1048574"><table:table-cell table:number-columns-repeated="1024"/></table:table-row>
          </table:table>
          <table:table table:name="Cachée" table:style-name="ta2"><table:table-row><table:table-cell/></table:table-row></table:table>"#,
        &[],
        None,
    );
    let (wb, report) = read_ods(&bytes).unwrap();
    assert!(!report.warnings.iter().any(|w| w == ODS_PARTIAL_NOTICE));
    // The document's default style is the workbook's Normal style.
    let normal = wb.styles.get(StyleId::DEFAULT);
    assert_eq!((normal.font.name.as_str(), normal.font.size, normal.font.color), ("Liberation Sans", 10.0, Color::Rgb(0)));
    let sh = &wb.sheets[0];
    let style = |r: u32, c: u32| wb.styles.get(sh.cell(CellRef::new(r, c)).map_or(StyleId::DEFAULT, |c| c.style));
    let title = style(0, 0);
    assert_eq!(title.font.name, "Liberation Serif");
    assert_eq!((title.font.size, title.font.bold, title.font.italic, title.font.underline), (14.0, true, true, Underline::Single));
    assert_eq!(title.font.color, Color::Rgb(0xffffff));
    assert_eq!(title.fill, Fill::solid(Color::Rgb(0x1f4e79)));
    assert_eq!(title.borders.left, BorderLine { style: BorderStyle::Thin, color: Color::Rgb(0) });
    assert_eq!(title.borders.bottom, BorderLine { style: BorderStyle::Medium, color: Color::Rgb(0xff0000) });
    assert_eq!((title.align.h, title.align.v, title.align.wrap), (HAlign::Center, VAlign::Center, true));
    assert_eq!(style(0, 1).num_fmt.as_str(), "#,##0.00\" €\"");
    assert_eq!(style(0, 2).num_fmt.as_str(), "0.0%");
    assert_eq!(style(1, 0).num_fmt.as_str(), "dd/mm/yyyy");
    assert_eq!(style(1, 1).num_fmt.as_str(), "0.00;[Red]-0.00");
    // Column D's default style (a yellow background) is the column's style; a cell without its
    // own style there takes it, and the blank yellow cells beside it are not stored one by one.
    let yellow = Fill::solid(Color::Rgb(0xffff00));
    assert_eq!(style(0, 3).fill, yellow);
    assert_eq!(sh.cols.get(&3).and_then(|c| c.style).map(|s| wb.styles.get(s).fill), Some(yellow));
    assert!(sh.cell(CellRef::new(1, 3)).is_none());
    assert_eq!(style(1, 2).fill, yellow, "a formatted blank cell is kept");
    // Widths: the most common one is the default, the others are kept; hidden column E.
    assert_eq!(sh.default_col_width, 72.0);
    assert_eq!(sh.cols.get(&0).and_then(|c| c.size), Some(96.0));
    assert!(sh.cols.get(&4).is_some_and(|c| c.hidden));
    // Heights: the optimal height is the default, the fixed one is kept.
    assert_eq!(sh.default_row_height, 18.0);
    assert_eq!(sh.rows.get(&1).map(|r| (r.size, r.custom)), Some((Some(48.0), true)));
    assert!(!sh.rows.contains_key(&0));
    // The hidden sheet with its tab colour.
    assert_eq!((wb.sheets[1].visibility, wb.sheets[1].tab_color), (Visibility::Hidden, Some(Color::Rgb(0x00ff00))));
    // And the same formatting in the .xlsx it saves as.
    let (x, _) = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
    let xs = |r: u32, c: u32| x.styles.get(x.sheets[0].cell(CellRef::new(r, c)).map_or(StyleId::DEFAULT, |c| c.style)).clone();
    assert_eq!(xs(0, 0).fill, title.fill);
    assert!(xs(0, 0).font.bold);
    assert_eq!(xs(1, 1).num_fmt.as_str(), "0.00;[Red]-0.00");
    assert_eq!(x.sheets[0].cols.get(&0).and_then(|c| c.size).map(f32::round), Some(96.0));
}

fn chart_object(class: &str, plot_props: &str, series_extra: &str) -> String {
    format!(
        r##"<?xml version="1.0"?><office:document-content {NS}><office:automatic-styles>
        <style:style style:name="ch1" style:family="chart"><style:chart-properties {plot_props}/></style:style>
        <style:style style:name="ch2" style:family="chart"><style:graphic-properties draw:fill-color="#4472c4"/></style:style>
        </office:automatic-styles><office:body><office:chart><chart:chart svg:width="12cm" svg:height="7cm" chart:class="{class}">
        <chart:title><text:p>Recettes <text:span>2026</text:span></text:p></chart:title>
        <chart:legend chart:legend-position="bottom"/>
        <chart:plot-area chart:style-name="ch1">
          <chart:axis chart:dimension="x" chart:name="primary-x"><chart:title><text:p>Mois</text:p></chart:title><chart:categories table:cell-range-address="Data.A2:Data.A5"/></chart:axis>
          <chart:axis chart:dimension="y" chart:name="primary-y"><chart:grid chart:class="major"/></chart:axis>
          <chart:series chart:style-name="ch2" chart:values-cell-range-address="Data.B2:Data.B5" chart:label-cell-address="Data.B1:Data.B1" chart:class="{class}"/>
          <chart:series chart:values-cell-range-address="'Mes données'.C2:'Mes données'.C5" chart:label-cell-address="'Mes données'.C1" {series_extra}/>
        </chart:plot-area></chart:chart></office:chart></office:body></office:document-content>"##
    )
}

#[test]
fn ods_charts_are_read_from_their_objects() {
    let body = r#"<table:table table:name="Data"><table:table-row><table:table-cell office:value-type="float" office:value="1"/><table:table-cell>
          <draw:frame svg:width="12cm" svg:height="7cm" svg:x="0.1in" svg:y="0in" table:end-cell-address="Data.H20"><draw:object xlink:href="./Object 1"/><draw:image xlink:href="./ObjectReplacements/Object 1"/></draw:frame>
          <draw:frame svg:width="10cm" svg:height="6cm" svg:x="0in" svg:y="0in"><draw:object xlink:href="./Object 2"/></draw:frame>
          <draw:frame svg:width="10cm" svg:height="6cm"><draw:object xlink:href="./Object 3"/></draw:frame>
        </table:table-cell></table:table-row></table:table>"#;
    let objects = [
        ("Object 1", chart_object("chart:bar", r#"chart:stacked="true""#, "")),
        ("Object 2", chart_object("chart:bar", r#"chart:vertical="true""#, r#"chart:class="chart:line""#)),
        ("Object 3", chart_object("chart:circle", r#"chart:angle-offset="90""#, "")),
    ];
    let (wb, report) = read_ods(&package(body, &objects, None)).unwrap();
    assert!(!report.warnings.iter().any(|w| w == ODS_PARTIAL_NOTICE), "a chart's preview picture is not a picture of its own");
    let charts = &wb.sheets[0].charts;
    assert_eq!(charts.len(), 3);
    let c = &charts[0];
    assert_eq!(c.kind, ChartKind::ColumnStacked);
    assert_eq!(c.title.as_deref(), Some("Recettes 2026"));
    assert_eq!((c.legend, c.gridlines, c.x_title.as_deref()), (LegendPos::Bottom, true, Some("Mois")));
    assert_eq!((c.anchor.cell, c.anchor.dx, c.anchor.mode), (CellRef::new(0, 1), 9.6, AnchorMode::MoveAndSize));
    assert!((c.anchor.width - 453.5).abs() < 0.1 && (c.anchor.height - 264.6).abs() < 0.1);
    assert_eq!(c.series[0].name.as_deref(), Some("Data!$B$1"));
    assert_eq!(c.series[0].categories.as_deref(), Some("Data!$A$2:$A$5"));
    assert_eq!(c.series[0].values, "Data!$B$2:$B$5");
    assert_eq!(c.series[0].color, Some(Color::Rgb(0x4472c4)));
    assert_eq!(c.series[1].values, "'Mes données'!$C$2:$C$5");
    // Bars and a line: a combination chart.
    assert_eq!(charts[1].kind, ChartKind::Combo);
    assert_eq!(charts[1].series.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![Some(ChartKind::BarClustered), Some(ChartKind::Line)]);
    assert_eq!(charts[1].anchor.mode, AnchorMode::MoveOnly);
    assert_eq!((charts[2].kind, charts[2].first_slice_angle), (ChartKind::Pie, 0));
}

#[test]
fn ods_comments_names_view_and_unkept_features() {
    let body = r#"<table:table table:name="Data">
          <table:table-row><table:table-cell office:value-type="float" office:value="1"><office:annotation office:display="true"><dc:creator>Philippe</dc:creator><dc:date>2026-11-22T10:00:00</dc:date><text:p>Vérifier</text:p><text:p>avec le <text:span>bureau</text:span></text:p></office:annotation><text:p>1</text:p></table:table-cell>
          <table:table-cell><office:annotation><text:p>seule</text:p></office:annotation></table:table-cell></table:table-row>
          <table:named-expressions><table:named-range table:name="Local" table:base-cell-address="$Data.$A$1" table:cell-range-address="$Data.$A$1:.$A$3"/></table:named-expressions>
        </table:table>
        <table:named-expressions><table:named-range table:name="Recettes" table:base-cell-address="$Data.$A$1" table:cell-range-address="$Data.$B$2:.$B$9"/><table:named-expression table:name="Taux" table:base-cell-address="$Data.$A$1" table:expression="of:=0.055*[$Data.$A$1]"/></table:named-expressions>"#;
    let settings = r#"<config:config-item-set config:name="ooo:view-settings"><config:config-item-map-indexed config:name="Views"><config:config-item-map-entry><config:config-item-map-named config:name="Tables"><config:config-item-map-entry config:name="Data"><config:config-item config:name="HorizontalSplitMode" config:type="short">2</config:config-item><config:config-item config:name="VerticalSplitMode" config:type="short">2</config:config-item><config:config-item config:name="HorizontalSplitPosition" config:type="int">1</config:config-item><config:config-item config:name="VerticalSplitPosition" config:type="int">2</config:config-item><config:config-item config:name="ShowGrid" config:type="boolean">false</config:config-item></config:config-item-map-entry></config:config-item-map-named><config:config-item config:name="ActiveTable" config:type="string">Data</config:config-item></config:config-item-map-entry></config:config-item-map-indexed></config:config-item-set>"#;
    let (wb, report) = read_ods(&package(body, &[], Some(settings))).unwrap();
    let sh = &wb.sheets[0];
    let note = sh.comments.get(&CellRef::new(0, 0)).unwrap();
    assert_eq!((note.author.as_str(), note.text.as_str(), note.visible), ("Philippe", "Vérifier\navec le bureau", true));
    assert_eq!(sh.value(CellRef::new(0, 0)), Value::Number(1.0), "the comment's text is not the cell's");
    assert_eq!(sh.comments.get(&CellRef::new(0, 1)).map(|c| c.text.as_str()), Some("seule"));
    let names: Vec<(&str, Option<usize>, &str)> = wb.names.iter().map(|n| (n.name.as_str(), n.scope, n.formula.as_str())).collect();
    assert_eq!(names, vec![("Local", Some(0), "Data!$A$1:$A$3"), ("Recettes", None, "Data!$B$2:$B$9"), ("Taux", None, "0.055*Data!$A$1")]);
    assert_eq!((sh.freeze, sh.show_gridlines), (Some((2, 1)), false));
    assert!(!report.warnings.iter().any(|w| w == ODS_PARTIAL_NOTICE));
    // A picture, or conditional formatting, is not kept: the open notice says so.
    let picture = r#"<table:table table:name="Data"><table:table-row><table:table-cell><draw:frame svg:width="1in" svg:height="1in"><draw:image xlink:href="Pictures/logo.png"/></draw:frame></table:table-cell></table:table-row></table:table>"#;
    let (_, report) = read_ods(&package(picture, &[], None)).unwrap();
    assert!(report.warnings.iter().any(|w| w == ODS_PARTIAL_NOTICE));
    let conditional =
        r#"<table:table table:name="Data"><table:table-row><table:table-cell/></table:table-row><calcext:conditional-formats/></table:table>"#;
    let (_, report) = read_ods(&package(conditional, &[], None)).unwrap();
    assert!(report.warnings.iter().any(|w| w == ODS_PARTIAL_NOTICE));
}

/// A workbook with what an association's budget holds: formatting, number formats, widths,
/// merged cells, formulas, a comment, a name, frozen panes, a hidden sheet and a chart.
fn budget() -> Workbook {
    let mut wb = Workbook::new();
    let header = wb.styles.derive(StyleId::DEFAULT, |s| {
        s.font.bold = true;
        s.font.color = Color::Rgb(0xffffff);
        s.fill = Fill::solid(Color::Rgb(0x1f4e79));
        s.align.h = HAlign::Center;
        s.borders.bottom = BorderLine { style: BorderStyle::Medium, color: Color::Rgb(0xff0000) };
    });
    let euros = wb.styles.derive(StyleId::DEFAULT, |s| s.num_fmt = NumFmt::new("#,##0.00\" €\";[Red]-#,##0.00\" €\""));
    let percent = wb.styles.derive(StyleId::DEFAULT, |s| s.num_fmt = NumFmt::new("0.0%"));
    let date = wb.styles.derive(StyleId::DEFAULT, |s| s.num_fmt = NumFmt::new("dd/mm/yyyy"));
    let wrap = wb.styles.derive(StyleId::DEFAULT, |s| {
        s.align.wrap = true;
        s.align.v = VAlign::Top;
        s.font.italic = true;
        s.font.size = 9.0;
    });
    let sh = Arc::make_mut(&mut wb.sheets[0]);
    sh.name = "Budget 2026".into();
    let set = |sh: &mut Sheet, r: u32, c: u32, v: Value, style: StyleId| sh.cells.set(CellRef::new(r, c), Cell { style, ..Cell::value(v) });
    for (c, t) in ["Mois", "Recettes", "Dépenses", "Taux"].iter().enumerate() {
        set(sh, 0, c as u32, Value::Text((*t).into()), header);
    }
    for (r, (m, a, b)) in [("Janv", 1200.5, 900.0), ("Févr", 1300.0, 1950.25), ("Mars", 1400.0, 1000.0)].iter().enumerate() {
        let r = r as u32 + 1;
        set(sh, r, 0, Value::Text((*m).into()), StyleId::DEFAULT);
        set(sh, r, 1, Value::Number(*a), euros);
        set(sh, r, 2, Value::Number(*b), euros);
        let mut rate = Cell { style: percent, ..Cell::value(Value::Number(b / a)) };
        rate.formula = Some(Arc::new(Formula::new(&format!("C{0}/B{0}", r + 1))));
        sh.cells.set(CellRef::new(r, 3), rate);
    }
    let mut total = Cell { style: euros, ..Cell::value(Value::Number(3900.5)) };
    total.formula = Some(Arc::new(Formula::new("SUM(B2:B4)")));
    sh.cells.set(CellRef::new(4, 1), total);
    set(sh, 4, 0, Value::Text("Total".into()), header);
    set(sh, 6, 0, Value::Number(46096.0), date);
    set(sh, 7, 0, Value::Text("  Note :\tdeux espaces\net une ligne".into()), wrap);
    set(sh, 8, 0, Value::Bool(true), StyleId::DEFAULT);
    sh.merges.push(RangeRef::new(CellRef::new(7, 0), CellRef::new(7, 2)));
    sh.cols.insert(0, LineInfo { size: Some(120.0), ..Default::default() });
    sh.cols.insert(5, LineInfo { hidden: true, ..Default::default() });
    sh.rows.insert(7, LineInfo { size: Some(40.0), custom: true, ..Default::default() });
    sh.freeze = Some((1, 0));
    sh.comments.insert(
        CellRef::new(4, 1),
        Comment {
            author: "Philippe".into(),
            text: "Vérifié en bureau".into(),
            replies: vec![],
            threaded: false,
            resolved: false,
            visible: false,
            dates: vec![],
            note_box: None,
        },
    );
    sh.charts.push(Chart {
        id: 1,
        kind: ChartKind::ColumnClustered,
        anchor: Anchor { cell: CellRef::new(1, 6), dx: 4.0, dy: 2.0, width: 480.0, height: 288.0, mode: AnchorMode::MoveAndSize },
        title: Some("Recettes et dépenses".into()),
        series: vec![
            Series {
                name: Some("'Budget 2026'!$B$1".into()),
                categories: Some("'Budget 2026'!$A$2:$A$4".into()),
                values: "'Budget 2026'!$B$2:$B$4".into(),
                color: Some(Color::Rgb(0x4472c4)),
                ..Default::default()
            },
            Series {
                name: Some("'Budget 2026'!$C$1".into()),
                categories: Some("'Budget 2026'!$A$2:$A$4".into()),
                values: "'Budget 2026'!$C$2:$C$4".into(),
                color: Some(Color::Rgb(0xed7d31)),
                ..Default::default()
            },
        ],
        legend: LegendPos::Bottom,
        data_labels: false,
        data_table: None,
        gridlines: true,
        style: 1,
        hole_size: 50,
        first_slice_angle: 0,
        x_title: None,
        y_title: Some("Euros".into()),
        source: None,
        by_rows: false,
        pivot: None,
    });
    let mut archive = Sheet::new("Archives");
    archive.visibility = Visibility::Hidden;
    archive.tab_color = Some(Color::Rgb(0x00b050));
    archive.cells.set(CellRef::new(0, 0), Cell::value(Value::Text("2025".into())));
    wb.sheets.push(Arc::new(archive));
    wb.names.push(DefinedName {
        name: "Recettes".into(),
        scope: None,
        formula: "'Budget 2026'!$B$2:$B$4".into(),
        comment: String::new(),
        hidden: false,
    });
    wb
}

#[test]
fn ods_written_by_lusk_tab_reads_back_whole() {
    let wb = budget();
    let bytes = write_ods(&wb).unwrap();
    // An ODF package: the uncompressed mimetype first, then the parts the manifest lists.
    assert_eq!(&bytes[30..38], b"mimetype");
    assert_eq!(&bytes[38..38 + MIME.len()], MIME);
    let (back, report) = read_ods(&bytes).unwrap();
    assert!(!report.warnings.iter().any(|w| w == ODS_PARTIAL_NOTICE), "{:?}", report.warnings);
    assert_eq!(back.sheets.len(), 2);
    let (a, b) = (&wb.sheets[0], &back.sheets[0]);
    assert_eq!(b.name, "Budget 2026");
    for (pos, cell) in a.cells.iter() {
        let got = b.cell(pos).unwrap_or_else(|| panic!("{pos:?} missing"));
        assert_eq!(got.value, cell.value, "value at {pos:?}");
        assert_eq!(got.formula.as_ref().map(|f| f.text.clone()), cell.formula.as_ref().map(|f| f.text.clone()), "formula at {pos:?}");
        let (sa, sb) = (wb.styles.get(cell.style), back.styles.get(got.style));
        assert_eq!(sb.num_fmt, sa.num_fmt, "format at {pos:?}");
        assert_eq!(
            (sb.font.bold, sb.font.italic, sb.font.size, sb.font.color),
            (sa.font.bold, sa.font.italic, sa.font.size, sa.font.color),
            "font at {pos:?}"
        );
        assert_eq!(
            (sb.fill, sb.borders, sb.align.h, sb.align.v, sb.align.wrap),
            (sa.fill, sa.borders, sa.align.h, sa.align.v, sa.align.wrap),
            "style at {pos:?}"
        );
    }
    assert_eq!(b.merges, a.merges);
    assert_eq!(b.cols.get(&0).and_then(|c| c.size), Some(120.0));
    assert!(b.cols.get(&5).is_some_and(|c| c.hidden));
    assert_eq!(b.rows.get(&7).and_then(|r| r.size), Some(40.0));
    assert_eq!((b.default_col_width, b.default_row_height), (a.default_col_width, a.default_row_height));
    assert_eq!(b.freeze, Some((1, 0)));
    assert_eq!(b.comments.get(&CellRef::new(4, 1)).map(|c| (c.author.as_str(), c.text.as_str())), Some(("Philippe", "Vérifié en bureau")));
    assert_eq!(back.names.iter().map(|n| (n.name.as_str(), n.formula.as_str())).collect::<Vec<_>>(), vec![("Recettes", "'Budget 2026'!$B$2:$B$4")]);
    let (ca, cb) = (&a.charts[0], &b.charts[0]);
    assert_eq!((cb.kind, &cb.title, cb.legend, cb.gridlines, &cb.y_title), (ca.kind, &ca.title, ca.legend, ca.gridlines, &ca.y_title));
    assert_eq!((cb.anchor.cell, cb.anchor.mode), (ca.anchor.cell, ca.anchor.mode));
    assert!((cb.anchor.width - ca.anchor.width).abs() < 0.5 && (cb.anchor.dx - ca.anchor.dx).abs() < 0.5);
    for (sa, sb) in ca.series.iter().zip(&cb.series) {
        assert_eq!((&sb.name, &sb.categories, &sb.values, sb.color), (&sa.name, &sa.categories, &sa.values, sa.color));
    }
    assert_eq!((back.sheets[1].visibility, back.sheets[1].tab_color), (Visibility::Hidden, Some(Color::Rgb(0x00b050))));
    // The same workbook saved as .xlsx from the .ods reads as the original does.
    let (x, _) = read_xlsx(&write_xlsx(&back).unwrap()).unwrap();
    for (pos, cell) in a.cells.iter() {
        let got = x.sheets[0].cell(pos).unwrap();
        assert_eq!(
            (&got.value, x.styles.get(got.style).num_fmt.as_str()),
            (&cell.value, wb.styles.get(cell.style).num_fmt.as_str()),
            "xlsx at {pos:?}"
        );
    }
    assert_eq!(x.sheets[0].charts.len(), 1);
}

/// An Excel file saved as .ods and opened again keeps what the .xlsx had.
#[test]
fn xlsx_saved_as_ods_keeps_its_content() {
    let (excel, _) = read_xlsx(&write_xlsx(&budget()).unwrap()).unwrap();
    let (back, _) = read_ods(&write_ods(&excel).unwrap()).unwrap();
    for (pos, cell) in excel.sheets[0].cells.iter() {
        let got = back.sheets[0].cell(pos).unwrap();
        assert_eq!(got.value, cell.value, "{pos:?}");
        assert_eq!(got.formula.as_ref().map(|f| &f.text), cell.formula.as_ref().map(|f| &f.text), "{pos:?}");
        assert_eq!(back.styles.get(got.style).num_fmt, excel.styles.get(cell.style).num_fmt, "{pos:?}");
        assert_eq!(back.styles.get(got.style).fill, excel.styles.get(cell.style).fill, "{pos:?}");
    }
    assert_eq!(back.sheets[0].charts.len(), excel.sheets[0].charts.len());
}
