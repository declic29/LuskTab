use gridcraft_formula::{FormulaLocale, parse, parse_input, print, print_input};

fn input(src: &str) -> String {
    print(&parse_input(src, FormulaLocale::Fr, |_| false).unwrap())
}

fn shown(english: &str) -> String {
    print_input(&parse(english).unwrap(), FormulaLocale::Fr, |_| false).unwrap()
}

#[test]
fn french_names_and_separators_like_excel_in_french() {
    for (src, expected) in [
        ("=somme(A1:A10)", "SUM(A1:A10)"),
        ("MOYENNE(A1:A10)", "AVERAGE(A1:A10)"),
        ("SI(A1>0;\"Oui\";\"Non\")", "IF(A1>0,\"Oui\",\"Non\")"),
        ("NB.SI(A1:A10;\">0\")", "COUNTIF(A1:A10,\">0\")"),
        ("NB.SI.ENS(A1:A10;\">0\";B1:B10;\"x\")", "COUNTIFS(A1:A10,\">0\",B1:B10,\"x\")"),
        ("SOMME.SI.ENS(C1:C9;A1:A9;\"Brest\")", "SUMIFS(C1:C9,A1:A9,\"Brest\")"),
        ("RECHERCHEV(A2;D:E;2;FAUX)", "VLOOKUP(A2,D:E,2,FALSE)"),
        ("RECHERCHEX(A2;D:D;E:E)", "XLOOKUP(A2,D:D,E:E)"),
        ("INDEX(B:B;EQUIV(A2;D:D;0))", "INDEX(B:B,MATCH(A2,D:D,0))"),
        ("AUJOURDHUI()", "TODAY()"),
        ("ARRONDI(A1*1,2;2)", "ROUND(A1*1.2,2)"),
        ("VPM(0,04/12;240;-150000)", "PMT(0.04/12,240,-150000)"),
        ("SIERREUR(A1/B1;0)", "IFERROR(A1/B1,0)"),
        ("CONCATENER(\"a\";\"b\")", "CONCATENATE(\"a\",\"b\")"),
        ("NBCAR(SUPPRESPACE(A1))", "LEN(TRIM(A1))"),
        ("JOURSEM(A1;2)", "WEEKDAY(A1,2)"),
        ("ECARTYPE.STANDARD(A1:A9)", "STDEV.S(A1:A9)"),
        ("MAX.SI.ENS(B:B;A:A;\"x\")", "MAXIFS(B:B,A:A,\"x\")"),
        ("=vrai", "TRUE"),
        ("=FAUX", "FALSE"),
        ("VRAI()", "TRUE()"),
        ("ET(VRAI;NON(FAUX))", "AND(TRUE,NOT(FALSE))"),
        ("LAMBDA(x;x+1)(2)", "LAMBDA(x,x+1)(2)"),
    ] {
        assert_eq!(input(src), expected, "{src}");
    }
}

#[test]
fn english_names_still_work_in_french() {
    // Except where Excel in French gives the name to another function: TRIM is MIRR there.
    assert_eq!(input("SUM(1;2)"), "SUM(1,2)");
    assert_eq!(input("VLOOKUP(A2;D:E;2;FALSE)"), "VLOOKUP(A2,D:E,2,FALSE)");
    assert_eq!(input("TRIM(A1:A5;0,1;0,12)"), "MIRR(A1:A5,0.1,0.12)");
}

#[test]
fn french_display_of_stored_formulas() {
    assert_eq!(shown("IF(A1>0,SUM(B1:B3),AVERAGE(C1:C3))"), "SI(A1>0;SOMME(B1:B3);MOYENNE(C1:C3))");
    assert_eq!(shown("ROUND(1.5*A1,2)+TRUE"), "ARRONDI(1,5*A1;2)+VRAI");
    assert_eq!(shown("VLOOKUP(A2,D:E,2,FALSE)"), "RECHERCHEV(A2;D:E;2;FAUX)");
    // Functions whose French name is the English one stay as they are.
    assert_eq!(shown("ABS(DATE(2026,11,11)-INDEX(A:A,1))"), "ABS(DATE(2026;11;11)-INDEX(A:A;1))");
    // Text keeps its commas, semicolons and points.
    assert_eq!(shown(r#"CONCAT("1.5",",",";")"#), r#"CONCAT("1.5";",";";")"#);
}

#[test]
fn french_array_constants_use_points_between_columns() {
    assert_eq!(input("SOMME({1.2;3.4})"), "SUM({1,2;3,4})");
    assert_eq!(input("SOMME({1,5.2,25})"), "SUM({1.5,2.25})");
    assert_eq!(input("{VRAI.FAUX}"), "{TRUE,FALSE}");
    assert_eq!(shown("SUM({1.5,2;3,4})"), "SOMME({1,5.2;3.4})");
}

#[test]
fn french_round_trip_keeps_every_formula() {
    for english in [
        "IF(AND(A1>0,B1<>\"\"),SUMIFS(C:C,A:A,A1),NA())",
        "LET(x,SUM(A1:A3),y,x*1.5,ROUND(y,2))",
        "SUM({1.5,2;3,4})+SUM((A1:A3,C1:C3))",
        "TEXTJOIN(\", \",TRUE,FILTER(A:A,B:B>0))",
        "'Feuil 1'!A1+Feuil2!B2",
    ] {
        let fr = shown(english);
        assert_eq!(print(&parse_input(&fr, FormulaLocale::Fr, |_| false).unwrap()), print(&parse(english).unwrap()), "{english} → {fr}");
    }
}

#[test]
fn defined_names_win_over_french_aliases() {
    let known = |name: &str| ["SOMME", "VRAI"].iter().any(|n| n.eq_ignore_ascii_case(name));
    let expr = parse_input("SOMME(2;3)+VRAI+SI(FAUX;4;5)", FormulaLocale::Fr, known).unwrap();
    assert_eq!(print(&expr), "SOMME(2,3)+VRAI+IF(FALSE,4,5)");
}
