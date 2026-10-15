//! `cargo xtask licenses`: writes `assets/THIRD-PARTY-NOTICES.html`, the page every package ships
//! and the About row opens. Parts: the sections for code, fonts, and models that cargo-about
//! cannot see (texts under `packaging/licenses/manual/`), then the cargo-about listing of every
//! crate. cargo-about runs offline, so the page depends on the lock file and the local crate
//! sources only.

use std::fs;
use std::path::Path;
use std::process::Command;

const ABOUT_CONFIG: &str = "packaging/licenses/about.toml";
const ABOUT_TEMPLATE: &str = "packaging/licenses/about.hbs";
const OUTPUT: &str = "assets/THIRD-PARTY-NOTICES.html";

pub struct Section {
    pub id: &'static str,
    pub title: &'static str,
    /// Plain text under the heading: who made it, the license name, where it is used.
    pub note: &'static str,
    /// License texts, relative to the repo root.
    pub texts: &'static [&'static str],
}

const MIT_GGML: &str = "packaging/licenses/manual/mit-ggml.txt";
const APACHE: &str = "packaging/licenses/manual/apache-2.0.txt";

pub const SECTIONS: &[Section] = &[
    Section {
        id: "whisper-cpp",
        title: "whisper.cpp",
        note: "Speech recognition engine, built into Hushpen. MIT license. https://github.com/ggml-org/whisper.cpp",
        texts: &[MIT_GGML],
    },
    Section {
        id: "llama-cpp",
        title: "llama.cpp",
        note: "Text cleanup engine, built into the hushpen-llm worker. MIT license. https://github.com/ggml-org/llama.cpp",
        texts: &[MIT_GGML],
    },
    Section {
        id: "ggml",
        title: "ggml",
        note: "Tensor library inside whisper.cpp and llama.cpp. MIT license. https://github.com/ggml-org/ggml",
        texts: &[MIT_GGML],
    },
    Section {
        id: "gpui",
        title: "GPUI",
        note: "User interface framework (the gpui-pre crates), Copyright 2022 - 2025 Zed Industries, Inc. Apache License 2.0. https://github.com/zed-industries/zed",
        texts: &[APACHE],
    },
    Section {
        id: "gpui-kit",
        title: "GPUI Kit",
        note: "User interface components (the gpui-kit crates). Apache License 2.0. https://github.com/longbridge/gpui-kit",
        texts: &[APACHE],
    },
    Section {
        id: "symphonia",
        title: "symphonia",
        note: "Audio file decoding (the symphonia crates), used unmodified. Mozilla Public License 2.0. The source is at https://github.com/pdeljanov/Symphonia and on crates.io under the versions in the crate list below.",
        texts: &["packaging/licenses/manual/mpl-2.0.txt"],
    },
    Section {
        id: "sqlite",
        title: "SQLite",
        note: "Database for the history and the dictionary, compiled into Hushpen through rusqlite. Public domain.",
        texts: &["packaging/licenses/manual/sqlite.txt"],
    },
    Section {
        id: "inter",
        title: "Inter",
        note: "The font Hushpen shows on Linux and embeds in every build, Copyright (c) 2016 The Inter Project Authors. SIL Open Font License 1.1. https://github.com/rsms/inter",
        texts: &["assets/fonts/OFL.txt"],
    },
    Section {
        id: "whisper-models",
        title: "Whisper models",
        note: "The speech models Hushpen downloads on request (Tiny, Base, Small, Medium, Large v3 Turbo; the files are the GGML conversions from https://huggingface.co/ggerganov/whisper.cpp). OpenAI Whisper, MIT license.",
        texts: &["packaging/licenses/manual/mit-openai-whisper.txt"],
    },
    Section {
        id: "llm-models",
        title: "LLM models",
        note: "The cleanup models Hushpen downloads on request: Gemma 4 E2B (Google, from https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF) and Qwen3.5 2B (Alibaba Cloud Qwen team, from https://huggingface.co/unsloth/Qwen3.5-2B-GGUF). Both are published under the Apache License 2.0.",
        texts: &[APACHE],
    },
];

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

fn section_html(section: &Section, texts: &[String]) -> String {
    let mut html = format!(
        "<section class=\"manual\" id=\"{}\">\n<h3>{}</h3>\n<p>{}</p>\n",
        section.id,
        escape(section.title),
        escape(section.note),
    );
    for text in texts {
        html.push_str(&format!("<pre>{}</pre>\n", escape(text.trim_end())));
    }
    html.push_str("</section>\n");
    html
}

/// The whole page. `about` is the cargo-about fragment; `texts[i]` belongs to `SECTIONS[i]`.
pub fn assemble(about: &str, texts: &[Vec<String>]) -> String {
    let mut html = String::from(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>Hushpen third-party notices</title>\n\
         <style>\n\
         :root { color-scheme: light dark; }\n\
         body { font: 14px/1.5 system-ui, sans-serif; max-width: 900px; margin: 24px auto; padding: 0 16px; }\n\
         pre { white-space: pre-wrap; font: 12px/1.4 ui-monospace, monospace; }\n\
         section { margin-bottom: 24px; }\n\
         </style>\n</head>\n<body>\n\
         <h1>Hushpen third-party notices</h1>\n\
         <p>Hushpen is free software under the GNU Affero General Public License, version 3 or \
         later. The source is at https://github.com/surajmandalcell/asrpro. Hushpen builds on \
         the software below. Each part keeps its own license.</p>\n\
         <h2>Libraries, fonts, and models</h2>\n",
    );
    for (section, section_texts) in SECTIONS.iter().zip(texts) {
        html.push_str(&section_html(section, section_texts));
    }
    html.push_str("<h2>Rust crates</h2>\n");
    html.push_str(about);
    html.push_str("\n</body>\n</html>\n");
    html
}

fn read_texts(root: &Path) -> Result<Vec<Vec<String>>, String> {
    SECTIONS
        .iter()
        .map(|section| {
            section
                .texts
                .iter()
                .map(|path| {
                    fs::read_to_string(root.join(path))
                        .map_err(|e| format!("cannot read {path} for {}: {e}", section.title))
                })
                .collect()
        })
        .collect()
}

fn cargo_about(root: &Path) -> Result<String, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(root)
        .args(["about", "generate", "--locked", "--offline", "--workspace", "-c"])
        .arg(ABOUT_CONFIG)
        .arg(ABOUT_TEMPLATE)
        .output()
        .map_err(|e| {
            format!(
                "cannot start cargo about: {e}; install it with: cargo install --locked cargo-about --features cli"
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "cargo about failed ({}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("cargo about wrote invalid UTF-8: {e}"))
}

pub fn run(root: &Path) -> Result<(), String> {
    let texts = read_texts(root)?;
    let about = cargo_about(root)?;
    let page = assemble(&about, &texts);
    let out = root.join(OUTPUT);
    fs::write(&out, &page).map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    println!("licenses: wrote {} ({} bytes)", out.display(), page.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn repo() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
    }

    #[test]
    fn escape_covers_the_html_specials() {
        assert_eq!(escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
    }

    #[test]
    fn section_ids_are_unique() {
        let ids: BTreeSet<_> = SECTIONS.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), SECTIONS.len());
        assert_eq!(SECTIONS.len(), 10);
    }

    #[test]
    fn every_section_text_exists_and_is_not_empty() {
        let texts = read_texts(repo()).unwrap();
        for (section, section_texts) in SECTIONS.iter().zip(&texts) {
            assert!(!section_texts.is_empty(), "{} has no text", section.title);
            for text in section_texts {
                assert!(text.trim().len() > 100, "{} has a tiny text", section.title);
            }
        }
    }

    #[test]
    fn the_page_escapes_license_text_and_keeps_the_about_fragment() {
        let texts = vec![vec!["a < b & c".to_string()]; SECTIONS.len()];
        let page = assemble("<ul><li>crate 1.0</li></ul>", &texts);
        assert!(page.contains("<pre>a &lt; b &amp; c</pre>"));
        assert!(page.contains("<ul><li>crate 1.0</li></ul>"));
        for section in SECTIONS {
            assert!(page.contains(&format!("id=\"{}\"", section.id)));
        }
    }

    #[test]
    fn the_inter_section_reads_the_font_license_the_app_embeds() {
        let inter = SECTIONS.iter().find(|s| s.id == "inter").unwrap();
        assert_eq!(inter.texts, ["assets/fonts/OFL.txt"]);
    }
}
