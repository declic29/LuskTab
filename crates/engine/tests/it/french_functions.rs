//! Lusk Tab : chaque fonction de calcul, tapée sous son nom français (SOMME, RECHERCHEV…),
//! est reconnue et calculée ; aucune ne donne #NOM?.

use gridcraft_engine::core::{CellError, CellRef, Value};
use gridcraft_formula::FormulaLocale;
use serde_json::json;

/// Le nom anglais de chaque nom français est une fonction que le moteur connaît, et deux
/// fonctions n'ont jamais le même nom français.
#[test]
fn every_french_name_points_to_a_real_function() {
    let fr = FormulaLocale::Fr.aliases();
    let unknown: Vec<_> = fr.iter().filter(|(en, _)| gridcraft_functions::lookup(en).is_none()).collect();
    assert!(unknown.is_empty(), "French names of unknown functions: {unknown:?}");
    let mut seen = std::collections::HashMap::new();
    for (en, local) in fr {
        if let Some(other) = seen.insert(local.to_ascii_uppercase(), en) {
            panic!("{local} is the French name of both {other} and {en}");
        }
    }
    // As in French Excel, a few French names are another function's English name (TRIM is
    // MIRR, VAR.P is VARP): that function then has a French name of its own (SUPPRESPACE,
    // VAR.P.N), so it stays reachable.
    for (en, local) in fr {
        if !local.eq_ignore_ascii_case(en) && gridcraft_functions::lookup(local).is_some() {
            let own = FormulaLocale::Fr.function_name(local);
            assert!(!own.eq_ignore_ascii_case(local), "{local} (French for {en}) hides the English function {local}");
        }
    }
}

/// Every function, entered in French with its minimum number of arguments, is calculated:
/// the result may be an error for these placeholder arguments, but never #NAME?.
#[test]
fn every_function_typed_in_french_is_calculated() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for spec in gridcraft_functions::all() {
        let name = FormulaLocale::Fr.function_name(spec.name);
        let args = vec!["1"; spec.min_args].join(";");
        let typed = format!("{name}({args})");
        let expr = match gridcraft_formula::parse_input(&typed, FormulaLocale::Fr, |_| false) {
            Ok(e) => e,
            Err(e) => {
                failures.push(format!("{typed}: parse error {}", e.msg));
                continue;
            }
        };
        let english = gridcraft_formula::print(&expr);
        if !english.to_ascii_uppercase().starts_with(&format!("{}(", spec.name)) {
            failures.push(format!("{typed} reads as ={english}, not {}", spec.name));
            continue;
        }
        let mut s = gridcraft_engine::Session::new();
        s.new_workbook();
        if let Err(e) = s.execute("cell.set", json!({"cell": "A1", "input": format!("={english}")})) {
            failures.push(format!("{typed}: {e:?}"));
            continue;
        }
        let d = s.doc().unwrap();
        let v = d.wb.active().unwrap().value(CellRef::new(0, 0));
        if v == Value::Error(CellError::Name) {
            failures.push(format!("{typed} (={english}) gives #NAME?"));
        }
        checked += 1;
    }
    assert!(checked > 400, "only {checked} functions checked");
    assert!(failures.is_empty(), "{} functions fail in French:\n{}", failures.len(), failures.join("\n"));
}
