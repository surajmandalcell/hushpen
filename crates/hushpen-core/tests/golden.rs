//! Runs `tests/golden/rules.json` through the rule cleanup and scores it.

use hushpen_core::cleanup::{Options, clean};
use serde_json::Value;

fn golden() -> Vec<Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/golden/rules.json");
    let text = std::fs::read_to_string(path).expect("golden file");
    let file: Value = serde_json::from_str(&text).expect("golden file is JSON");
    file["cases"].as_array().expect("cases array").clone()
}

#[test]
fn the_golden_set_has_at_least_fifty_cases_with_names() {
    let cases = golden();
    assert!(cases.len() >= 50, "only {} cases", cases.len());
    let mut names: Vec<&str> = cases.iter().map(|c| c["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), cases.len(), "case names must be unique");
}

#[test]
fn the_golden_set_scores_one() {
    let cases = golden();
    let mut failures = Vec::new();
    for case in &cases {
        let input = case["input"].as_str().unwrap();
        let expected = case["expected"].as_str().unwrap();
        let options = Options {
            spoken_punctuation: case["spokenPunctuation"].as_bool().unwrap_or(true),
        };
        let got = clean(input, &options);
        if got != expected {
            failures.push(format!(
                "{}: input {input:?}\n    expected {expected:?}\n    got      {got:?}",
                case["name"].as_str().unwrap()
            ));
        }
    }
    let passed = cases.len() - failures.len();
    let score = passed as f64 / cases.len() as f64;
    println!("golden score {score:.4} ({passed}/{} cases)", cases.len());
    assert!(
        failures.is_empty(),
        "score {score:.4}, {} failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
