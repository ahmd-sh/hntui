// `hntui update`: swap this binary for the latest GitHub release, in place.
// Same release assets and flow as the TypeScript updater, so old installs
// update into new ones and back.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::api::{compare_versions, fetch_latest_version};

const REPO: &str = "ahmd-sh/hntui";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The release tarball for this machine (it holds a single `hntui`).
pub fn release_asset() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("hntui-darwin-arm64.tar.gz"),
        ("macos", "x86_64") => Some("hntui-darwin-x64.tar.gz"),
        ("linux", "x86_64") => Some("hntui-linux-x64.tar.gz"),
        ("linux", "aarch64") => Some("hntui-linux-arm64.tar.gz"),
        _ => None,
    }
}

enum Channel {
    Binary,
    // installed by a JS package manager: it owns the file, so it updates it
    Package(&'static str),
    Dev,
}

fn channel(exe: &Path) -> Channel {
    let p = exe.to_string_lossy();
    if p.contains("/node_modules/") {
        Channel::Package(if p.contains("/.bun/") {
            "bun add -g @ahmd-sh/hntui"
        } else if p.contains("/pnpm/") {
            "pnpm add -g @ahmd-sh/hntui"
        } else {
            "npm install -g @ahmd-sh/hntui"
        })
    } else if p.contains("/target/release/") || p.contains("/target/debug/") {
        Channel::Dev
    } else {
        Channel::Binary
    }
}

/// Why an update didn't happen. Every one of these leaves the installed
/// binary exactly as it was; `report` says so and what to try next.
#[derive(Debug)]
enum Failure {
    Offline,
    NoAsset,
    NotPublished { latest: String, asset: &'static str },
    Download(String),
    NotWritable { dir: PathBuf, sudo: bool },
    BadDownload,
    WontRun(String),
    Other(String),
}

const INSTALL: &str = "curl -fsSL https://raw.githubusercontent.com/ahmd-sh/hntui/main/install.sh | sh";
const ISSUES: &str = "https://github.com/ahmd-sh/hntui/issues";

impl Failure {
    // (what happened, why it probably did and what to do)
    fn explain(&self) -> (String, String) {
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        match self {
            Failure::Offline => (
                "couldn't reach GitHub to check for a new release.".into(),
                "Check your internet connection (and proxy, if you use one), then try again.\n\
                 GitHub's own status: https://www.githubstatus.com"
                    .into(),
            ),
            Failure::NoAsset => (
                format!("there's no prebuilt hntui for {platform}."),
                "You can build it from source with Rust installed:\n  \
                 cargo install --git https://github.com/ahmd-sh/hntui"
                    .into(),
            ),
            Failure::NotPublished { latest, asset } => (
                format!("v{latest} doesn't have {asset} (yet)."),
                "A release that's just been published can take a few minutes to get all its\n\
                 binaries; try again shortly."
                    .into(),
            ),
            Failure::Download(e) => (
                format!("the download didn't finish ({e})."),
                "Usually a flaky connection. Try again.".into(),
            ),
            Failure::NotWritable { dir, sudo } => (
                format!("can't write to {}.", dir.display()),
                if *sudo {
                    format!(
                        "hntui is installed somewhere your user can't change. Either update as an admin:\n  \
                         sudo hntui update\n\
                         or reinstall into ~/.local/bin, which needs no admin rights:\n  {INSTALL}"
                    )
                } else {
                    format!("Check the folder's permissions, or reinstall with:\n  {INSTALL}")
                },
            ),
            // tar's own wording ("Unknown error: -1") helps nobody
            Failure::BadDownload => (
                "the download was incomplete or damaged.".into(),
                "It may have been cut short or changed on the way (a proxy?). Try again.".into(),
            ),
            Failure::WontRun(e) => (
                format!("the new version doesn't run on this machine{}.", if e.is_empty() { String::new() } else { format!(" ({e})") }),
                format!(
                    "That's a bug in the release, not something on your end. Please report it at\n{ISSUES}\n\
                     with the output of `uname -a` and this message."
                ),
            ),
            Failure::Other(e) => (format!("{e}."), String::new()),
        }
    }
}

fn report(f: Failure, exe: Option<&Path>) -> i32 {
    let (what, hint) = f.explain();
    let at = exe.map(|p| format!(" at {}", p.display())).unwrap_or_default();
    eprintln!("hntui: update failed: {what}");
    if !hint.is_empty() {
        eprintln!("\n{hint}");
    }
    eprintln!("\nNothing was changed: hntui {VERSION} is still installed{at}.");
    match f {
        // no network: reinstalling needs it too
        Failure::Offline => {}
        Failure::NotWritable { .. } | Failure::NoAsset => eprintln!("Still stuck? {ISSUES}"),
        _ => eprintln!("To try again:  hntui update\nOr reinstall:  {INSTALL}\nStill stuck?   {ISSUES}"),
    }
    1
}

/// Returns the process exit code.
pub async fn run() -> i32 {
    let Some(latest) = fetch_latest_version().await else {
        return report(Failure::Offline, None);
    };
    if compare_versions(&latest, VERSION).is_le() {
        println!("hntui {VERSION} is up to date (latest release is v{latest}).");
        return 0;
    }
    println!("hntui {VERSION} → v{latest} available.");

    // the real file, not a symlink to it
    let exe = match std::env::current_exe().and_then(|p| p.canonicalize()) {
        Ok(p) => p,
        Err(e) => return report(Failure::Other(format!("can't find the hntui executable ({e})")), None),
    };
    match channel(&exe) {
        Channel::Package(cmd) => {
            println!("This copy was installed with a package manager. Update it with:\n  {cmd}");
            return 0;
        }
        Channel::Dev => {
            println!("Running from a source checkout. Update it with git pull.");
            return 0;
        }
        Channel::Binary => {}
    }
    let Some(asset) = release_asset() else {
        return report(Failure::NoAsset, Some(&exe));
    };

    println!("downloading {asset} ...");
    match download(&latest, asset).await.and_then(|bytes| {
        // Unpack next to the target: the final rename mustn't cross
        // filesystems, and renaming over a running executable is safe (the
        // running process keeps its inode; the path now names the new file).
        let dir = exe.parent().unwrap_or(Path::new("."));
        let tmp = dir.join(format!(".hntui-update-{}", std::process::id()));
        let result = install(&bytes, asset, &tmp, &exe, &latest);
        let _ = std::fs::remove_dir_all(&tmp);
        result
    }) {
        Ok(()) => {
            println!("updated to v{latest}  ({})", exe.display());
            0
        }
        Err(f) => report(f, Some(&exe)),
    }
}

// the version that was just checked, not "latest" again: a release published
// in between mustn't swap in something else
async fn download(latest: &str, asset: &'static str) -> Result<Vec<u8>, Failure> {
    let url = format!("https://github.com/{REPO}/releases/download/v{latest}/{asset}");
    let client = reqwest::Client::builder()
        .user_agent("hntui")
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| Failure::Other(e.to_string()))?;
    let res = client.get(&url).send().await.map_err(|e| Failure::Download(short(&e)))?;
    match res.status().as_u16() {
        200..=299 => {}
        404 => return Err(Failure::NotPublished { latest: latest.into(), asset }),
        code => return Err(Failure::Download(format!("HTTP {code}"))),
    }
    res.bytes().await.map(|b| b.to_vec()).map_err(|e| Failure::Download(short(&e)))
}

// reqwest's errors nest their cause; one line is plenty here
fn short(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "timed out".into()
    } else if e.is_connect() {
        "couldn't connect".into()
    } else {
        e.to_string().lines().next().unwrap_or("network error").to_string()
    }
}

fn not_writable(dir: &Path, e: &std::io::Error) -> Failure {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem => {
            // a system folder (sudo helps) vs. one of the user's own
            let home = std::env::var_os("HOME").map(PathBuf::from);
            let sudo = !home.is_some_and(|h| dir.starts_with(h));
            Failure::NotWritable { dir: dir.to_path_buf(), sudo }
        }
        _ => Failure::Other(format!("{} ({e})", dir.display())),
    }
}

fn install(tarball: &[u8], asset: &str, tmp: &Path, exe: &Path, latest: &str) -> Result<(), Failure> {
    let dir = tmp.parent().unwrap_or(Path::new("."));
    std::fs::create_dir(tmp).map_err(|e| not_writable(dir, &e))?;
    let tar_path = tmp.join(asset);
    std::fs::write(&tar_path, tarball).map_err(|e| Failure::Other(format!("couldn't save the download ({e})")))?;
    let out = Command::new("tar")
        .arg("-xzf")
        .arg(&tar_path)
        .arg("-C")
        .arg(tmp)
        .output()
        .map_err(|e| Failure::Other(format!("couldn't run tar to unpack the download ({e})")))?;
    if !out.status.success() {
        return Err(Failure::BadDownload);
    }
    let fresh = tmp.join("hntui");
    if !fresh.is_file() {
        return Err(Failure::BadDownload);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fresh, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| Failure::Other(format!("couldn't make the new binary executable ({e})")))?;
    }
    // never swap in a binary that can't run here (wrong libc, a corrupt
    // download): the old one stays put
    match Command::new(&fresh).arg("--version").output() {
        Ok(o) if o.status.success() && String::from_utf8_lossy(&o.stdout).contains(latest) => {}
        Ok(o) if o.status.success() => {
            let said = String::from_utf8_lossy(&o.stdout);
            return Err(Failure::WontRun(format!("it says it's {}, expected {latest}", said.trim())));
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let why = err.lines().next().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string);
            #[cfg(unix)]
            let why = why.or_else(|| {
                use std::os::unix::process::ExitStatusExt;
                o.status.signal().map(|s| format!("killed by signal {s}"))
            });
            return Err(Failure::WontRun(why.unwrap_or_else(|| o.status.to_string())));
        }
        Err(e) => return Err(Failure::WontRun(e.to_string())),
    }
    std::fs::rename(&fresh, exe).map_err(|e| not_writable(dir, &e))
}

#[cfg(test)]
mod tests {
    use super::{channel, install, Channel};
    use std::path::Path;

    #[test]
    fn knows_who_installed_it() {
        let pm = |p: &str| match channel(Path::new(p)) {
            Channel::Package(cmd) => cmd,
            Channel::Dev => "dev",
            Channel::Binary => "binary",
        };
        assert_eq!(pm("/Users/a/.local/bin/hntui"), "binary");
        assert_eq!(pm("/usr/local/lib/node_modules/@ahmd-sh/hntui/node_modules/@ahmd-sh/hntui-darwin-arm64/bin/hntui"), "npm install -g @ahmd-sh/hntui");
        assert_eq!(pm("/Users/a/.bun/install/global/node_modules/@ahmd-sh/hntui-darwin-arm64/bin/hntui"), "bun add -g @ahmd-sh/hntui");
        assert_eq!(pm("/Users/a/Library/pnpm/global/5/.pnpm/x/node_modules/@ahmd-sh/hntui-darwin-arm64/bin/hntui"), "pnpm add -g @ahmd-sh/hntui");
        assert_eq!(pm("/Users/a/dev/hntui/target/release/hntui"), "dev");
    }
    use std::process::Command;

    // a tarball holding an `hntui` that's really a shell script
    fn tarball(dir: &std::path::Path, script: &str) -> Vec<u8> {
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("hntui"), script).unwrap();
        let tar = dir.join("t.tar.gz");
        assert!(Command::new("tar").arg("-czf").arg(&tar).arg("-C").arg(&src).arg("hntui").status().unwrap().success());
        std::fs::read(tar).unwrap()
    }

    #[test]
    fn swaps_in_a_working_binary_and_refuses_a_broken_one() {
        let dir = std::env::temp_dir().join(format!("hntui-update-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("hntui-installed");
        std::fs::write(&exe, "old").unwrap();

        // refuses: runs but fails (think: wrong libc)
        let bad = tarball(&dir.join("a"), "#!/bin/sh\necho 'GLIBC_2.39 not found' >&2\nexit 1\n");
        let err = install(&bad, "x.tar.gz", &dir.join("t1"), &exe, "9.9.9").unwrap_err();
        assert!(matches!(&err, super::Failure::WontRun(e) if e.contains("GLIBC")), "{err:?}");
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "old");

        // refuses: runs but reports another version
        let wrong = tarball(&dir.join("b"), "#!/bin/sh\necho 'hntui 1.0.0'\n");
        assert!(install(&wrong, "x.tar.gz", &dir.join("t2"), &exe, "9.9.9").is_err());
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "old");

        // swaps
        let good = tarball(&dir.join("c"), "#!/bin/sh\necho 'hntui 9.9.9'\n");
        install(&good, "x.tar.gz", &dir.join("t3"), &exe, "9.9.9").unwrap();
        assert!(std::fs::read_to_string(&exe).unwrap().contains("9.9.9"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
