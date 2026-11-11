//! The labels reports write into cells (Lusk Tab): Analysis ToolPak, Solver reports, Scenario
//! Summary and Forecast Sheet, in French when the cells show French words (the interface is in
//! French, see `gridcraft_numfmt::set_french_words`), as Excel in French writes them.
//!
//! Only labels are translated: numbers, formulas and the names taken from the user's data stay
//! as they are. With French words off (tests, the command-line tool, another interface
//! language), [`label`] returns its text unchanged.

/// `text` as a report writes it: translated when French words are on.
pub fn label(text: &str) -> String {
    if !gridcraft_numfmt::french_words() {
        return text.to_string();
    }
    french(text)
}

/// A text cell value holding [`label`] of `text`.
pub fn text(text: &str) -> gridcraft_core::Value {
    gridcraft_core::Value::text(label(text).as_str())
}

/// The French of a report label: a fixed label, or one built around a number or a name
/// (`Largest(2)`, `Lower 95.0%`, `Predicted Sales`); anything else unchanged.
fn french(text: &str) -> String {
    if let Some(f) = fixed(text) {
        return f.to_string();
    }
    // Numbers in labels use the French decimal comma: "Niveau de confiance(95,0%)".
    let comma = |s: &str| s.replace('.', ",");
    let prefixes: [(&str, &str); 25] = [
        ("Largest(", "Plus grande valeur("),
        ("Smallest(", "Plus petite valeur("),
        ("Confidence Level(", "Niveau de confiance("),
        ("Lower ", "Limite inférieure pour seuil de confiance = "),
        ("Upper ", "Limite supérieure pour seuil de confiance = "),
        ("X Variable ", "Variable X "),
        ("Predicted ", "Prévisions pour "),
        ("Column ", "Colonne "),
        ("Column", "Colonne"),
        ("Row ", "Ligne "),
        ("Forecast(", "Prévision("),
        ("Lower Confidence Bound(", "Limite de confiance inférieure("),
        ("Upper Confidence Bound(", "Limite de confiance supérieure("),
        ("Worksheet: ", "Feuille de calcul : "),
        ("Result: ", "Résultat : "),
        ("Engine: ", "Moteur : "),
        ("Objective Cell (", "Cellule objectif ("),
        ("Value Of ", "Valeur de "),
        ("Answer Report ", "Rapport des réponses "),
        ("Sensitivity Report ", "Rapport de sensibilité "),
        ("Limits Report ", "Rapport des limites "),
        ("Scenario Summary ", "Synthèse de scénarios "),
        ("Lusk Tab Answer Report", "Lusk Tab Rapport des réponses"),
        ("Lusk Tab Sensitivity Report", "Lusk Tab Rapport de sensibilité"),
        ("Lusk Tab Limits Report", "Lusk Tab Rapport des limites"),
    ];
    // The longest matching prefix wins ("Lower Confidence Bound(" over "Lower ").
    let best = prefixes.iter().filter(|(en, _)| text.starts_with(en)).max_by_key(|(en, _)| en.len());
    if let Some((en, fr)) = best {
        let rest = text.get(en.len()..).unwrap_or("");
        let rest = match *en {
            // The objective's goal is a word or "Value Of n"; the solver's result a sentence.
            "Objective Cell (" | "Result: " | "Engine: " => {
                let inner = rest.strip_suffix(')').unwrap_or(rest);
                let translated = french(inner);
                if rest.ends_with(')') { format!("{translated})") } else { translated }
            }
            "Confidence Level(" | "Lower " | "Upper " => comma(rest),
            _ => rest.to_string(),
        };
        // "Column1" (a default name) needs a digit after the prefix; "Columns" is a fixed label.
        if *en == "Column" && !rest.starts_with(|c: char| c.is_ascii_digit()) {
            return text.to_string();
        }
        return format!("{fr}{rest}");
    }
    // "$B$4=Integer", "$B$5=Binary": a Solver constraint on a variable cell.
    for (en, fr) in [("=Integer", "=Entier"), ("=Binary", "=Binaire")] {
        if let Some(cell) = text.strip_suffix(en) {
            return format!("{cell}{fr}");
        }
    }
    text.to_string()
}

fn fixed(text: &str) -> Option<&'static str> {
    Some(match text {
        // Descriptive Statistics.
        "Mean" => "Moyenne",
        "Standard Error" => "Erreur-type",
        "Median" => "Médiane",
        "Mode" => "Mode",
        "Standard Deviation" => "Écart-type",
        "Sample Variance" => "Variance de l'échantillon",
        "Kurtosis" => "Kurtosis (aplatissement)",
        "Skewness" => "Coefficient d'asymétrie",
        "Range" => "Plage",
        "Minimum" => "Minimum",
        "Maximum" => "Maximum",
        "Sum" => "Somme",
        "Count" => "Nombre d'échantillons",
        // Histogram, Rank and Percentile.
        "More" => "Autre",
        "Bin" => "Classes",
        "Frequency" => "Fréquence",
        "Cumulative %" => "% cumulé",
        "Point" => "Point",
        "Rank" => "Rang",
        "Percent" => "Pourcentage",
        // Regression.
        "SUMMARY OUTPUT" => "RAPPORT DÉTAILLÉ",
        "Regression Statistics" => "Statistiques de la régression",
        "Multiple R" => "Coefficient de détermination multiple",
        "R Square" => "Coefficient de détermination R^2",
        "Adjusted R Square" => "Coefficient de détermination R^2 ajusté",
        "Observations" => "Observations",
        "ANOVA" => "ANALYSE DE VARIANCE",
        "df" => "Degré de liberté",
        "SS" => "Somme des carrés",
        "MS" => "Moyenne des carrés",
        "F" => "F",
        "Significance F" => "Valeur critique de F",
        "Regression" => "Régression",
        "Residual" => "Résidus",
        "Total" => "Total",
        "Coefficients" => "Coefficients",
        "t Stat" => "Statistique t",
        "P-value" => "Probabilité",
        "Intercept" => "Constante",
        "RESIDUAL OUTPUT" => "ANALYSE DES RÉSIDUS",
        "Observation" => "Observation",
        "Residuals" => "Résidus",
        "Standard Residuals" => "Résidus normalisés",
        "PROBABILITY OUTPUT" => "RÉSULTATS DE LA PROBABILITÉ",
        "Percentile" => "Centile",
        // t-Tests, F-Test and z-Test.
        "Variable 1" => "Variable 1",
        "Variable 2" => "Variable 2",
        "Variance" => "Variance",
        "Pearson Correlation" => "Coefficient de corrélation de Pearson",
        "Pooled Variance" => "Variance pondérée",
        "Hypothesized Mean Difference" => "Différence hypothétique des moyennes",
        "P(T<=t) one-tail" => "P(T<=t) unilatéral",
        "t Critical one-tail" => "Valeur critique de t (unilatéral)",
        "P(T<=t) two-tail" => "P(T<=t) bilatéral",
        "t Critical two-tail" => "Valeur critique de t (bilatéral)",
        "P(F<=f) one-tail" => "P(F<=f) unilatéral",
        "F Critical one-tail" => "Valeur critique pour F (unilatéral)",
        "Known Variance" => "Variance connue",
        "P(Z<=z) one-tail" => "P(Z<=z) unilatéral",
        "z Critical one-tail" => "Valeur critique de z (unilatéral)",
        "P(Z<=z) two-tail" => "P(Z<=z) bilatéral",
        "z Critical two-tail" => "Valeur critique de z (bilatéral)",
        // ANOVA.
        "SUMMARY" => "RAPPORT DÉTAILLÉ",
        "Groups" => "Groupes",
        "Average" => "Moyenne",
        "Source of Variation" => "Source des variations",
        "F crit" => "Valeur critique pour F",
        "Between Groups" => "Entre groupes",
        "Within Groups" => "À l'intérieur des groupes",
        "Rows" => "Lignes",
        "Columns" => "Colonnes",
        "Error" => "Erreur",
        "Sample" => "Échantillon",
        "Interaction" => "Interaction",
        "Within" => "À l'intérieur du groupe",
        // Report titles.
        "Anova: Single Factor" => "Analyse de variance : un facteur",
        "Anova: Two-Factor With Replication" => "Analyse de variance : deux facteurs avec répétition d'expérience",
        "Anova: Two-Factor Without Replication" => "Analyse de variance : deux facteurs sans répétition d'expérience",
        "F-Test Two-Sample for Variances" => "Test d'égalité des variances (F-Test)",
        "t-Test: Paired Two Sample for Means" => "Test d'égalité des espérances : observations pairées",
        "t-Test: Two-Sample Assuming Equal Variances" => "Test d'égalité des espérances : deux observations de variances égales",
        "t-Test: Two-Sample Assuming Unequal Variances" => "Test d'égalité des espérances : deux observations de variances différentes",
        "z-Test: Two Sample for Means" => "Test d'égalité des espérances : deux observations de variances connues",
        // Solver reports.
        "Lusk Tab Answer Report" => "Lusk Tab Rapport des réponses",
        "Lusk Tab Sensitivity Report" => "Lusk Tab Rapport de sensibilité",
        "Lusk Tab Limits Report" => "Lusk Tab Rapport des limites",
        "Simplex LP" => "Simplex PL",
        "GRG Nonlinear" => "GRG non linéaire",
        "Evolutionary" => "Évolutionnaire",
        "Max" => "Max",
        "Min" => "Min",
        "Cell" => "Cellule",
        "Name" => "Nom",
        "Original Value" => "Valeur d'origine",
        "Final Value" => "Valeur finale",
        "Variable Cells" => "Cellules variables",
        "Binary" => "Binaire",
        "Integer" => "Entier",
        "Contin" => "Continu",
        "Constraints" => "Contraintes",
        "Cell Value" => "Valeur de la cellule",
        "Formula" => "Formule",
        "Status" => "État",
        "Slack" => "Marge",
        "Binding" => "Lié",
        "Not Binding" => "Non lié",
        "Unbounded" => "Non borné",
        "Objective Cell" => "Cellule objectif",
        "Value" => "Valeur",
        "Variable Cell" => "Cellule variable",
        "Lower Limit" => "Limite inférieure",
        "Objective Result" => "Résultat objectif",
        "Upper Limit" => "Limite supérieure",
        "Reduced Cost" => "Coût réduit",
        "Objective Coefficient" => "Coefficient objectif",
        "Allowable Increase" => "Marge d'augmentation",
        "Allowable Decrease" => "Marge de réduction",
        "Shadow Price" => "Prix fictif",
        "Constraint R.H. Side" => "Contrainte à droite",
        "Reduced Gradient" => "Gradient réduit",
        "Lagrange Multiplier" => "Multiplicateur de Lagrange",
        "Solver found an optimal solution. All constraints are satisfied." => {
            "Le solveur a trouvé une solution optimale. Toutes les contraintes sont satisfaites."
        }
        "Solver converged to a locally optimal solution. All constraints are satisfied." => {
            "Le solveur a convergé vers une solution optimale locale. Toutes les contraintes sont satisfaites."
        }
        "Solver stopped when the best solution stopped improving. All constraints are satisfied." => {
            "Le solveur s'est arrêté quand la meilleure solution a cessé de s'améliorer. Toutes les contraintes sont satisfaites."
        }
        "Solver stopped at its evaluation limit; the best solution found satisfies the constraints." => {
            "Le solveur s'est arrêté à sa limite d'évaluations ; la meilleure solution trouvée satisfait les contraintes."
        }
        "Solver stopped at its evaluation limit without finding a feasible solution." => {
            "Le solveur s'est arrêté à sa limite d'évaluations sans trouver de solution réalisable."
        }
        "Solver could not find a solution that satisfies all constraints." => {
            "Le solveur n'a pas trouvé de solution qui satisfasse toutes les contraintes."
        }
        // PivotTables.
        "Row Labels" => "Étiquettes de lignes",
        "Column Labels" => "Étiquettes de colonnes",
        "Grand Total" => "Total général",
        // Scenario Summary.
        "Scenario Summary" => "Synthèse de scénarios",
        "Current Values:" => "Valeurs actuelles :",
        "Changing Cells:" => "Cellules variables :",
        "Result Cells:" => "Cellules résultantes :",
        "Notes: Current Values column represents values of changing cells at time Scenario Summary Report was created." => {
            "Remarques : la colonne Valeurs actuelles représente les valeurs des cellules variables au moment de la création du rapport de synthèse de scénarios."
        }
        // Forecast Sheet.
        "Timeline" => "Chronologie",
        "Values" => "Valeurs",
        "Statistic" => "Statistique",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_unless_french_words_are_on() {
        assert_eq!(gridcraft_numfmt::with_french_words(false, || label("Mean")), "Mean");
        gridcraft_numfmt::with_french_words(true, || {
            assert_eq!(label("Mean"), "Moyenne");
            assert_eq!(label("Confidence Level(95.0%)"), "Niveau de confiance(95,0%)");
            assert_eq!(label("Lower 95.0%"), "Limite inférieure pour seuil de confiance = 95,0%");
            assert_eq!(label("Lower Confidence Bound(Ventes)"), "Limite de confiance inférieure(Ventes)");
            assert_eq!(label("X Variable 2"), "Variable X 2");
            assert_eq!(label("Column1"), "Colonne1");
            assert_eq!(label("Columns"), "Colonnes");
            assert_eq!(label("Objective Cell (Value Of 10)"), "Cellule objectif (Valeur de 10)");
            assert_eq!(label("$B$4=Integer"), "$B$4=Entier");
            assert_eq!(label("Answer Report 2"), "Rapport des réponses 2");
            // The user's own names are kept.
            assert_eq!(label("Ventes 2026"), "Ventes 2026");
        });
    }

    #[test]
    fn descriptive_statistics_report_in_french() {
        use serde_json::json;
        let mut s = crate::Session::new();
        s.new_workbook();
        s.execute("range.setValues", json!({"range": "A1:A5", "values": [[2], [4], [4], [6], [9]]})).unwrap();
        let cell = |s: &crate::Session, a1: &str| {
            let d = s.doc().unwrap();
            d.wb.active().unwrap().value(gridcraft_core::CellRef::parse(a1).unwrap()).display()
        };
        gridcraft_numfmt::with_french_words(true, || {
            s.execute("data.analysisToolPak", json!({"tool": "descriptive", "range": "A1:A5", "output": "C1", "confidence": 95})).unwrap();
        });
        assert_eq!(cell(&s, "C1"), "Colonne1");
        assert_eq!(cell(&s, "C3"), "Moyenne");
        assert_eq!(cell(&s, "C4"), "Erreur-type");
    }
}
