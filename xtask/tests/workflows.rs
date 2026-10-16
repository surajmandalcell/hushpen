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
    for image in ["ubuntu:24.04", "fedora:40"] {
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

fn release_yml() -> String {
    fs::read_to_string(workflows_dir().join("release.yml")).expect("release.yml")
}

/// The lines of one top-level job, from `  <name>:` to the next job.
fn job(text: &str, name: &str) -> String {
    let start = format!("\n  {name}:\n");
    let from = text
        .find(&start)
        .unwrap_or_else(|| panic!("job {name} missing"))
        + 1;
    let rest = &text[from..];
    let end = rest[start.len() - 1..]
        .lines()
        .scan(0, |offset, line| {
            let at = *offset;
            *offset += line.len() + 1;
            Some((at, line))
        })
        .find(|(_, line)| line.starts_with("  ") && !line.starts_with("   ") && line.ends_with(':'))
        .map_or(rest.len(), |(at, _)| start.len() - 1 + at);
    rest[..end].to_string()
}

#[test]
fn release_yml_is_dispatch_only_with_one_required_tag_input() {
    let text = release_yml();
    assert_eq!(triggers(&text), ["workflow_dispatch"]);
    assert_eq!(input_names(&text), ["tag"]);
    let tag = text.split("      tag:\n").nth(1).expect("tag input");
    let tag: Vec<&str> = tag
        .lines()
        .take_while(|l| l.starts_with("        "))
        .collect();
    assert!(
        tag.contains(&"        required: true"),
        "tag must be required: {tag:?}"
    );
}

#[test]
fn release_yml_checks_out_the_tag_ref_and_never_a_branch() {
    let text = release_yml();
    let checkouts = text.matches("uses: actions/checkout@").count();
    assert_eq!(
        checkouts, 4,
        "plan, package, release, and verify each check out the tag"
    );
    assert_eq!(
        text.matches("ref: refs/tags/${{ inputs.tag }}").count(),
        checkouts,
        "every checkout names the tag ref"
    );
    for branch in [
        "refs/heads",
        "github.ref_name",
        "github.head_ref",
        "github.sha",
        "ref: master",
        "ref: main",
    ] {
        assert!(!text.contains(branch), "{branch} would build a branch");
    }
}

#[test]
fn release_yml_builds_linux_packages_on_the_22_04_runners_and_never_in_a_container() {
    let text = release_yml();
    let package = job(&text, "package");
    for target in [
        "target: macos-arm64\n            os: macos-15",
        "target: linux-x64\n            os: ubuntu-22.04\n",
        "target: linux-arm64\n            os: ubuntu-22.04-arm",
    ] {
        assert!(
            package.contains(target),
            "{target} missing from the package matrix"
        );
    }
    assert!(
        !text.contains("container:"),
        "a container would change the glibc baseline"
    );
    assert!(
        !text.contains("hushpen-val"),
        "the harness image has glibc 2.39"
    );
    assert!(!text.contains("ubuntu-24.04-arm") && !text.contains("ubuntu-latest"));
    assert!(package.contains("HUSHPEN_BUILD_VERSION: ${{ needs.plan.outputs.version }}"));
    assert!(package.contains("cargo xtask package --target ${{ matrix.target }}"));
    assert!(package.contains("cargo xtask package-verify"));
}

#[test]
fn release_yml_signs_with_the_secret_key_in_the_release_job_only() {
    let text = release_yml();
    let release = job(&text, "release");
    assert!(release.contains("HUSHPEN_UPDATE_KEY: ${{ secrets.HUSHPEN_UPDATE_KEY }}"));
    assert!(release.contains("cargo xtask release-manifest"));
    assert!(release.contains("--pubkey assets/update-key.pub"));
    // The plan job reads the secret only to fail early when it is unset.
    assert_eq!(text.matches("secrets.HUSHPEN_UPDATE_KEY }}").count(), 2);
    assert_eq!(
        text.matches("secrets.HUSHPEN_UPDATE_KEY_PASSWORD").count(),
        1
    );
    for line in text.lines() {
        let line = line.trim();
        for echoing in [
            "echo \"$HUSHPEN_UPDATE_KEY",
            "echo $HUSHPEN_UPDATE_KEY",
            "printenv",
            "env |",
            "set -x",
            "::debug",
        ] {
            assert!(!line.contains(echoing), "{line}");
        }
    }
    assert!(
        !job(&text, "verify").contains("secrets."),
        "verify needs no secret"
    );
    assert!(
        !job(&text, "package").contains("secrets."),
        "builds need no secret"
    );
}

#[test]
fn release_yml_makes_a_draft_and_never_publishes() {
    let text = release_yml();
    let release = job(&text, "release");
    assert!(release.contains("--draft"));
    assert!(release.contains("--verify-tag"));
    assert!(release.contains("gh release create \"$TAG\" release/*"));
    assert!(!text.contains("--draft=false") && !text.contains("gh release edit"));
    assert!(
        release.contains("is already published"),
        "a published release is never replaced"
    );
}

#[test]
fn release_yml_verifies_the_draft_after_it_exists() {
    let text = release_yml();
    let verify = job(&text, "verify");
    assert!(verify.contains("needs: [plan, release]"));
    for step in [
        "packaging/release/download-draft.sh",
        "cargo xtask release-verify",
        "minisign -V -p assets/update-key.pub",
    ] {
        assert!(verify.contains(step), "{step} missing from the verify job");
    }
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../packaging/release/download-draft.sh");
    let script = fs::read_to_string(script).unwrap();
    assert!(
        script.contains("is published, not a draft"),
        "the draft check is in the script"
    );
}

#[test]
fn the_electron_release_workflow_is_gone_and_the_workflow_set_is_the_plan() {
    let mut names: Vec<String> = fs::read_dir(workflows_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["ci.yml", "deploy-docs.yml", "release.yml"]);
}

#[test]
fn job_splitter_stops_at_the_next_job() {
    let text = "jobs:\n  a:\n    steps: x\n  b:\n    steps: y\n";
    assert_eq!(job(text, "a"), "  a:\n    steps: x\n");
    assert_eq!(job(text, "b"), "  b:\n    steps: y\n");
}
