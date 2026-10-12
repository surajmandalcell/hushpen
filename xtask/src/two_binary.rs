//! `cargo xtask two-binary`: whisper.cpp and llama.cpp each bring their own ggml, and two ggml
//! copies in one binary link without an error while one silently replaces the other. So the
//! speech engine lives only in the `hushpen` binary (package `hushpen-app`) and the language
//! model only in `hushpen-llm`. This reads `cargo tree` for both packages and fails when a
//! sys crate crosses over, or when the expected one is missing (the check would pass for the
//! wrong reason).

use std::path::Path;
use std::process::Command;

const APP: &str = "hushpen-app";
const LLM: &str = "hushpen-llm";

/// A crate that must (or must not) show up in the tree of a package.
struct Rule {
    package: &'static str,
    present: &'static [&'static str],
    absent: &'static [&'static str],
}

const RULES: [Rule; 2] = [
    Rule {
        package: APP,
        present: &["whisper-rs-sys"],
        absent: &["llama-cpp-sys-2", "llama-cpp-2", LLM],
    },
    Rule {
        package: LLM,
        present: &["llama-cpp-sys-2"],
        absent: &["whisper-rs-sys", "whisper-rs", "hushpen-engine", APP],
    },
];

/// The crate names in `cargo tree --prefix none --format "{p}"` output.
fn crate_names(tree: &str) -> Vec<&str> {
    let mut names: Vec<&str> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// The problems of one package's tree against its rule. Empty when the tree is right.
fn problems(rule: &Rule, tree: &str) -> Vec<String> {
    let names = crate_names(tree);
    let mut found = Vec::new();
    for needed in rule.present {
        if !names.contains(needed) {
            found.push(format!(
                "{} must depend on {needed}, but the tree has no such crate",
                rule.package
            ));
        }
    }
    for forbidden in rule.absent {
        if names.contains(forbidden) {
            found.push(format!(
                "{} must not depend on {forbidden}: whisper.cpp and llama.cpp never share a binary",
                rule.package
            ));
        }
    }
    found
}

fn tree_of(cargo: &str, root: &Path, package: &str) -> Result<String, String> {
    let output = Command::new(cargo)
        .args([
            "tree",
            "--locked",
            "-p",
            package,
            "-e",
            "normal,build",
            "--target",
            "all",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start {cargo}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo tree -p {package}` failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn run(root: &Path) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut found = Vec::new();
    for rule in &RULES {
        found.extend(problems(rule, &tree_of(&cargo, root, rule.package)?));
    }
    if found.is_empty() {
        println!("two-binary: whisper-rs-sys only in {APP}, llama-cpp-sys-2 only in {LLM}");
        Ok(())
    } else {
        Err(format!(
            "two-binary check failed:\n  {}",
            found.join("\n  ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP_TREE: &str = "hushpen-app v2.0.0 (/r/crates/hushpen-app)\n\
        hushpen-engine v2.0.0 (/r/crates/hushpen-engine)\n\
        whisper-rs v0.16.0\n\
        whisper-rs-sys v0.15.0 (/r/vendor/whisper-rs-sys)\n\
        hushpen-core v2.0.0 (/r/crates/hushpen-core) (*)\n";
    const LLM_TREE: &str = "hushpen-llm v2.0.0 (/r/crates/hushpen-llm)\n\
        hushpen-core v2.0.0 (/r/crates/hushpen-core)\n\
        llama-cpp-2 v0.1.158\n\
        llama-cpp-sys-2 v0.1.158\n";

    #[test]
    fn the_expected_trees_pass() {
        assert!(problems(&RULES[0], APP_TREE).is_empty());
        assert!(problems(&RULES[1], LLM_TREE).is_empty());
    }

    #[test]
    fn llama_in_the_app_tree_fails() {
        let tree = format!("{APP_TREE}llama-cpp-2 v0.1.158\nllama-cpp-sys-2 v0.1.158\n");
        let found = problems(&RULES[0], &tree);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.iter().all(|line| line.contains("llama-cpp")));
    }

    #[test]
    fn whisper_in_the_llm_tree_fails() {
        let tree = format!("{LLM_TREE}whisper-rs-sys v0.15.0\n");
        let found = problems(&RULES[1], &tree);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("whisper-rs-sys"));
    }

    #[test]
    fn the_llm_worker_may_not_pull_in_the_engine_crate() {
        let tree = format!("{LLM_TREE}hushpen-engine v2.0.0 (/r/crates/hushpen-engine)\n");
        assert_eq!(problems(&RULES[1], &tree).len(), 1);
    }

    #[test]
    fn a_tree_without_its_own_sys_crate_fails_so_the_check_cannot_pass_vacuously() {
        let found = problems(&RULES[1], "hushpen-llm v2.0.0 (/r)\n");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("must depend on llama-cpp-sys-2"));
    }

    #[test]
    fn crate_names_ignore_versions_paths_and_repeat_markers() {
        assert_eq!(crate_names("a v1 (/x)\nb v2\na v1 (/x) (*)\n"), ["a", "b"]);
    }
}
