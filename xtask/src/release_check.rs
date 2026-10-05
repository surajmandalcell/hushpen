//! `cargo xtask release-check`: proves a release build has no test hook.
//!
//! Plain run: release feature tree, release build, binary scan.
//! `--binary <path>`: scan one existing binary (proves the scan is not vacuous
//! when pointed at a debug `test-automation` build).
//! `--guard`: also prove `--release --features test-automation` is refused.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Crates and features that must not appear in the release feature tree.
const FORBIDDEN_TREE: &[&str] = &["hushpen-testhook", "update-test-feed"];

/// Byte strings that only a hook build contains (symbols, socket name, env var).
const HOOK_MARKERS: &[&str] = &[
    "hushpen_testhook",
    "testhook",
    "hookctl",
    "hook.sock",
    "HUSHPEN_TESTHOOK",
];

/// Engine symbols that must stay in their own binary.
const WHISPER_MARKER: &str = "whisper_full";
const LLAMA_MARKER: &str = "llama_decode";

const APP_BINARY: &str = "hushpen";
const LLM_BINARY: &str = "hushpen-llm";

const GUARD_MESSAGE: &str = "the test hook is for debug builds only";

#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub marker: String,
    pub count: usize,
    pub sample: String,
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    binary: Option<PathBuf>,
    guard: bool,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        binary: None,
        guard: false,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--binary" => {
                let path = iter.next().ok_or("--binary needs a path")?;
                options.binary = Some(PathBuf::from(path));
            }
            "--guard" => options.guard = true,
            other => return Err(format!("unknown release-check argument {other:?}")),
        }
    }
    Ok(options)
}

/// Lists the forbidden crates and features present in `cargo tree -e features`
/// output. A feature line reads `hushpen-app feature "update-test-feed"`; a
/// crate line starts with the crate name.
pub fn tree_violations(tree: &str) -> Vec<String> {
    FORBIDDEN_TREE
        .iter()
        .filter(|forbidden| {
            tree.lines().any(|line| {
                line.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
                    .any(|word| word == **forbidden)
            })
        })
        .map(|forbidden| (*forbidden).to_string())
        .collect()
}

fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .map(|(at, _)| at)
        .collect()
}

/// The printable run of bytes around `at`, so the report names the symbol.
fn printable_run(bytes: &[u8], at: usize, len: usize) -> String {
    let printable = |b: &u8| b.is_ascii_graphic();
    let mut start = at;
    while start > 0 && at - start < 60 && printable(&bytes[start - 1]) {
        start -= 1;
    }
    let mut end = at + len;
    while end < bytes.len() && end - at < 100 && printable(&bytes[end]) {
        end += 1;
    }
    String::from_utf8_lossy(&bytes[start..end]).into_owned()
}

pub fn scan_bytes(bytes: &[u8], markers: &[&str]) -> Vec<Finding> {
    markers
        .iter()
        .filter_map(|marker| {
            let hits = find_all(bytes, marker.as_bytes());
            let first = *hits.first()?;
            Some(Finding {
                marker: (*marker).to_string(),
                count: hits.len(),
                sample: printable_run(bytes, first, marker.len()),
            })
        })
        .collect()
}

/// Markers a binary named `name` must not contain.
fn markers_for(name: &str) -> Vec<&'static str> {
    let mut markers = HOOK_MARKERS.to_vec();
    markers.push(if name == LLM_BINARY {
        WHISPER_MARKER
    } else {
        LLAMA_MARKER
    });
    markers
}

fn scan_file(path: &Path) -> Result<Vec<Finding>, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(scan_bytes(&bytes, &markers_for(&name)))
}

fn report(path: &Path, findings: &[Finding]) -> bool {
    if findings.is_empty() {
        println!("clean: {}", path.display());
        return true;
    }
    for finding in findings {
        println!(
            "FOUND {} x{} in {} (e.g. {})",
            finding.marker,
            finding.count,
            path.display(),
            finding.sample
        );
    }
    false
}

fn cargo(root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command.current_dir(root).args(args);
    command
}

fn target_dir(root: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => root.join("target"),
    }
}

fn check_tree(root: &Path) -> Result<(), String> {
    let mut violations = Vec::new();
    for package in ["hushpen-app", "hushpen-llm"] {
        let output = cargo(root, &["tree", "-e", "features", "--locked", "-p", package])
            .output()
            .map_err(|e| format!("cannot run cargo tree: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "cargo tree -p {package} failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        for hit in tree_violations(&String::from_utf8_lossy(&output.stdout)) {
            violations.push(format!("{package}: {hit}"));
        }
    }
    if violations.is_empty() {
        println!("clean: release feature tree has no hook crate or test feed");
        Ok(())
    } else {
        Err(format!(
            "release feature tree lists {}",
            violations.join(", ")
        ))
    }
}

fn check_guard(root: &Path) -> Result<(), String> {
    let output = cargo(
        root,
        &[
            "build",
            "--release",
            "--locked",
            "-p",
            "hushpen-app",
            "--features",
            "test-automation",
        ],
    )
    .output()
    .map_err(|e| format!("cannot run cargo build: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        return Err("a release build with test-automation succeeded; the guard is gone".into());
    }
    if !stderr.contains(GUARD_MESSAGE) {
        return Err(format!(
            "the release build with test-automation failed, but not on the guard:\n{stderr}"
        ));
    }
    println!("clean: --release --features test-automation is refused by compile_error!");
    Ok(())
}

pub fn run(root: &Path, args: &[String]) -> Result<(), String> {
    let options = parse_args(args)?;

    if let Some(binary) = &options.binary {
        let findings = scan_file(binary)?;
        return if report(binary, &findings) {
            Ok(())
        } else {
            Err(format!("{} contains hook code", binary.display()))
        };
    }

    check_tree(root)?;

    let status = cargo(
        root,
        &[
            "build",
            "--release",
            "--locked",
            "-p",
            "hushpen-app",
            "-p",
            "hushpen-llm",
        ],
    )
    .status()
    .map_err(|e| format!("cannot run cargo build: {e}"))?;
    if !status.success() {
        return Err("release build failed".into());
    }

    let release = target_dir(root).join("release");
    let mut clean = true;
    for name in [APP_BINARY, LLM_BINARY] {
        let path = release.join(name);
        if !path.is_file() {
            return Err(format!("release binary {} is missing", path.display()));
        }
        clean &= report(&path, &scan_file(&path)?);
    }
    if !clean {
        return Err("a release binary contains hook or engine symbols it must not".into());
    }

    if options.guard {
        check_guard(root)?;
    }
    println!("release-check passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_with_the_hook_crate_or_the_test_feed_is_a_violation() {
        let tree = "hushpen-app v2.0.0\n├── hushpen-testhook v2.0.0\n└── hushpen-app feature \"update-test-feed\"\n";
        assert_eq!(
            tree_violations(tree),
            ["hushpen-testhook", "update-test-feed"]
        );
    }

    #[test]
    fn a_tree_without_them_is_clean_even_with_similar_names() {
        let tree = "hushpen-app v2.0.0\n├── hushpen-store v2.0.0\n└── hushpen-testhook-docs-not-a-crate-name-here\n";
        assert!(tree_violations(tree).is_empty());
    }

    #[test]
    fn scan_names_each_marker_with_a_count_and_the_surrounding_symbol() {
        let mut bytes = b"\0\0_ZN15hushpen_testhook6server5start17h0123E\0".to_vec();
        bytes.extend_from_slice(b"\0/run/hook.sock\0hook.sock\0");
        let findings = scan_bytes(&bytes, HOOK_MARKERS);
        let markers: Vec<&str> = findings.iter().map(|f| f.marker.as_str()).collect();
        assert_eq!(markers, ["hushpen_testhook", "testhook", "hook.sock"]);
        assert_eq!(findings[2].count, 2);
        assert!(findings[0].sample.contains("_ZN15hushpen_testhook6server"));
    }

    #[test]
    fn scan_of_clean_bytes_finds_nothing() {
        assert!(scan_bytes(b"\0hello\0world\0", HOOK_MARKERS).is_empty());
        assert!(scan_bytes(b"", HOOK_MARKERS).is_empty());
    }

    #[test]
    fn the_app_must_not_carry_llama_and_the_llm_binary_must_not_carry_whisper() {
        assert!(markers_for(APP_BINARY).contains(&LLAMA_MARKER));
        assert!(!markers_for(APP_BINARY).contains(&WHISPER_MARKER));
        assert!(markers_for(LLM_BINARY).contains(&WHISPER_MARKER));
        assert!(!markers_for(LLM_BINARY).contains(&LLAMA_MARKER));
    }

    #[test]
    fn pointing_the_check_at_a_file_with_hook_symbols_fails_and_names_them() {
        let dir = std::env::temp_dir().join(format!("release-check-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let hooked = dir.join("hushpen");
        std::fs::write(
            &hooked,
            b"\0hushpen_testhook::server\0HUSHPEN_TESTHOOK_SOCKET\0",
        )
        .unwrap();
        let clean = dir.join("hushpen-llm");
        std::fs::write(&clean, b"\0nothing to see\0").unwrap();

        let err = run(
            Path::new("/unused"),
            &["--binary".into(), hooked.display().to_string()],
        )
        .unwrap_err();
        assert!(err.contains("contains hook code"));
        let found: Vec<String> = scan_file(&hooked)
            .unwrap()
            .into_iter()
            .map(|f| f.marker)
            .collect();
        assert_eq!(found, ["hushpen_testhook", "testhook", "HUSHPEN_TESTHOOK"]);

        run(
            Path::new("/unused"),
            &["--binary".into(), clean.display().to_string()],
        )
        .unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn arguments_are_parsed_and_unknown_ones_refused() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse_args(&args(&["--guard"])).unwrap(),
            Options {
                binary: None,
                guard: true
            }
        );
        assert_eq!(
            parse_args(&args(&["--binary", "/x/hushpen"]))
                .unwrap()
                .binary,
            Some(PathBuf::from("/x/hushpen"))
        );
        assert!(parse_args(&args(&["--binary"])).is_err());
        assert!(parse_args(&args(&["--bogus"])).is_err());
    }
}
