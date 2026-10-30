//! `cargo xtask tts-runtime [--target <t>] [--dir <dir>]`: fetches the sherpa-onnx libraries
//! that the `hushpen-tts` worker opens at run time.
//!
//! The worker builds without sherpa-onnx and nothing downloads at build time. The release
//! archives of one pinned version are fetched here, each with its size and sha256, and every
//! library taken out of an archive is checked against its own pinned digest. `cargo xtask
//! package` calls [`ensure`] for its target, so a package can only ship the pinned files.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::package::names::Target;
use crate::package::{copy, run_tool, sha256_hex};

/// The only sherpa-onnx release whose C API layout the worker knows (see its `sherpa` module).
pub const SHERPA_VERSION: &str = "2.0.0";
/// The ONNX Runtime library inside that release, which the Chatterbox voices also run on.
pub const ONNXRUNTIME_VERSION: &str = "1.28.2";
const RELEASE: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.0";

/// One library a package ships, with the digest of the exact file in the release archive.
pub struct Lib {
    pub name: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

/// A pinned release archive: the two libraries the worker needs out of its `lib/` folder.
/// The cxx API library of the same folder is not shipped; nothing calls it.
pub struct Archive {
    pub target: Target,
    pub file: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
    pub libs: [Lib; 2],
}

const fn lib(name: &'static str, bytes: u64, sha256: &'static str) -> Lib {
    Lib {
        name,
        bytes,
        sha256,
    }
}

const ARCHIVES: [Archive; 3] = [
    Archive {
        target: Target::MacosArm64,
        file: "sherpa-onnx-v2.0.0-osx-arm64-shared-lib.tar.bz2",
        bytes: 8_704_036,
        sha256: "a28dfdbf62677f712f29116271880a2552c09b6f06b485e6dad64ea56ad9fffe",
        libs: [
            lib(
                "libsherpa-onnx-c-api.dylib",
                4_095_008,
                "2d4ff0698773133858aabc7aa6c396b6243db8385bb6da8f965ae235311f0d61",
            ),
            Lib {
                name: "libonnxruntime.dylib",
                bytes: 28_775_120,
                sha256: "3567d114f7299d559993e536d605a6f46d7bc9d2542004accc80ee9bf5457f0b",
            },
        ],
    },
    Archive {
        target: Target::LinuxX64,
        file: "sherpa-onnx-v2.0.0-linux-x64-shared-lib.tar.bz2",
        bytes: 9_690_703,
        sha256: "e95f0bb7dbf838d45b0c2831be780b5236fa4c2f891e8bc95b1859dd3311d392",
        libs: [
            lib(
                "libsherpa-onnx-c-api.so",
                4_838_888,
                "c942b1b81a66ab9f247d8f28aac7dd373f81063f008b880136d32d011fa87f4e",
            ),
            Lib {
                name: "libonnxruntime.so",
                bytes: 27_026_609,
                sha256: "4b3607aebd1784b26b6f9b20e4bd974c7ab8287043e4d095cb7d2cb40b5e566e",
            },
        ],
    },
    Archive {
        target: Target::LinuxArm64,
        file: "sherpa-onnx-v2.0.0-linux-aarch64-shared-cpu-lib.tar.bz2",
        bytes: 12_468_072,
        sha256: "833848c29192f137ced0cd9670699e5ad5ad524dead39baa4925c0e1d52275d0",
        libs: [
            lib(
                "libsherpa-onnx-c-api.so",
                4_374_312,
                "323ca4e8094b569b4666c85774e306ba4b973312154ae207f0f04dff4768a178",
            ),
            Lib {
                name: "libonnxruntime.so",
                bytes: 34_606_912,
                sha256: "cfc800d4438f15e8d9017521326776ca7117d8783cb847ccccc2fc61ac4d540f",
            },
        ],
    },
];

pub fn archive_for(target: Target) -> &'static Archive {
    ARCHIVES
        .iter()
        .find(|archive| archive.target == target)
        .expect("every packaging target has a runtime archive")
}

/// The folder the packaging step reads the libraries of `target` from.
pub fn dir(target_dir: &Path, target: Target) -> PathBuf {
    target_dir.join("tts-runtime").join(target.name())
}

/// The library files of one target inside `dir`.
pub fn libs_in(dir: &Path, target: Target) -> Vec<PathBuf> {
    archive_for(target)
        .libs
        .iter()
        .map(|lib| dir.join(lib.name))
        .collect()
}

/// The file at `path` is the pinned library: right size, right sha256.
pub fn matches_pinned(path: &Path, lib: &Lib) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }
    let size = fs::metadata(path)
        .map_err(|e| format!("cannot stat {}: {e}", path.display()))?
        .len();
    Ok(size == lib.bytes && sha256_hex(path)? == lib.sha256)
}

/// The machine code of a library: the ELF `.text` section or the Mach-O `__TEXT,__text`
/// section of a 64-bit little-endian file. Packagers change the rest: the macOS bundle signs
/// every dylib again, and the AppImage tool strips each `.so` and sets its rpath.
pub fn code_section(bytes: &[u8]) -> Option<&[u8]> {
    let u16_at = |at: usize| Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let u64_at = |at: usize| Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?));
    let slice = |offset: u64, size: u64| {
        let start = usize::try_from(offset).ok()?;
        bytes.get(start..start.checked_add(usize::try_from(size).ok()?)?)
    };
    if bytes.starts_with(b"\x7fELF") {
        if bytes.get(4..6)? != [2, 1] {
            return None;
        }
        let headers = usize::try_from(u64_at(0x28)?).ok()?;
        let entry = usize::from(u16_at(0x3A)?);
        let count = usize::from(u16_at(0x3C)?);
        let names_at = headers + usize::from(u16_at(0x3E)?) * entry;
        let names = usize::try_from(u64_at(names_at + 0x18)?).ok()?;
        return (0..count).find_map(|index| {
            let header = headers + index * entry;
            let name = names + usize::try_from(u32_at(header)?).ok()?;
            (bytes.get(name..name + 6)? == b".text\0")
                .then(|| slice(u64_at(header + 0x18)?, u64_at(header + 0x20)?))
                .flatten()
        });
    }
    const MACH_O_64: u32 = 0xfeed_facf;
    const LC_SEGMENT_64: u32 = 0x19;
    if u32_at(0)? != MACH_O_64 {
        return None;
    }
    let field = |at: usize, name: &[u8]| {
        let raw = bytes.get(at..at + 16)?;
        Some(raw.split(|&byte| byte == 0).next()? == name)
    };
    let mut command = 32;
    for _ in 0..u32_at(16)? {
        let size = usize::try_from(u32_at(command + 4)?).ok()?;
        if u32_at(command)? == LC_SEGMENT_64 && field(command + 8, b"__TEXT")? {
            for section in 0..usize::try_from(u32_at(command + 64)?).ok()? {
                let at = command + 72 + section * 80;
                if field(at, b"__text")? {
                    return slice(u64::from(u32_at(at + 48)?), u64_at(at + 40)?);
                }
            }
        }
        command += size;
    }
    None
}

/// The library at `packaged` carries the same machine code as the pinned copy at `pinned`.
pub fn same_code(packaged: &Path, pinned: &Path) -> Result<bool, String> {
    if !packaged.is_file() {
        return Ok(false);
    }
    let read =
        |path: &Path| fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()));
    let (packaged, pinned_bytes) = (read(packaged)?, read(pinned)?);
    let pinned_code = code_section(&pinned_bytes)
        .ok_or_else(|| format!("{} has no code section", pinned.display()))?;
    Ok(code_section(&packaged) == Some(pinned_code))
}

/// Makes sure `dir` holds the pinned libraries of `target`, downloading the archive when one
/// is missing or does not match. Returns `true` when a download ran.
pub fn ensure(dir: &Path, target: Target) -> Result<bool, String> {
    let archive = archive_for(target);
    let mut ready = true;
    for (path, lib) in libs_in(dir, target).iter().zip(&archive.libs) {
        if !matches_pinned(path, lib)? {
            ready = false;
            break;
        }
    }
    if ready {
        return Ok(false);
    }
    fetch(dir, archive)?;
    Ok(true)
}

fn fetch(dir: &Path, archive: &Archive) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let scratch = dir.join("download");
    if scratch.exists() {
        fs::remove_dir_all(&scratch)
            .map_err(|e| format!("cannot clear {}: {e}", scratch.display()))?;
    }
    fs::create_dir_all(&scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;
    let result = fetch_into(dir, &scratch, archive);
    let _ = fs::remove_dir_all(&scratch);
    result
}

fn fetch_into(dir: &Path, scratch: &Path, archive: &Archive) -> Result<(), String> {
    let package = scratch.join(archive.file);
    let url = format!("{RELEASE}/{}", archive.file);
    run_tool(
        Command::new("curl")
            .args(["-fsSL", "--retry", "3", "-o"])
            .arg(&package)
            .arg(&url),
        "curl sherpa-onnx",
    )?;
    let size = fs::metadata(&package)
        .map_err(|e| format!("cannot stat {}: {e}", package.display()))?
        .len();
    let hash = sha256_hex(&package)?;
    if size != archive.bytes || hash != archive.sha256 {
        return Err(format!(
            "{} is not the pinned archive (size {size}, sha256 {hash}); \
             expected {} bytes, {}",
            archive.file, archive.bytes, archive.sha256
        ));
    }
    run_tool(
        Command::new("tar")
            .arg("-xjf")
            .arg(&package)
            .arg("-C")
            .arg(scratch),
        "tar -xjf sherpa-onnx",
    )?;
    let top = archive
        .file
        .strip_suffix(".tar.bz2")
        .ok_or("the archive name lost its .tar.bz2 suffix")?;
    for lib in &archive.libs {
        let extracted = scratch.join(top).join("lib").join(lib.name);
        let to = dir.join(lib.name);
        copy(&extracted, &to)?;
        if !matches_pinned(&to, lib)? {
            return Err(format!(
                "{} in {} is not the pinned library",
                lib.name, archive.file
            ));
        }
    }
    println!(
        "tts-runtime: sherpa-onnx {SHERPA_VERSION} for {} is in {}",
        archive.target.name(),
        dir.display()
    );
    Ok(())
}

struct Options {
    target: Option<Target>,
    dir: Option<PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        target: None,
        dir: None,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| iter.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--target" => options.target = Some(Target::parse(value("--target")?)?),
            "--dir" => options.dir = Some(PathBuf::from(value("--dir")?)),
            other => return Err(format!("unknown tts-runtime argument {other:?}")),
        }
    }
    Ok(options)
}

pub fn run(root: &Path, args: &[String]) -> Result<(), String> {
    let options = parse_args(args)?;
    let target = match options.target {
        Some(target) => target,
        None => Target::host()?,
    };
    let dir = options
        .dir
        .unwrap_or_else(|| dir(&crate::package::target_dir(root), target));
    ensure(&dir, target)?;
    for path in libs_in(&dir, target) {
        println!("{}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_packaging_target_has_an_archive_of_two_libraries() {
        for target in [Target::MacosArm64, Target::LinuxX64, Target::LinuxArm64] {
            let archive = archive_for(target);
            assert!(archive.file.ends_with(".tar.bz2"));
            assert_eq!(archive.sha256.len(), 64);
            let names: Vec<&str> = archive.libs.iter().map(|lib| lib.name).collect();
            assert_eq!(names.len(), 2);
            assert!(names[0].starts_with("libsherpa-onnx-c-api."));
            assert!(names[1].starts_with("libonnxruntime."));
            let extension = if target.is_linux() { "so" } else { "dylib" };
            assert!(names.iter().all(|name| name.ends_with(extension)));
        }
    }

    #[test]
    fn the_folder_layout_groups_the_libraries_by_target() {
        let dir = dir(Path::new("/t"), Target::LinuxArm64);
        assert_eq!(dir, Path::new("/t/tts-runtime/linux-arm64"));
        let libs = libs_in(&dir, Target::LinuxArm64);
        assert_eq!(
            libs,
            [
                dir.join("libsherpa-onnx-c-api.so"),
                dir.join("libonnxruntime.so")
            ]
        );
    }

    #[test]
    fn a_library_matches_only_its_pinned_digest() {
        let dir = std::env::temp_dir().join(format!("tts-rt-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let lib = Lib {
            name: "libx.so",
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        let path = dir.join("libx.so");
        assert!(!matches_pinned(&path, &lib).unwrap());
        fs::write(&path, b"abc").unwrap();
        assert!(matches_pinned(&path, &lib).unwrap());
        fs::write(&path, b"abcd").unwrap();
        assert!(!matches_pinned(&path, &lib).unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A 64-bit little-endian ELF file with a section-name table and `.text` holding `code`.
    fn elf(code: &[u8]) -> Vec<u8> {
        let names = b"\0.text\0.shstrtab\0";
        let names_at = 64u64;
        let code_at = names_at + names.len() as u64;
        let headers = code_at + code.len() as u64;
        let mut file = vec![0u8; 64];
        file[..6].copy_from_slice(b"\x7fELF\x02\x01");
        file[0x28..0x30].copy_from_slice(&headers.to_le_bytes());
        file[0x3A..0x3C].copy_from_slice(&64u16.to_le_bytes());
        file[0x3C..0x3E].copy_from_slice(&3u16.to_le_bytes());
        file[0x3E..0x40].copy_from_slice(&2u16.to_le_bytes());
        file.extend_from_slice(names);
        file.extend_from_slice(code);
        let section = |name: u32, offset: u64, size: u64| {
            let mut header = vec![0u8; 64];
            header[..4].copy_from_slice(&name.to_le_bytes());
            header[0x18..0x20].copy_from_slice(&offset.to_le_bytes());
            header[0x20..0x28].copy_from_slice(&size.to_le_bytes());
            header
        };
        file.extend(section(0, 0, 0));
        file.extend(section(1, code_at, code.len() as u64));
        file.extend(section(7, names_at, names.len() as u64));
        file
    }

    /// A 64-bit Mach-O file with one `__TEXT` segment whose `__text` section holds `code`.
    fn mach_o(code: &[u8], signature: &[u8]) -> Vec<u8> {
        let mut file = vec![0u8; 32];
        file[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
        file[16..20].copy_from_slice(&1u32.to_le_bytes());
        let mut segment = vec![0u8; 72 + 80];
        segment[..4].copy_from_slice(&0x19u32.to_le_bytes());
        segment[4..8].copy_from_slice(&(152u32).to_le_bytes());
        segment[8..14].copy_from_slice(b"__TEXT");
        segment[64..68].copy_from_slice(&1u32.to_le_bytes());
        segment[72..78].copy_from_slice(b"__text");
        segment[88..94].copy_from_slice(b"__TEXT");
        segment[112..120].copy_from_slice(&(code.len() as u64).to_le_bytes());
        segment[120..124].copy_from_slice(&(32u32 + 152).to_le_bytes());
        file.extend(segment);
        file.extend_from_slice(code);
        file.extend_from_slice(signature);
        file
    }

    #[test]
    fn the_code_section_survives_a_new_signature_or_a_strip() {
        assert_eq!(code_section(&elf(b"CODE")), Some(&b"CODE"[..]));
        let signed_once = mach_o(b"CODE", b"sig-1");
        let signed_again = mach_o(b"CODE", b"another-signature");
        assert_ne!(signed_once, signed_again);
        assert_eq!(code_section(&signed_once), code_section(&signed_again));
        assert_eq!(code_section(&signed_once), Some(&b"CODE"[..]));
        assert_ne!(code_section(&mach_o(b"EDOC", b"")), Some(&b"CODE"[..]));
        assert_eq!(code_section(b"not a library"), None);
        assert_eq!(code_section(&elf(b"CODE")[..80]), None);
    }

    #[test]
    fn arguments_are_parsed_and_unknown_ones_refused() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let options = parse_args(&args(&["--target", "linux-x64", "--dir", "/x"])).unwrap();
        assert_eq!(options.target, Some(Target::LinuxX64));
        assert_eq!(options.dir, Some(PathBuf::from("/x")));
        assert!(parse_args(&args(&["--dir"])).is_err());
        assert!(parse_args(&args(&["--bogus"])).is_err());
    }
}
