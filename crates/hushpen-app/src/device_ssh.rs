//! Install and pair a model host through an existing SSH login.

use std::fs::File;
use std::io::{self, BufRead, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Connecting,
    Downloading,
    Verifying,
    Installing,
    Starting,
    Pairing,
}

impl Step {
    pub fn label(self) -> &'static str {
        match self {
            Self::Connecting => "Connect to the server",
            Self::Downloading => "Copy the model host",
            Self::Verifying => "Check the download",
            Self::Installing => "Install the model host",
            Self::Starting => "Start the model host",
            Self::Pairing => "Pair this computer",
        }
    }
}

pub enum Source {
    Release {
        version: String,
    },
    Local {
        archive: PathBuf,
        sha256: String,
        architecture: String,
    },
}

/// A local bundle lets setup work before the host has a published release.
pub fn source(directory: &Path, version: &str) -> io::Result<Source> {
    let available: Vec<_> = ["x64", "arm64"]
        .into_iter()
        .filter_map(|architecture| {
            let name = format!("Hushpen-{version}-host-linux-{architecture}.tar.gz");
            let archive = directory.join(&name);
            archive.is_file().then_some((architecture, name, archive))
        })
        .collect();
    let sums = directory.join("SHA256SUMS");
    if available.len() == 1 && sums.is_file() {
        let mut manifest = String::new();
        File::open(sums)?
            .take(65_537)
            .read_to_string(&mut manifest)?;
        if manifest.len() > 65_536 {
            return Err(invalid("The local checksum list is too large."));
        }
        let (architecture, name, archive) = &available[0];
        let mut matches = manifest.lines().filter_map(|line| {
            let mut fields = line.split_whitespace();
            let sum = fields.next()?;
            let filename = fields.next()?;
            let filename = filename.strip_prefix('*').unwrap_or(filename);
            (filename == name && fields.next().is_none()).then_some(sum)
        });
        let sha256 = matches
            .next()
            .filter(|sum| sum.len() == 64 && sum.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or_else(|| invalid("The local host bundle has no valid checksum."))?;
        if matches.next().is_some() {
            return Err(invalid("The local host bundle has duplicate checksums."));
        }
        return Ok(Source::Local {
            archive: archive.clone(),
            sha256: sha256.to_owned(),
            architecture: (*architecture).to_owned(),
        });
    }
    Ok(Source::Release {
        version: version.to_owned(),
    })
}

/// Host patterns cannot name an individual server or become SSH options.
pub fn hosts(config: &str) -> Vec<String> {
    let mut hosts = Vec::new();
    for line in config.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        let line = line.replacen('=', " ", 1);
        let mut fields = line.split_whitespace();
        if !fields
            .next()
            .is_some_and(|keyword| keyword.eq_ignore_ascii_case("host"))
        {
            continue;
        }
        for host in fields.filter(|host| valid_host(host)) {
            if !hosts.iter().any(|saved| saved == host) {
                hosts.push(host.to_owned());
            }
        }
    }
    hosts
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

/// No password, host-key, or keyboard-interactive prompt is allowed.
pub fn setup(
    host: &str,
    source: &Source,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(Step),
) -> io::Result<String> {
    if cancelled.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Server setup was cancelled.",
        ));
    }
    if !valid_host(host) {
        return Err(invalid("Choose a host from your SSH config."));
    }
    let script = script(source)?;
    let input = match source {
        Source::Local { archive, .. } => Stdio::from(File::open(archive)?),
        Source::Release { .. } => Stdio::null(),
    };
    progress(Step::Connecting);
    let mut command = Command::new("ssh");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=10",
            "-o",
            "ServerAliveCountMax=2",
            "--",
            host,
            &format!("sh -c {}", quote(&script)),
        ])
        .stdin(input)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("SSH output is unavailable."))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("SSH errors are unavailable."))?;
    let (sender, receiver) = mpsc::channel();
    let output = std::thread::spawn(move || {
        let mut total = 0;
        for line in io::BufReader::new(stdout).take(65_537).lines() {
            let line = line?;
            total += line.len();
            if total > 65_536 {
                return Err(io::Error::other("The server returned too much output."));
            }
            if sender.send(line).is_err() {
                break;
            }
        }
        Ok(())
    });
    let errors = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut stderr = stderr;
        let _ = stderr.by_ref().take(8192).read_to_end(&mut bytes);
        let _ = io::copy(&mut stderr, &mut io::sink());
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(240);
    let mut pairing = None;
    let mut failure = None;
    let status = loop {
        while let Ok(line) = receiver.try_recv() {
            if let Err(problem) = receive(&line, &mut progress, &mut pairing) {
                failure = Some(problem);
                break;
            }
        }
        if failure.is_some() {
            terminate(&mut child);
            break child.wait()?;
        }
        if cancelled.load(Ordering::Acquire) {
            terminate(&mut child);
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Server setup was cancelled.",
            ));
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            terminate(&mut child);
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "SSH setup timed out.",
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    while !output.is_finished() || !errors.is_finished() {
        if Instant::now() >= deadline || cancelled.load(Ordering::Acquire) {
            terminate(&mut child);
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "SSH output did not close.",
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    output
        .join()
        .map_err(|_| io::Error::other("SSH output stopped."))??;
    let errors = errors
        .join()
        .map_err(|_| io::Error::other("SSH error output stopped."))?;
    for line in receiver.try_iter() {
        if failure.is_none() {
            receive(&line, &mut progress, &mut pairing)?;
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if !status.success() {
        // SSH errors can contain an offer, so only show our fixed failure text.
        let _ = errors;
        return Err(io::Error::other(
            "Server setup failed. Check SSH access, free space, and systemd user services.",
        ));
    }
    pairing.ok_or_else(|| io::Error::other("The server did not return a pairing offer."))
}

fn terminate(child: &mut std::process::Child) {
    #[cfg(unix)]
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(windows)]
    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &child.id().to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
}

fn receive(
    line: &str,
    progress: &mut impl FnMut(Step),
    pairing: &mut Option<String>,
) -> io::Result<()> {
    let step = match line {
        "HUSHPEN_STEP download" => Some(Step::Downloading),
        "HUSHPEN_STEP verify" => Some(Step::Verifying),
        "HUSHPEN_STEP install" => Some(Step::Installing),
        "HUSHPEN_STEP start" => Some(Step::Starting),
        "HUSHPEN_STEP pair" => Some(Step::Pairing),
        _ => None,
    };
    if let Some(step) = step {
        progress(step);
    } else if line.starts_with('{') {
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|_| invalid("The server returned an invalid pairing offer."))?;
        let uri = value["uri"]
            .as_str()
            .ok_or_else(|| invalid("The server returned no pairing offer."))?;
        hushpen_remote::PairOffer::parse(uri)
            .map_err(|_| invalid("The server returned an invalid pairing offer."))?;
        if pairing.replace(uri.to_owned()).is_some() {
            return Err(invalid("The server returned more than one pairing offer."));
        }
    }
    Ok(())
}

fn script(source: &Source) -> io::Result<String> {
    let copy = match source {
        Source::Release { version }
            if !version.is_empty()
                && version.len() <= 64
                && version
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b".-".contains(&byte)) =>
        {
            format!(
                r#"version={}
name="Hushpen-$version-host-linux-$arch.tar.gz"
base="https://github.com/surajmandalcell/hushpen/releases/download/v$version"
curl --fail --location --proto '=https' --tlsv1.2 --connect-timeout 10 --max-time 150 "$base/$name" -o "$stage/host.tar.gz"
curl --fail --location --proto '=https' --tlsv1.2 --connect-timeout 10 --max-time 30 "$base/SHA256SUMS" -o "$stage/SHA256SUMS"
sum=$(awk -v name="$name" '$2 == name {{print $1}}' "$stage/SHA256SUMS")
"#,
                quote(version)
            )
        }
        Source::Local {
            sha256,
            architecture,
            ..
        } if sha256.len() == 64
            && sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            && ["x64", "arm64"].contains(&architecture.as_str()) =>
        {
            format!(
                "[ \"$arch\" = {} ] || exit 11\ncat > \"$stage/host.tar.gz\"\nsum={}\n",
                quote(architecture),
                quote(sha256)
            )
        }
        _ => return Err(invalid("The model host source is invalid.")),
    };
    Ok(format!(
        r#"set -eu
umask 077
[ "$(uname -s)" = Linux ] || exit 10
case "$(uname -m)" in x86_64|amd64) arch=x64 ;; aarch64|arm64) arch=arm64 ;; *) exit 11 ;; esac
command -v systemctl >/dev/null
command -v sha256sum >/dev/null
root_device=$(df -Pk / | awk 'NR == 2 {{print $1}}')
data=${{HUSHPEN_DATA_DIR:-}}
if [ -z "$data" ] && [ -f "$HOME/.config/hushpen-host/environment" ]; then
    data=$(awk -F= '$1 == "HUSHPEN_DATA_DIR" {{print substr($0, index($0,"=")+1)}}' "$HOME/.config/hushpen-host/environment")
fi
if [ -z "$data" ]; then
    drive=$(df -Pk | awk -v root="$root_device" '$1 ~ /^\/dev\// && $1 != root && $6 !~ /^\/boot/ {{print $2, $6}}' | sort -nr | head -n 1 | cut -d' ' -f2-)
    [ -n "$drive" ] && [ -w "$drive" ] || exit 12
    data="$drive/data/hushpen-host"
fi
case "$data" in /*) ;; *) exit 12 ;; esac
mkdir -p "$data"
[ "$(df -Pk "$data" | awk 'NR == 2 {{print $1}}')" != "$root_device" ] || exit 12
stage=$(mktemp -d "$data/setup.XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT
trap 'exit 130' HUP INT TERM
echo 'HUSHPEN_STEP download'
{copy}
echo 'HUSHPEN_STEP verify'
[ "${{#sum}}" = 64 ]
printf '%s  %s\n' "$sum" "$stage/host.tar.gz" | sha256sum --check --status
tar -tzf "$stage/host.tar.gz" > "$stage/files"
tar -tvzf "$stage/host.tar.gz" | awk 'substr($1,1,1) != "-" {{exit 1}}'
while IFS= read -r file; do
    case "$file" in hushpen-host|hushpen-asr|hushpen-llm|ggml-silero-v5.1.2.bin) ;; *) exit 13 ;; esac
done < "$stage/files"
tar --extract --gzip --no-same-owner --no-same-permissions --file "$stage/host.tar.gz" --directory "$stage"
for file in hushpen-host hushpen-asr hushpen-llm ggml-silero-v5.1.2.bin; do
    [ -f "$stage/$file" ] && [ ! -L "$stage/$file" ] || exit 13
done
chmod 700 "$stage/hushpen-host" "$stage/hushpen-asr" "$stage/hushpen-llm"
for file in hushpen-host hushpen-asr hushpen-llm; do
    "$stage/$file" --version >/dev/null
done
echo 'HUSHPEN_STEP install'
install="$HOME/.local/share/hushpen-host"
mkdir -p "$install" "$HOME/.config/hushpen-host" "$HOME/.config/systemd/user"
environment="$HOME/.config/hushpen-host/environment"
unit="$HOME/.config/systemd/user/hushpen-host.service"
was_active=0
systemctl --user is-active --quiet hushpen-host.service && was_active=1
was_enabled=$(systemctl --user is-enabled hushpen-host.service 2>/dev/null || true)
case "$was_enabled" in masked|masked-runtime) exit 14 ;; esac
mkdir "$stage/previous"
remember() {{
    if [ -e "$1" ] || [ -L "$1" ]; then cp -Pp "$1" "$stage/previous/$2"; fi
}}
restore() {{
    rm -f "$1.new" || return 1
    if [ -e "$stage/previous/$2" ] || [ -L "$stage/previous/$2" ]; then
        cp -Pp "$stage/previous/$2" "$1.new" && mv -f "$1.new" "$1"
    else
        rm -f "$1" "$1.new"
    fi
}}
rollback() {{
    systemctl --user stop hushpen-host.service || return 1
    if [ -e "$unit" ] || [ -L "$unit" ]; then
        systemctl --user disable hushpen-host.service >/dev/null || return 1
    fi
    for file in hushpen-host hushpen-asr hushpen-llm ggml-silero-v5.1.2.bin; do
        restore "$install/$file" "$file" || return 1
    done
    restore "$environment" environment || return 1
    restore "$unit" unit || return 1
    systemctl --user daemon-reload || return 1
    case "$was_enabled" in
        enabled) systemctl --user enable hushpen-host.service >/dev/null || return 1 ;;
        enabled-runtime) systemctl --user enable --runtime hushpen-host.service >/dev/null || return 1 ;;
    esac
    if [ "$was_active" = 1 ]; then systemctl --user start hushpen-host.service || return 1; fi
}}
finish() {{
    status=$?
    trap - EXIT HUP INT TERM
    if [ "$status" != 0 ] && ! rollback; then
        printf 'Host rollback failed. Previous files remain in %s/previous\n' "$stage" >&2
        exit "$status"
    fi
    rm -rf -- "$stage"
    exit "$status"
}}
for file in hushpen-host hushpen-asr hushpen-llm ggml-silero-v5.1.2.bin; do
    remember "$install/$file" "$file"
done
remember "$environment" environment
remember "$unit" unit
trap finish EXIT
for file in hushpen-host hushpen-asr hushpen-llm ggml-silero-v5.1.2.bin; do
    cp "$stage/$file" "$install/$file.new"
    mv -f "$install/$file.new" "$install/$file"
done
printf 'HUSHPEN_DATA_DIR=%s\n' "$data" > "$environment.new"
mv -f "$environment.new" "$environment"
cat > "$unit.new" <<'HUSHPEN_UNIT'
{unit}
HUSHPEN_UNIT
mv -f "$unit.new" "$unit"
echo 'HUSHPEN_STEP start'
systemctl --user daemon-reload
systemctl --user enable hushpen-host.service >/dev/null
systemctl --user restart hushpen-host.service
sleep 1
systemctl --user is-active --quiet hushpen-host.service
echo 'HUSHPEN_STEP pair'
address=$(printf '%s\n' "$SSH_CONNECTION" | awk '{{print $3}}')
[ -n "$address" ]
"$install/hushpen-host" pair --json --data-dir "$data" --host "$address"
systemctl --user is-active --quiet hushpen-host.service
"#,
        unit = include_str!("../../../packaging/hushpen-host.service")
    ))
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_source_requires_one_bundle_and_its_exact_checksum() {
        let directory = tempfile::tempdir().unwrap();
        let name = "Hushpen-2.3.0-host-linux-x64.tar.gz";
        assert!(matches!(
            source(directory.path(), "2.3.0").unwrap(),
            Source::Release { .. }
        ));
        std::fs::write(directory.path().join(name), b"bundle").unwrap();
        std::fs::write(
            directory.path().join("SHA256SUMS"),
            format!("{}  {name}\n", "ab".repeat(32)),
        )
        .unwrap();
        let Source::Local {
            archive,
            architecture,
            sha256,
        } = source(directory.path(), "2.3.0").unwrap()
        else {
            panic!("one checksummed local bundle must be used");
        };
        assert_eq!(archive, directory.path().join(name));
        assert_eq!(architecture, "x64");
        assert_eq!(sha256, "ab".repeat(32));
        std::fs::write(directory.path().join("SHA256SUMS"), "bad checksum").unwrap();
        assert!(source(directory.path(), "2.3.0").is_err());
        std::fs::write(
            directory
                .path()
                .join("Hushpen-2.3.0-host-linux-arm64.tar.gz"),
            b"other bundle",
        )
        .unwrap();
        assert!(matches!(
            source(directory.path(), "2.3.0").unwrap(),
            Source::Release { .. }
        ));
    }

    #[test]
    fn ssh_hosts_exclude_patterns_options_and_duplicate_aliases() {
        assert_eq!(
            hosts(
                "Host lin1 win1\nhost=another\nHost *.local !private ?box -oBad\nHOST lin1 oci1 # comment\n Host valid_name\n"
            ),
            ["lin1", "win1", "another", "oci1", "valid_name"]
        );
        assert!(!valid_host("host;touch /tmp/file"));
        assert!(!valid_host("host\n"));
    }

    #[test]
    fn cancelled_setup_does_not_start_ssh() {
        let problem = setup(
            "lin1",
            &Source::Release {
                version: "2.3.0".into(),
            },
            &AtomicBool::new(true),
            |_| panic!("cancelled setup must not start"),
        )
        .unwrap_err();
        assert_eq!(problem.kind(), io::ErrorKind::Interrupted);
    }

    #[test]
    fn setup_script_checks_architecture_and_checksum_before_installing() {
        let rendered = script(&Source::Local {
            archive: "host.tar.gz".into(),
            sha256: "ab".repeat(32),
            architecture: "arm64".into(),
        })
        .unwrap();
        let positions: Vec<_> = [
            "[ \"$arch\" = 'arm64' ]",
            "sha256sum --check --status",
            "tar --extract",
            "echo 'HUSHPEN_STEP install'",
            "systemctl --user restart",
            "pair --json",
        ]
        .iter()
        .map(|needle| rendered.find(needle).unwrap())
        .collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(rendered.contains("trap 'rm -rf --"));
        assert!(
            script(&Source::Release {
                version: "bad'$(id)".into()
            })
            .is_err()
        );
    }

    #[test]
    fn setup_streams_steps_without_exposing_the_offer() {
        let mut steps = Vec::new();
        let mut offer = None;
        receive(
            "HUSHPEN_STEP verify",
            &mut |step| steps.push(step),
            &mut offer,
        )
        .unwrap();
        assert_eq!(steps, [Step::Verifying]);
        assert!(offer.is_none());
        assert!(receive("{\"uri\":\"invalid\"}", &mut |_| {}, &mut offer).is_err());
    }

    #[test]
    fn failed_upgrade_restores_files_before_starting_the_previous_service() {
        let rendered = script(&Source::Release {
            version: "2.3.0".into(),
        })
        .unwrap();
        let version_check = rendered.find("\"$stage/$file\" --version").unwrap();
        let backup = rendered.find("remember \"$unit\" unit").unwrap();
        let trap = rendered.find("trap finish EXIT").unwrap();
        let swap = rendered
            .find("cp \"$stage/$file\" \"$install/$file.new\"")
            .unwrap();
        assert!(version_check < backup && backup < trap && trap < swap);
        let rollback = rendered
            .split_once("rollback() {")
            .unwrap()
            .1
            .split_once("finish() {")
            .unwrap()
            .0;
        assert!(rollback.find("stop hushpen-host").unwrap() < rollback.find("restore").unwrap());
        assert!(
            rollback.find("restore \"$unit\" unit").unwrap()
                < rollback.find("daemon-reload").unwrap()
        );
        assert!(rollback.contains("if [ \"$was_active\" = 1 ]; then"));
        assert!(rollback.contains("enabled-runtime)"));
        assert!(rendered.contains("if [ \"$status\" != 0 ] && ! rollback; then"));
        assert!(!rendered.contains("rm -rf -- \"$data\""));
    }

    #[cfg(unix)]
    #[test]
    fn generated_setup_is_a_valid_posix_shell_script() {
        use std::io::Write;

        for source in [
            Source::Release {
                version: "2.3.0-rc.1".into(),
            },
            Source::Local {
                archive: "host.tar.gz".into(),
                sha256: "ab".repeat(32),
                architecture: "x64".into(),
            },
        ] {
            let mut child = Command::new("sh")
                .arg("-n")
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(script(&source).unwrap().as_bytes())
                .unwrap();
            assert!(child.wait().unwrap().success());
        }
    }

    #[test]
    #[ignore = "Installs the supplied Linux host bundle on an explicitly selected SSH server"]
    fn install_a_local_bundle_over_ssh() {
        let host = std::env::var("HUSHPEN_SSH_TEST_HOST").unwrap();
        let archive = PathBuf::from(std::env::var_os("HUSHPEN_SSH_TEST_ARCHIVE").unwrap());
        let architecture = std::env::var("HUSHPEN_SSH_TEST_ARCH").unwrap();
        let sha256 = hushpen_store::model_files::hash_file(&archive, |_| true).unwrap();
        let source = Source::Local {
            archive,
            sha256,
            architecture,
        };
        let mut steps = Vec::new();
        let uri = setup(&host, &source, &AtomicBool::new(false), |step| {
            steps.push(step)
        })
        .unwrap();
        let offer = hushpen_remote::PairOffer::parse(&uri).unwrap();
        assert_eq!(offer.port, 47321);
        assert_eq!(
            steps,
            [
                Step::Connecting,
                Step::Downloading,
                Step::Verifying,
                Step::Installing,
                Step::Starting,
                Step::Pairing,
            ]
        );
    }
}
