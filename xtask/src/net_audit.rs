//! `cargo xtask net-audit`: lists every network call site and host in
//! `crates/` and fails when one sits outside the allowed places.
//!
//! Rules (architecture section 9): HTTP lives only in `hushpen-app/src/net/`,
//! only `hushpen-app` depends on `ureq`, child crates depend on no HTTP or
//! socket crate, and only allow-listed hosts appear in the code.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const NET_DIR: &str = "crates/hushpen-app/src/net/";
const HOOK_DIR: &str = "crates/hushpen-testhook/src/";
const DECODE_DIR: &str = "crates/hushpen-engine/src/child/";
const AUDIO_DIR: &str = "crates/hushpen-audio/";

const CALL_TOKENS: [&str; 6] = [
    "ureq::",
    "reqwest::",
    "hyper::",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
];
/// Tokens the test hook may use for its loopback listener.
const HOOK_TOKENS: [&str; 2] = ["TcpStream", "TcpListener"];

const HTTP_CRATES: [&str; 9] = [
    "ureq",
    "reqwest",
    "hyper",
    "isahc",
    "curl",
    "attohttpc",
    "minreq",
    "surf",
    "tungstenite",
];
const SOCKET_CRATES: [&str; 3] = ["socket2", "mio", "tokio"];
const CHILD_CRATES: [&str; 2] = ["hushpen-engine", "hushpen-llm"];
const HTTP_OWNER: &str = "hushpen-app";

const PURPOSE_LOOKBACK: usize = 15;

#[derive(Debug, PartialEq, Eq)]
pub struct Site {
    pub path: String,
    pub line: usize,
    pub what: String,
    pub purpose: Option<String>,
    pub host: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Violation {
    pub path: String,
    pub line: usize,
    pub message: String,
}

pub fn host_allowed(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    matches!(
        host.as_str(),
        "huggingface.co"
            | "hf.co"
            | "github.com"
            | "githubusercontent.com"
            | "localhost"
            | "127.0.0.1"
            | "::1"
    ) || host.ends_with(".hf.co")
        || host.ends_with(".githubusercontent.com")
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

/// Hosts of every `http://` and `https://` literal on a line, with the scheme.
fn urls_in(line: &str) -> Vec<(&'static str, String)> {
    let mut found = Vec::new();
    for scheme in ["https://", "http://"] {
        let mut rest = line;
        while let Some(at) = rest.find(scheme) {
            let after = &rest[at + scheme.len()..];
            let authority = after
                .split(['/', '"', '\'', ' ', '?', '#', ')', '>', '`', '\\'])
                .next()
                .unwrap_or("");
            let host = if let Some(inner) = authority.strip_prefix('[') {
                inner.split(']').next().unwrap_or("").to_string()
            } else {
                let without_user = authority.rsplit('@').next().unwrap_or("");
                without_user.split(':').next().unwrap_or("").to_string()
            };
            if !host.is_empty() {
                found.push((
                    if scheme == "https://" {
                        "https"
                    } else {
                        "http"
                    },
                    host,
                ));
            }
            rest = after;
        }
    }
    found
}

fn purpose_near(lines: &[&str], index: usize) -> Option<String> {
    let start = index.saturating_sub(PURPOSE_LOOKBACK);
    lines[start..=index].iter().rev().find_map(|line| {
        let at = line.find("Purpose::")? + "Purpose::".len();
        let name: String = line[at..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        (!name.is_empty()).then_some(name)
    })
}

pub fn scan_source(rel_path: &str, text: &str) -> (Vec<Site>, Vec<Violation>) {
    let mut sites = Vec::new();
    let mut violations = Vec::new();
    let in_net = rel_path.starts_with(NET_DIR);
    let in_hook = rel_path.starts_with(HOOK_DIR);
    let lines: Vec<&str> = text.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        let number = index + 1;
        let mut violate = |message: String| {
            violations.push(Violation {
                path: rel_path.to_string(),
                line: number,
                message,
            });
        };

        for token in CALL_TOKENS {
            if !line.contains(token) {
                continue;
            }
            sites.push(Site {
                path: rel_path.to_string(),
                line: number,
                what: token.to_string(),
                purpose: purpose_near(&lines, index),
                host: None,
            });
            let hook_ok = in_hook && HOOK_TOKENS.contains(&token);
            if !in_net && !hook_ok {
                violate(format!("`{token}` outside {NET_DIR}"));
            }
        }

        for (scheme, host) in urls_in(line) {
            sites.push(Site {
                path: rel_path.to_string(),
                line: number,
                what: format!("{scheme}://"),
                purpose: purpose_near(&lines, index),
                host: Some(host.clone()),
            });
            if host.contains('{') {
                continue;
            }
            if !host_allowed(&host) {
                violate(format!("host `{host}` is not on the allow list"));
            } else if scheme == "http" && !is_loopback(&host.to_ascii_lowercase()) {
                violate(format!(
                    "plain http to `{host}`; only loopback may use http"
                ));
            }
        }

        if line.contains("hushpen_audio::decode")
            && !rel_path.starts_with(DECODE_DIR)
            && !rel_path.starts_with(AUDIO_DIR)
        {
            violate(format!("`hushpen_audio::decode` outside {DECODE_DIR}"));
        }
    }
    (sites, violations)
}

/// Dependency names declared in one member manifest, across all dependency tables.
fn declared_dependencies(manifest: &str) -> Vec<(usize, String)> {
    let mut names = Vec::new();
    let mut in_deps = false;
    for (index, raw) in manifest.lines().enumerate() {
        let line = raw.trim();
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_end_matches(']');
            in_deps = header.ends_with("dependencies");
            if let Some((_, name)) = header.rsplit_once("dependencies.") {
                names.push((index + 1, name.to_string()));
            }
            continue;
        }
        if in_deps && !line.is_empty() && !line.starts_with('#') {
            let key = line.split(['=', '.', ' ']).next().unwrap_or("");
            if !key.is_empty() {
                names.push((index + 1, key.to_string()));
            }
        }
    }
    names
}

pub fn scan_manifest(rel_path: &str, crate_name: &str, text: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (line, name) in declared_dependencies(text) {
        let http = HTTP_CRATES.contains(&name.as_str());
        let socket = SOCKET_CRATES.contains(&name.as_str());
        let message = if http && !(crate_name == HTTP_OWNER && name == "ureq") {
            Some(format!(
                "`{crate_name}` must not depend on HTTP crate `{name}`"
            ))
        } else if socket && CHILD_CRATES.contains(&crate_name) {
            Some(format!(
                "child crate `{crate_name}` must not depend on socket crate `{name}`"
            ))
        } else {
            None
        };
        if let Some(message) = message {
            violations.push(Violation {
                path: rel_path.to_string(),
                line,
                message,
            });
        }
    }
    violations
}

/// Workspace crates that depend directly on `package`, from `cargo tree -i`
/// output printed with `--prefix depth`. Third-party dependents are ignored.
pub fn workspace_dependents(tree_output: &str, package: &str) -> Vec<String> {
    tree_output
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix('1')?;
            if rest.starts_with(|c: char| c.is_ascii_digit()) {
                return None;
            }
            let name = rest.split_whitespace().next()?;
            (rest.contains("(/") && name != package).then(|| name.to_string())
        })
        .collect()
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                collect_files(&path, out)?;
            }
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn ureq_dependent_violations(root: &Path) -> Result<Vec<Violation>, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args([
            "tree",
            "--workspace",
            "--locked",
            "--prefix",
            "depth",
            "-i",
            "ureq",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot run cargo tree: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        // `ureq` is not in the dependency graph yet, which satisfies the rule.
        return if stderr.contains("did not match any packages") {
            Ok(Vec::new())
        } else {
            Err(format!("cargo tree failed: {stderr}"))
        };
    }
    Ok(workspace_dependents(&stdout, "ureq")
        .into_iter()
        .filter(|name| name != HTTP_OWNER)
        .map(|name| Violation {
            path: "Cargo.lock".to_string(),
            line: 0,
            message: format!("workspace crate `{name}` depends on `ureq`; only `{HTTP_OWNER}` may"),
        })
        .collect())
}

pub fn run(root: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    collect_files(&root.join("crates"), &mut files)?;
    files.sort();

    let mut sites = Vec::new();
    let mut violations = Vec::new();
    for file in &files {
        let rel_path = rel(root, file);
        let is_rust = file.extension().is_some_and(|ext| ext == "rs");
        let is_manifest = file.file_name().is_some_and(|name| name == "Cargo.toml");
        if !is_rust && !is_manifest {
            continue;
        }
        let text =
            fs::read_to_string(file).map_err(|error| format!("cannot read {rel_path}: {error}"))?;
        if is_rust {
            let (found, bad) = scan_source(&rel_path, &text);
            sites.extend(found);
            violations.extend(bad);
        } else if let Some(crate_name) = file
            .parent()
            .and_then(|dir| dir.file_name())
            .and_then(|name| name.to_str())
        {
            violations.extend(scan_manifest(&rel_path, crate_name, &text));
        }
    }
    violations.extend(ureq_dependent_violations(root)?);

    println!("net-audit: {} call sites and host references", sites.len());
    for site in &sites {
        println!(
            "  {}:{}  {}  purpose={}  host={}",
            site.path,
            site.line,
            site.what,
            site.purpose.as_deref().unwrap_or("-"),
            site.host.as_deref().unwrap_or("-")
        );
    }
    if violations.is_empty() {
        println!("net-audit: ok");
        return Ok(());
    }
    for violation in &violations {
        eprintln!(
            "  {}:{}  {}",
            violation.path, violation.line, violation.message
        );
    }
    Err(format!("net-audit found {} violation(s)", violations.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violations(path: &str, text: &str) -> Vec<String> {
        scan_source(path, text)
            .1
            .into_iter()
            .map(|v| v.message)
            .collect()
    }

    #[test]
    fn ureq_inside_net_is_a_listed_site_and_not_a_violation() {
        let text = "// Purpose::ModelDownload\nlet resp = ureq::get(\"https://huggingface.co/x\").call();\n";
        let (sites, bad) = scan_source("crates/hushpen-app/src/net/download.rs", text);
        assert!(bad.is_empty(), "{bad:?}");
        let call = sites.iter().find(|s| s.what == "ureq::").unwrap();
        assert_eq!(call.purpose.as_deref(), Some("ModelDownload"));
        let url = sites.iter().find(|s| s.host.is_some()).unwrap();
        assert_eq!(url.host.as_deref(), Some("huggingface.co"));
    }

    #[test]
    fn network_tokens_outside_net_are_violations() {
        for token in [
            "ureq::get(x)",
            "TcpStream::connect(a)",
            "TcpListener::bind(a)",
            "UdpSocket::bind(a)",
        ] {
            let bad = violations("crates/hushpen-app/src/ui/home.rs", token);
            assert_eq!(bad.len(), 1, "{token}: {bad:?}");
        }
    }

    #[test]
    fn the_test_hook_may_listen_on_tcp_but_not_use_ureq() {
        assert!(
            violations(
                "crates/hushpen-testhook/src/server.rs",
                "TcpListener::bind(a)"
            )
            .is_empty()
        );
        assert_eq!(
            violations("crates/hushpen-testhook/src/server.rs", "ureq::get(x)").len(),
            1
        );
    }

    #[test]
    fn comments_are_skipped() {
        assert!(
            violations(
                "crates/hushpen-core/src/lib.rs",
                "// see https://example.com/a and TcpStream"
            )
            .is_empty()
        );
    }

    #[test]
    fn hosts_must_be_on_the_allow_list() {
        let path = "crates/hushpen-app/src/ui/about.rs";
        assert!(
            violations(path, r#"open("https://github.com/surajmandalcell/asrpro")"#).is_empty()
        );
        assert!(violations(path, r#"u("https://us.aws.cdn.hf.co/f")"#).is_empty());
        assert!(violations(path, r#"u("https://objects.githubusercontent.com/f")"#).is_empty());
        assert_eq!(violations(path, r#"u("https://example.com/f")"#).len(), 1);
        assert_eq!(violations(path, r#"u("https://evilhf.co/f")"#).len(), 1);
    }

    #[test]
    fn plain_http_is_allowed_only_for_loopback() {
        let path = "crates/hushpen-app/src/net/endpoint.rs";
        assert!(violations(path, r#"const P: &str = "http://localhost:11434/v1";"#).is_empty());
        assert!(violations(path, r#"const P: &str = "http://127.0.0.1:1234/v1";"#).is_empty());
        assert!(violations(path, r#"const P: &str = "http://[::1]:8080/v1";"#).is_empty());
        assert_eq!(
            violations(path, r#"const P: &str = "http://huggingface.co/x";"#).len(),
            1
        );
    }

    #[test]
    fn audio_decode_is_reachable_only_from_the_engine_child() {
        let line = "use hushpen_audio::decode::Windows;";
        assert!(violations("crates/hushpen-engine/src/child/import.rs", line).is_empty());
        assert_eq!(violations("crates/hushpen-app/src/main.rs", line).len(), 1);
    }

    #[test]
    fn manifests_keep_http_crates_out_of_everything_but_the_app() {
        let manifest = "[dependencies]\nureq.workspace = true\nserde = \"1\"\n";
        assert!(scan_manifest("crates/hushpen-app/Cargo.toml", "hushpen-app", manifest).is_empty());
        assert_eq!(
            scan_manifest("crates/hushpen-store/Cargo.toml", "hushpen-store", manifest).len(),
            1
        );
        let reqwest = "[dependencies]\nreqwest = { version = \"0.13\" }\n";
        assert_eq!(
            scan_manifest("crates/hushpen-app/Cargo.toml", "hushpen-app", reqwest).len(),
            1
        );
        let table = "[dependencies.ureq]\nversion = \"3\"\n";
        assert_eq!(
            scan_manifest("crates/hushpen-core/Cargo.toml", "hushpen-core", table).len(),
            1
        );
    }

    #[test]
    fn child_crates_may_not_use_socket_crates() {
        let manifest = "[target.'cfg(unix)'.dependencies]\nsocket2 = \"0.5\"\n";
        assert_eq!(
            scan_manifest("crates/hushpen-llm/Cargo.toml", "hushpen-llm", manifest).len(),
            1
        );
        assert!(
            scan_manifest(
                "crates/hushpen-platform/Cargo.toml",
                "hushpen-platform",
                manifest
            )
            .is_empty()
        );
    }

    #[test]
    fn a_normal_manifest_has_no_violations() {
        let manifest =
            "[package]\nname = \"hushpen-llm\"\n\n[dependencies]\nhushpen-core.workspace = true\n";
        assert!(scan_manifest("crates/hushpen-llm/Cargo.toml", "hushpen-llm", manifest).is_empty());
    }

    #[test]
    fn tree_output_yields_only_direct_workspace_dependents() {
        let tree = "0ureq v3.4.2\n1hushpen-app v2.0.0 (/repo/crates/hushpen-app)\n2hushpen-core v2.0.0 (/repo/crates/hushpen-core)\n1some-lib v1.0.0\n1hushpen-store v2.0.0 (/repo/crates/hushpen-store)\n";
        assert_eq!(
            workspace_dependents(tree, "ureq"),
            ["hushpen-app", "hushpen-store"]
        );
    }
}
