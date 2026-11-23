//! Lusk Tab : the charts of an .ods file. Each chart is an embedded object
//! (`Object 1/content.xml`, an `office:chart` document); its series point at the sheet's cells
//! (`'Budget 2026'.B2:'Budget 2026'.B5`), which become the model's own series references.
//! Format reference: ODF 1.3 part 3, section 11 (charts).

use std::collections::HashMap;

use gridcraft_model::{Anchor, Chart, ChartKind, Color, LegendPos, Series};

use super::style::el_text;
use crate::xml::El;

/// Series one chart keeps, at most.
const MAX_SERIES: usize = 255;

/// Properties of a chart's automatic styles, by style name: `chart:*`, `draw:*` and `svg:*`
/// attributes of their `style:chart-properties` and `style:graphic-properties`, by local name.
type Props = HashMap<String, HashMap<String, String>>;

fn props(doc: &El, styles: Option<&El>) -> Props {
    let mut out = Props::new();
    for root in [styles, Some(doc)].into_iter().flatten() {
        for list in root.children.iter().filter(|c| c.name == "automatic-styles" || c.name == "styles") {
            for style in list.kids("style") {
                let Some(name) = style.attr("name") else { continue };
                let entry = out.entry(name.to_string()).or_default();
                for p in style.children.iter().filter(|c| matches!(c.name.as_str(), "chart-properties" | "graphic-properties")) {
                    for (k, v) in &p.attrs {
                        entry.insert(k.clone(), v.clone());
                    }
                }
            }
        }
    }
    out
}

/// A chart object's document as a model chart anchored at `anchor`; `None` when it has no
/// series the model can show.
pub(super) fn read(doc: &El, styles: Option<&El>, anchor: Anchor, id: u32) -> Option<Chart> {
    let chart = doc.path(&["body", "chart", "chart"])?;
    let props = props(doc, styles);
    let style_of = |el: &El| el.attr("style-name").and_then(|n| props.get(n));
    let get = |el: &El, key: &str| style_of(el).and_then(|p| p.get(key)).map(String::as_str);
    let class = class_name(chart.attr("class").unwrap_or("chart:bar"));
    let plot = chart.child("plot-area")?;
    let flag = |key: &str| get(plot, key) == Some("true") || get(chart, key) == Some("true");
    let (vertical, stacked, percent) = (flag("vertical"), flag("stacked"), flag("percentage"));
    let kind_of = |class: &str, series: Option<&El>| -> ChartKind {
        let line_markers = || {
            let symbol = series.and_then(|s| get(s, "symbol-type")).or_else(|| get(plot, "symbol-type"));
            !matches!(symbol, None | Some("none"))
        };
        match class {
            "bar" => match (vertical, stacked, percent) {
                (true, _, true) => ChartKind::BarStacked100,
                (true, true, _) => ChartKind::BarStacked,
                (true, ..) => ChartKind::BarClustered,
                (false, _, true) => ChartKind::ColumnStacked100,
                (false, true, _) => ChartKind::ColumnStacked,
                _ => ChartKind::ColumnClustered,
            },
            "line" if stacked || percent => ChartKind::LineStacked,
            "line" if line_markers() => ChartKind::LineMarkers,
            "line" => ChartKind::Line,
            "area" if percent => ChartKind::AreaStacked100,
            "area" if stacked => ChartKind::AreaStacked,
            "area" => ChartKind::Area,
            "circle" => ChartKind::Pie,
            "ring" => ChartKind::Doughnut,
            "scatter" => {
                let stroke = series.and_then(|s| get(s, "stroke")).or_else(|| get(plot, "stroke"));
                if stroke == Some("none") { ChartKind::Scatter } else { ChartKind::ScatterLines }
            }
            "bubble" => ChartKind::Bubble,
            "radar" | "filled-radar" => ChartKind::Radar,
            "stock" => ChartKind::Stock,
            _ => ChartKind::ColumnClustered,
        }
    };
    let kind = kind_of(class, None);
    let axis = |dim: &str| plot.kids("axis").find(|a| a.attr("dimension") == Some(dim) && a.attr("name").is_none_or(|n| n.starts_with("primary")));
    let categories = axis("x").and_then(|a| a.child("categories")).and_then(|c| c.attr("cell-range-address")).and_then(range);
    let mut series = Vec::new();
    let mut kinds = Vec::new();
    for s in plot.kids("series").take(MAX_SERIES) {
        let Some(values) = s.attr("values-cell-range-address").and_then(range) else { continue };
        let own = s.attr("class").map(class_name).unwrap_or(class);
        let series_kind = kind_of(own, Some(s));
        let domains: Vec<String> = s.kids("domain").filter_map(|d| d.attr("cell-range-address")).filter_map(range).collect();
        let (categories, values, bubble_sizes) = match series_kind {
            // Bubbles: the values are the sizes, the first domain the Y values, the second the X values.
            ChartKind::Bubble => (domains.get(1).cloned(), domains.first().cloned().unwrap_or_else(|| values.clone()), Some(values)),
            ChartKind::Scatter | ChartKind::ScatterLines => (domains.first().cloned().or_else(|| categories.clone()), values, None),
            _ => (categories.clone(), values, None),
        };
        let line =
            matches!(series_kind, ChartKind::Line | ChartKind::LineMarkers | ChartKind::LineStacked | ChartKind::ScatterLines | ChartKind::Radar);
        // A pie's colours are its slices', not one series colour.
        let color = get(s, if line { "stroke-color" } else { "fill-color" })
            .and_then(Color::from_hex)
            .filter(|_| !matches!(series_kind, ChartKind::Pie | ChartKind::Doughnut));
        let smooth = matches!(get(s, "interpolation").or_else(|| get(plot, "interpolation")), Some("cubic-spline" | "b-spline"));
        kinds.push(series_kind);
        series.push(Series {
            name: s.attr("label-cell-address").and_then(range),
            categories,
            values,
            bubble_sizes,
            color,
            secondary: s.attr("attached-axis").is_some_and(|a| a.starts_with("secondary")),
            kind: None,
            smooth,
            ..Default::default()
        });
    }
    if series.is_empty() {
        return None;
    }
    // Series of several kinds (columns and a line): a combination chart.
    let combo = kinds.iter().any(|k| *k != kind);
    if combo {
        for (s, k) in series.iter_mut().zip(&kinds) {
            s.kind = Some(*k);
        }
    }
    let data_labels = plot.kids("series").any(|s| {
        matches!(get(s, "data-label-number"), Some("value" | "percentage" | "value-and-percentage")) || get(s, "data-label-text") == Some("true")
    });
    let legend = match chart.child("legend").and_then(|l| l.attr("legend-position")) {
        None if chart.child("legend").is_none() => LegendPos::None,
        Some(p) if p.starts_with("start") => LegendPos::Left,
        Some(p) if p.starts_with("top") => LegendPos::Top,
        Some(p) if p.starts_with("bottom") => LegendPos::Bottom,
        _ => LegendPos::Right,
    };
    let title = chart.child("title").map(el_text).map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let axis_title = |dim: &str| axis(dim).and_then(|a| a.child("title")).map(el_text).map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let pie = matches!(kind, ChartKind::Pie | ChartKind::Doughnut);
    // ODF measures the first slice counter-clockwise from 3 o'clock (90 by default); the model
    // clockwise from 12 o'clock.
    let offset = get(plot, "angle-offset").and_then(|a| a.parse::<f64>().ok()).filter(|a| a.is_finite()).unwrap_or(90.0);
    Some(Chart {
        id,
        kind: if combo { ChartKind::Combo } else { kind },
        anchor,
        title,
        series,
        legend,
        data_labels,
        data_table: None,
        gridlines: axis("y").is_some_and(|a| a.kids("grid").any(|g| g.attr("class").is_none_or(|c| c == "major"))),
        style: 1,
        hole_size: 50,
        first_slice_angle: if pie { (90.0 - offset).rem_euclid(360.0).round() as u16 % 360 } else { 0 },
        x_title: axis_title("x"),
        y_title: axis_title("y"),
        source: plot.attr("cell-range-address").and_then(range),
        by_rows: plot.attr("data-source-has-labels").is_some() && chart_series_by_rows(plot),
        pivot: None,
    })
}

/// A series laid out along a row (`B2:E2`) means the chart reads its data by rows.
fn chart_series_by_rows(plot: &El) -> bool {
    let Some(v) = plot.kids("series").next().and_then(|s| s.attr("values-cell-range-address")).and_then(range) else { return false };
    let Some((_, cells)) = v.rsplit_once('!') else { return false };
    let mut parts = cells.split(':').map(|p| p.trim_start_matches('$').trim_start_matches(|c: char| c.is_ascii_alphabetic()).replace('$', ""));
    matches!((parts.next(), parts.next()), (Some(a), Some(b)) if a == b && !a.is_empty())
}

/// `chart:bar` → `bar`.
fn class_name(class: &str) -> &str {
    class.rsplit_once(':').map_or(class, |(_, c)| c)
}

/// A chart range (`'Budget 2026'.A2:'Budget 2026'.A5`, several separated by spaces) as the
/// model's absolute reference (`'Budget 2026'!$A$2:$A$5`); several ranges in parentheses.
pub(super) fn range(address: &str) -> Option<String> {
    let parts = split_spaces(address);
    let mut out = Vec::new();
    for part in parts.iter().filter(|p| !p.is_empty()) {
        let r = super::odf_reference(part)?;
        if r.starts_with('#') || !r.contains('!') {
            return None;
        }
        out.push(absolute(&r));
    }
    match out.len() {
        0 => None,
        1 => out.pop(),
        _ => Some(format!("({})", out.join(","))),
    }
}

/// Splits on spaces outside quoted sheet names.
pub(super) fn split_spaces(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut quoted, mut start) = (false, 0);
    for (i, c) in s.char_indices() {
        if c == '\'' {
            quoted = !quoted;
        } else if c == ' ' && !quoted {
            parts.push(&s[start..i]);
            start = i + 1;
        }
    }
    parts.push(&s[start..]);
    parts
}

/// `Sheet!A1:B2` → `Sheet!$A$1:$B$2`; a one-cell range becomes that cell.
fn absolute(r: &str) -> String {
    let (sheet, cells) = r.rsplit_once('!').unwrap_or(("", r));
    let fix = |cell: &str| {
        let cell = cell.replace('$', "");
        let split = cell.find(|c: char| c.is_ascii_digit()).unwrap_or(cell.len());
        let (col, row) = cell.split_at(split);
        match (col.is_empty(), row.is_empty()) {
            (false, false) => format!("${col}${row}"),
            (false, true) => format!("${col}"),
            _ => format!("${row}"),
        }
    };
    let mut cells: Vec<String> = cells.split(':').map(fix).collect();
    if cells.len() == 2 && cells[0] == cells[1] {
        cells.pop();
    }
    format!("{sheet}!{}", cells.join(":"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_ranges_become_absolute_references() {
        assert_eq!(range("'Budget 2026'.A2:'Budget 2026'.A5").as_deref(), Some("'Budget 2026'!$A$2:$A$5"));
        assert_eq!(range("'Budget 2026'.B1:'Budget 2026'.B1").as_deref(), Some("'Budget 2026'!$B$1"));
        assert_eq!(range("Feuille1.$A$1:.$B$3").as_deref(), Some("Feuille1!$A$1:$B$3"));
        assert_eq!(range("Data.A1:Data.A2 Data.C1:Data.C2").as_deref(), Some("(Data!$A$1:$A$2,Data!$C$1:$C$2)"));
        assert_eq!(range(".A1"), None);
    }
}
