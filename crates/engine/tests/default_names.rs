//! Lusk Tab: new workbooks and sheets are named in the interface language (`Classeur1`, `Feuil1`
//! in French). Its own test binary, because the base names are process-wide.

use gridcraft_engine::Session;
use gridcraft_engine::model::workbook::set_default_names;

#[test]
fn new_workbooks_and_sheets_take_the_default_names() {
    set_default_names("Feuil", "Classeur");
    let mut s = Session::new();
    s.new_workbook();
    s.new_workbook();
    let titles: Vec<String> = s.documents().iter().map(|d| d.display_title()).collect();
    assert_eq!(titles, ["Classeur1", "Classeur2"]);
    let names: Vec<String> = s.active().unwrap().wb.sheets.iter().map(|sh| sh.name.clone()).collect();
    assert_eq!(names, ["Feuil1"]);
    s.execute("home.insertSheet", serde_json::json!({})).unwrap();
    let names: Vec<String> = s.active().unwrap().wb.sheets.iter().map(|sh| sh.name.clone()).collect();
    assert!(names.contains(&"Feuil2".to_string()), "{names:?}");
    set_default_names("Sheet", "Book");
    let mut s = Session::new();
    s.new_workbook();
    assert_eq!(s.active().unwrap().display_title(), "Book1");
}
