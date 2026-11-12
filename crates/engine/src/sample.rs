//! Sample workbooks built in code (original content), for the start screen, demos, README
//! screenshots and tests.

use std::sync::Arc;

use gridcraft_model::{Sheet, Workbook};
use serde_json::json;

use crate::{DocState, Session};

/// Lusk Tab : modèles en français (contenu original), pour une association et ses membres. Les
/// identifiants restent ceux de GridCraft (`budget`, `sales`, `grades`), que `--sample` et les
/// tests utilisent.
pub const SAMPLES: &[(&str, &str)] = &[("budget", "Budget familial"), ("sales", "Comptes de l'association"), ("grades", "Suivi des adhérents")];

pub fn title(name: &str) -> String {
    SAMPLES.iter().find(|(n, _)| *n == name).map(|(_, t)| t.to_string()).unwrap_or_else(|| "Modèle".into())
}

/// Builds a sample by replaying commands (so samples also exercise the command layer).
pub fn build(name: &str) -> Option<Workbook> {
    let mut s = Session::new();
    let mut wb = Workbook::new();
    if let Some(sh) = wb.sheet_mut(0) {
        sh.name = match name {
            "budget" => "Budget".into(),
            "sales" => "Comptes".into(),
            "grades" => "Adhérents".into(),
            _ => return None,
        };
    }
    s.add_document(DocState::new(wb, None, title(name), crate::Trust::Local));
    let ok = match name {
        "budget" => budget(&mut s),
        "sales" => accounts(&mut s),
        "grades" => members(&mut s),
        _ => false,
    };
    if !ok {
        return None;
    }
    let mut wb = (*s.doc().ok()?.wb).clone();
    wb.set_active_sheet(0);
    if let Some(sh) = wb.sheet_mut(0) {
        sh.view_active = gridcraft_core::CellRef::default();
    }
    Some(wb)
}

fn run(s: &mut Session, id: &str, p: serde_json::Value) -> bool {
    match s.execute(id, p) {
        Ok(_) => true,
        Err(e) => {
            log::warn!("sample: {id} failed: {e}");
            false
        }
    }
}

/// Montants en euros, avec les séparateurs de la région d'affichage (1 850,00 €).
const EUROS: &str = "#,##0.00 \"€\"";
const EUROS_ROUND: &str = "#,##0 \"€\"";

/// Comptes de l'association : les recettes de l'exercice par trimestre, leur total, leur
/// évolution et leur tendance, deux graphiques et une synthèse.
fn accounts(s: &mut Session) -> bool {
    let rows = json!([
        ["Recette", "T1", "T2", "T3", "T4", "Total", "Évolution", "Tendance"],
        ["Cotisations", 3240, 1180, 860, 2410],
        ["Subventions", 2500, 1000, 4000, 1500],
        ["Dons", 640, 520, 380, 1290],
        ["Manifestations", 1150, 2870, 3420, 980],
        ["Buvette et ventes", 410, 1260, 1880, 350],
        ["Partenariats", 800, 800, 1200, 1200],
    ]);
    let mut ok = run(s, "range.setValues", json!({"range": "B4", "values": rows}));
    ok &= run(s, "range.setValues", json!({"range": "B2", "values": [["Comptes de l'association — exercice 2026"]]}));
    ok &= run(s, "home.cellStyle", json!({"range": "B2", "name": "Title"}));
    ok &= run(s, "range.setValues", json!({"range": "B3", "values": [["Recettes par trimestre · montants en euros"]]}));
    ok &= run(s, "home.cellStyle", json!({"range": "B3", "name": "Explanatory Text"}));
    for r in 5..=10 {
        ok &= run(s, "cell.set", json!({"cell": format!("G{r}"), "input": format!("=SUM(C{r}:F{r})")}));
        ok &= run(s, "cell.set", json!({"cell": format!("H{r}"), "input": format!("=F{r}/C{r}-1")}));
        ok &= run(s, "insert.sparkline", json!({"range": format!("C{r}:F{r}"), "location": format!("I{r}"), "type": "line", "markers": true}));
    }
    ok &= run(s, "insert.table", json!({"range": "B4:I10", "style": "TableStyleMedium7", "name": "Recettes"}));
    ok &= run(s, "table.totalRow", json!({"table": "Recettes", "on": true}));
    for col in ["T1", "T2", "T3", "T4", "Total"] {
        ok &= run(s, "table.totalFunction", json!({"table": "Recettes", "column": col, "function": "sum"}));
    }
    ok &= run(s, "table.totalFunction", json!({"table": "Recettes", "column": "Évolution", "function": "average"}));
    ok &= run(s, "table.totalFunction", json!({"table": "Recettes", "column": "Tendance", "function": "none"}));
    ok &= run(s, "home.numberFormat", json!({"range": "C5:G11", "code": EUROS_ROUND}));
    ok &= run(s, "home.numberFormat", json!({"range": "H5:H11", "code": "0.0%"}));
    ok &= run(s, "home.conditionalFormat", json!({"range": "G5:G10", "rule": {"type": "dataBar", "color": "#7BC143"}}));
    ok &= run(s, "home.conditionalFormat", json!({"range": "H5:H10", "rule": {"type": "iconSet", "set": "3Arrows"}}));
    ok &= run(s, "home.columnWidth", json!({"cols": "A:A", "width": 20}));
    ok &= run(s, "home.columnWidth", json!({"cols": "B:B", "width": 150}));
    ok &= run(s, "home.columnWidth", json!({"cols": "C:G", "width": 84}));
    ok &= run(s, "home.columnWidth", json!({"cols": "H:I", "width": 80}));
    ok &= run(s, "selection.set", json!({"range": "B4:F10"}));
    ok &= run(s, "insert.chart", json!({"type": "column", "title": "Recettes par trimestre", "at": "K3", "width": 460, "height": 270}));
    // Les totaux de l'année en secteurs.
    ok &= run(
        s,
        "insert.chart",
        json!({"range": "B4:B10", "type": "pie", "title": "Répartition des recettes", "at": "K19", "width": 460, "height": 260}),
    );
    if let Ok(d) = s.doc_mut() {
        let wb = Arc::make_mut(&mut d.wb);
        if let Some(sh) = wb.sheet_mut(0)
            && let Some(c) = sh.charts.last_mut()
        {
            c.series = vec![gridcraft_model::Series {
                name: Some("Comptes!$G$4".into()),
                categories: Some("Comptes!$B$5:$B$10".into()),
                values: "Comptes!$G$5:$G$10".into(),
                bubble_sizes: None,
                color: None,
                secondary: false,
                kind: None,
                quartiles: Default::default(),
                smooth: false,
                trendlines: Vec::new(),
            }];
        }
    }
    ok &= run(
        s,
        "range.setValues",
        json!({"range": "B14", "values": [["Plus forte recette"], ["Moyenne par trimestre"], ["Part des cotisations"], ["Évolution T1 → T4"]]}),
    );
    ok &= run(s, "cell.set", json!({"cell": "C14", "input": "=INDEX(Recettes[Recette],MATCH(MAX(Recettes[Total]),Recettes[Total],0))"}));
    ok &= run(s, "cell.set", json!({"cell": "C15", "input": "=AVERAGE(C5:F10)"}));
    ok &= run(s, "cell.set", json!({"cell": "C16", "input": "=G5/SUM(G5:G10)"}));
    ok &= run(s, "cell.set", json!({"cell": "C17", "input": "=SUM(F5:F10)/SUM(C5:C10)-1"}));
    ok &= run(s, "home.numberFormat", json!({"range": "C15", "code": EUROS_ROUND}));
    ok &= run(s, "home.numberFormat", json!({"range": "C16:C17", "code": "0.0%"}));
    ok &= run(s, "home.bold", json!({"range": "B14:B17", "on": true}));
    ok &= run(s, "home.borders", json!({"range": "B14:C17", "preset": "outside"}));
    ok &= run(s, "home.fillColor", json!({"range": "B14:C17", "color": "#F1F8EA"}));
    ok &= run(s, "review.newNote", json!({"cell": "H4", "text": "Évolution entre le T1 et le T4, pour chaque recette."}));
    ok &= run(s, "selection.set", json!({"cell": "B2"}));
    ok
}

/// Budget familial : le prévu et le réel du mois par poste, l'écart, la part utilisée et un
/// graphique.
fn budget(s: &mut Session) -> bool {
    let rows = json!([
        ["Poste", "Prévu", "Réel", "Écart", "% utilisé"],
        ["Loyer", 650, 650],
        ["Courses", 420, 463.8],
        ["Électricité et chauffage", 120, 134.2],
        ["Transports", 90, 76.5],
        ["Téléphone et Internet", 45, 45],
        ["Assurances", 85, 85],
        ["Santé et mutuelle", 60, 48.3],
        ["Loisirs et sorties", 100, 127.9],
        ["Épargne", 150, 150],
        ["Cotisations et dons", 30, 30],
    ]);
    let mut ok = run(s, "range.setValues", json!({"range": "B2", "values": [["Budget du mois — novembre 2026"]]}));
    ok &= run(s, "home.cellStyle", json!({"range": "B2", "name": "Title"}));
    ok &= run(s, "range.setValues", json!({"range": "B4", "values": rows}));
    for r in 5..=14 {
        ok &= run(s, "cell.set", json!({"cell": format!("E{r}"), "input": format!("=C{r}-D{r}")}));
        ok &= run(s, "cell.set", json!({"cell": format!("F{r}"), "input": format!("=D{r}/C{r}")}));
    }
    ok &= run(s, "range.setValues", json!({"range": "B15", "values": [["Total"]]}));
    for c in ["C", "D", "E"] {
        ok &= run(s, "cell.set", json!({"cell": format!("{c}15"), "input": format!("=SUM({c}5:{c}14)")}));
    }
    ok &= run(s, "cell.set", json!({"cell": "F15", "input": "=D15/C15"}));
    ok &= run(s, "home.cellStyle", json!({"range": "B4:F4", "name": "Heading 3"}));
    ok &= run(s, "home.cellStyle", json!({"range": "B15:F15", "name": "Total"}));
    ok &= run(s, "home.numberFormat", json!({"range": "C5:E15", "code": EUROS}));
    ok &= run(s, "home.numberFormat", json!({"range": "F5:F15", "code": "0%"}));
    ok &= run(
        s,
        "home.conditionalFormat",
        json!({"range": "E5:E14", "rule": {"type": "cellIs", "operator": "less", "value": 0, "preset": "lightRedFillDarkRedText"}}),
    );
    ok &= run(s, "home.conditionalFormat", json!({"range": "F5:F14", "rule": {"type": "colorScale", "colors": ["#63BE7B", "#FFEB84", "#F8696B"]}}));
    ok &= run(s, "home.columnWidth", json!({"cols": "B:B", "width": 170}));
    ok &= run(s, "home.columnWidth", json!({"cols": "C:E", "width": 92}));
    ok &= run(s, "selection.set", json!({"range": "B4:D14"}));
    ok &= run(s, "insert.chart", json!({"type": "bar", "title": "Prévu et réel", "at": "H3", "width": 440, "height": 300}));
    ok &= run(s, "view.freezePanes", json!({"cell": "A5"}));
    ok &= run(s, "selection.set", json!({"cell": "B2"}));
    ok
}

/// Suivi des adhérents : la cotisation de chaque adhérent, ce qu'il a réglé, le reste dû et
/// s'il est à jour ou à relancer. Noms inventés.
fn members(s: &mut Session) -> bool {
    let rows = json!([
        ["Adhérent", "Commune", "Adhésion", "Cotisation", "Réglé", "Reste dû", "Statut"],
        ["Annaïg Le Floc'h", "Quimper", "1/12/2026", 25, 25],
        ["Yannick Kerdraon", "Brest", "2/3/2026", 25, 10],
        ["Maëlle Tanguy", "Morlaix", "2/20/2026", 15, 15],
        ["Erwan Le Gourrierec", "Concarneau", "3/8/2026", 25, 0],
        ["Soizic Pennec", "Quimperlé", "4/14/2026", 25, 25],
        ["Loïc Morvezen", "Landerneau", "5/2/2026", 40, 40],
        ["Gwenola Riou", "Douarnenez", "6/21/2026", 15, 0],
        ["Tugdual Cariou", "Carhaix-Plouguer", "9/9/2026", 25, 25],
    ]);
    let mut ok = true;
    // Les dates en texte de saisie, pour qu'elles deviennent des dates.
    for (i, row) in rows.as_array().into_iter().flatten().enumerate() {
        let r = i + 1;
        for (c, v) in row.as_array().into_iter().flatten().enumerate() {
            let col = ["A", "B", "C", "D", "E", "F", "G"][c];
            let input = match v {
                serde_json::Value::String(t) => t.clone(),
                other => other.to_string(),
            };
            ok &= run(s, "cell.set", json!({"cell": format!("{col}{r}"), "input": input}));
        }
    }
    for r in 2..=9 {
        ok &= run(s, "cell.set", json!({"cell": format!("F{r}"), "input": format!("=D{r}-E{r}")}));
        ok &= run(s, "cell.set", json!({"cell": format!("G{r}"), "input": format!("=IF(F{r}<=0,\"À jour\",\"À relancer\")")}));
    }
    ok &= run(s, "home.numberFormat", json!({"range": "C2:C9", "code": "dd/mm/yyyy"}));
    ok &= run(s, "home.numberFormat", json!({"range": "D2:F9", "code": EUROS}));
    ok &= run(s, "insert.table", json!({"range": "A1:G9", "style": "TableStyleLight11", "name": "Adherents"}));
    ok &= run(s, "home.columnWidth", json!({"cols": "A:A", "width": 150}));
    ok &= run(s, "home.columnWidth", json!({"cols": "B:B", "width": 120}));
    ok &= run(s, "home.columnWidth", json!({"cols": "C:G", "width": 90}));
    ok &= run(
        s,
        "home.conditionalFormat",
        json!({"range": "G2:G9", "rule": {"type": "cellIs", "operator": "equal", "value": "À relancer", "preset": "lightRedFillDarkRedText"}}),
    );
    ok &= run(
        s,
        "home.conditionalFormat",
        json!({"range": "G2:G9", "rule": {"type": "cellIs", "operator": "equal", "value": "À jour", "preset": "greenFillDarkGreenText"}}),
    );
    ok &= run(s, "home.conditionalFormat", json!({"range": "E2:E9", "rule": {"type": "dataBar", "color": "#7BC143"}}));
    ok
}

/// A blank sheet (for callers that need one without a session).
pub fn blank_sheet(name: &str) -> Sheet {
    Sheet::new(name)
}
