//! Workflow files must never run on their own: CI minutes are budgeted.

use std::fs;
use std::path::{Path, PathBuf};

fn workflows_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows")
}

/// Trigger names under the top-level `on:` key (block form).
fn triggers(text: &str) -> Vec<String> {
    let mut lines = text.lines();
    assert!(
        lines.any(|line| line.trim_end() == "on:"),
        "workflow has no block-form `on:` key"
    );
    let mut names = Vec::new();
    let mut indent = None;
    for line in lines {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let width = line.len() - line.trim_start().len();
        if width == 0 {
            break;
        }
        let first = *indent.get_or_insert(width);
        if width == first {
            names.push(line.trim().trim_end_matches(':').to_string());
        }
    }
    names
}

fn input_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_inputs = false;
    for line in text.lines() {
        if line.trim_end() == "    inputs:" {
            in_inputs = true;
        } else if in_inputs {
            let width = line.len() - line.trim_start().len();
            if line.trim().is_empty() {
                continue;
            }
            if width <= 4 {
                break;
            }
            if width == 6 {
                names.push(line.trim().trim_end_matches(':').to_string());
            }
        }
    }
    names
}

#[test]
fn every_workflow_but_deploy_docs_is_dispatch_only() {
    let mut checked = 0;
    for entry in fs::read_dir(workflows_dir()).expect("workflows dir") {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with(".yml") || name == "deploy-docs.yml" {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(triggers(&text), ["workflow_dispatch"], "{name}");
        checked += 1;
    }
    assert!(checked >= 1, "ci.yml is missing");
}

#[test]
fn ci_yml_takes_targets_and_suite_inputs() {
    let text = fs::read_to_string(workflows_dir().join("ci.yml")).unwrap();
    assert_eq!(input_names(&text), ["targets", "suite"]);
}

#[test]
fn ci_yml_has_a_gate_job_and_the_three_runner_build_matrix() {
    let text = fs::read_to_string(workflows_dir().join("ci.yml")).unwrap();
    assert!(text.contains("\n  gate:\n"));
    assert!(text.contains("\n  build:\n"));
    for runner in ["macos-15", "ubuntu-22.04", "ubuntu-22.04-arm"] {
        assert!(text.contains(runner), "{runner} missing");
    }
}

#[test]
fn trigger_parser_reads_block_form() {
    let text =
        "name: x\non:\n  workflow_dispatch:\n    inputs:\n      a:\n  push:\njobs:\n  a: {}\n";
    assert_eq!(triggers(text), ["workflow_dispatch", "push"]);
}

#[test]
fn ci_yml_packages_each_target_and_smokes_the_installers() {
    let text = fs::read_to_string(workflows_dir().join("ci.yml")).unwrap();
    assert!(text.contains("\n  package:\n"));
    assert!(text.contains("\n  smoke:\n"));
    assert!(text.contains("- package-smoke"));
    for stock in ["debian:12", "ubuntu:22.04", "fedora:40"] {
        assert!(text.contains(stock), "{stock} install step missing");
    }
    assert!(text.contains("appimage-xvfb.sh"));
    assert!(text.contains("cargo xtask package-verify"));
}

#[test]
fn the_dmg_mount_script_refuses_to_run_off_the_ci_runner() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packaging/smoke/mac.sh");
    let text = fs::read_to_string(script).unwrap();
    assert!(text.contains(r#"[ "${CI:-}" != "true" ]"#));
    assert!(text.contains(r#"[ "${GITHUB_ACTIONS:-}" != "true" ]"#));
}

#[test]
fn ci_yml_has_an_updater_suite_that_checks_the_update_from_a_test_feed() {
    let text = fs::read_to_string(workflows_dir().join("ci.yml")).unwrap();
    assert!(text.contains("- updater"));
    assert!(text.contains("\n  updater-x64:\n"));
    assert!(text.contains("inputs.suite == 'updater'"));
    for step in [
        "--test-feed",
        "harness/update-fixtures.sh",
        "harness/run-suite.sh self-update",
        "update-notice-in-container.sh",
    ] {
        assert!(text.contains(step), "{step} missing");
    }
    for image in ["ubuntu:22.04", "fedora:40"] {
        assert!(text.contains(image), "{image} notice step missing");
    }
}

#[test]
fn the_other_suites_skip_the_updater_job_and_the_updater_skips_theirs() {
    let text = fs::read_to_string(workflows_dir().join("ci.yml")).unwrap();
    assert!(
        text.contains(
            "inputs.suite != 'gate' && inputs.suite != 'e2e' && inputs.suite != 'updater'"
        )
    );
    assert!(text.contains(
        "inputs.suite != 'package' && inputs.suite != 'package-smoke' && inputs.suite != 'updater'"
    ));
}
