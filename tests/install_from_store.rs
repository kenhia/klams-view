//! Tests for `deploy/install-from-store.sh` (#1013).
//!
//! The whole homelab package store is a temp directory here, because
//! **curl speaks `file://`** — so the happy path, the rollback path and
//! every refusal are exercised for real in `cargo test`, with no
//! network, no server and no root. (The idiom is klams'
//! `install_from_store.rs`, sprint 042.)
//!
//! What these tests are actually protecting: klams-view ships **two**
//! assets, and a half-deploy means a binary serving a bundle it was not
//! built with. So "one bad asset installs neither" is the property under
//! test, not a nicety.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fake package store plus the two destination directories, all under
/// one temp root that is removed when the test ends.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        // Nanos + the test name: cargo runs tests in parallel threads of
        // ONE process, so a pid would collide.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("kv-store-test-{name}-{nanos}"));
        fs::create_dir_all(root.join("store/artifacts/klams-view")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("share")).unwrap();
        Self { root }
    }

    fn store_url(&self) -> String {
        format!("file://{}/store", self.root.display())
    }
    fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }
    fn share_dir(&self) -> PathBuf {
        self.root.join("share/klams-view")
    }
    fn version_dir(&self, version: &str) -> PathBuf {
        self.root
            .join(format!("store/artifacts/klams-view/{version}"))
    }

    /// Publish one version: a stub binary that reports `reports_version`,
    /// a bundle stamped `stamp`, the installer script, and SHA256SUMS
    /// over all three. `latest` is pointed at it.
    fn publish(&self, version: &str, reports_version: &str, stamp: &str) {
        let dir = self.version_dir(version);
        fs::create_dir_all(&dir).unwrap();

        let suffix = target_suffix();
        let bin = dir.join(format!("klams-view-{suffix}"));
        // A shell stub, because the assertion under test is "does the
        // binary report the version it was published as" — which does
        // not need a real Rust build.
        fs::write(
            &bin,
            format!("#!/bin/sh\nif [ \"$1\" = --version ] || [ \"$1\" = -V ]; then echo 'klams-view {reports_version}'; fi\n"),
        )
        .unwrap();
        make_executable(&bin);

        // Bundle: index.html + the VERSION stamp an offline check reads.
        let staged = self.root.join(format!("staged-{version}"));
        fs::create_dir_all(&staged).unwrap();
        fs::write(staged.join("index.html"), "<html>klams-view</html>").unwrap();
        fs::write(staged.join("VERSION"), format!("{stamp}\n")).unwrap();
        let tarball = dir.join("klams-view-web.tar.gz");
        let st = Command::new("tar")
            .args(["-czf", tarball.to_str().unwrap(), "-C"])
            .arg(&staged)
            .args(["index.html", "VERSION"])
            .status()
            .unwrap();
        assert!(st.success(), "tar failed while publishing {version}");

        fs::copy(installer_path(), dir.join("install-from-store.sh")).unwrap();

        write_sums(&dir);
        fs::write(
            self.root.join("store/artifacts/klams-view/latest"),
            format!("{version}\n"),
        )
        .unwrap();
    }

    /// Run the installer against this fixture's store.
    fn install(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new("bash");
        cmd.arg(installer_path())
            .args(args)
            .env("KLAMS_STORE_URL", self.store_url())
            .env("BIN_DST_DIR", self.bin_dir())
            .env("SHARE_DIR", self.share_dir());
        cmd.output().unwrap()
    }

    /// What the installed binary reports, or None when nothing is there.
    fn installed_binary_version(&self) -> Option<String> {
        let bin = self.bin_dir().join("klams-view");
        if !bin.exists() {
            return None;
        }
        let out = Command::new(&bin).arg("--version").output().unwrap();
        Some(
            String::from_utf8_lossy(&out.stdout)
                .split_whitespace()
                .next_back()
                .unwrap_or_default()
                .to_string(),
        )
    }

    fn stamp(&self, sub: &str) -> Option<String> {
        fs::read_to_string(self.share_dir().join(sub).join("VERSION"))
            .ok()
            .map(|s| s.trim().to_string())
    }

    /// Corrupt one published file without touching SHA256SUMS.
    fn tamper(&self, version: &str, file: &str) {
        let path = self.version_dir(version).join(file);
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(b"\n# tampered\n");
        fs::write(&path, bytes).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn installer_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("deploy/install-from-store.sh")
}

fn target_suffix() -> String {
    let arch = run_capture("uname", &["-m"]);
    let os = run_capture("uname", &["-s"]).to_lowercase();
    format!("{arch}-{os}")
}

fn run_capture(cmd: &str, args: &[&str]) -> String {
    let out = Command::new(cmd).args(args).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();
}

/// `kpkg` writes SHA256SUMS over the artifact directory; this is the
/// same shape, `sha256sum *` from inside it.
fn write_sums(dir: &Path) {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n != "SHA256SUMS")
        .collect();
    names.sort();
    let out = Command::new("sha256sum")
        .args(&names)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "sha256sum failed in {dir:?}");
    fs::write(dir.join("SHA256SUMS"), out.stdout).unwrap();
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}
fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

// ---- the happy path -------------------------------------------------

#[test]
fn resolves_latest_and_installs_both_assets() {
    let fx = Fixture::new("happy");
    fx.publish("0.1.3", "0.1.3", "0.1.3");

    let out = fx.install(&[]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert!(stdout(&out).contains("resolved latest klams-view = 0.1.3"));

    assert_eq!(fx.installed_binary_version().as_deref(), Some("0.1.3"));
    assert_eq!(fx.stamp("web").as_deref(), Some("0.1.3"));
    assert!(fx.share_dir().join("web/index.html").exists());
    // Staging must not be left behind.
    assert!(!fx.share_dir().join("web.new").exists());
}

#[test]
fn without_restart_it_says_the_new_bundle_is_already_being_served() {
    // The window is real — ServeDir reads the bundle per request — so the
    // closing message has to name it rather than say a bland "done".
    let fx = Fixture::new("norestart");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    let out = fx.install(&[]);
    let text = stdout(&out);
    assert!(text.contains("ALREADY being served"), "{text}");
    assert!(
        text.contains("systemctl restart klams-view.service"),
        "{text}"
    );
}

#[test]
fn an_upgrade_rotates_both_previous_copies() {
    let fx = Fixture::new("rotate");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    assert!(fx.install(&[]).status.success());
    fx.publish("0.1.4", "0.1.4", "0.1.4");
    let out = fx.install(&[]);
    assert!(out.status.success(), "{}", stderr(&out));

    assert_eq!(fx.installed_binary_version().as_deref(), Some("0.1.4"));
    assert_eq!(fx.stamp("web").as_deref(), Some("0.1.4"));
    // Rollback targets, for both assets.
    let prev = fx.bin_dir().join("klams-view.prev");
    assert!(prev.exists(), "no .prev binary");
    let reported = run_capture(prev.to_str().unwrap(), &["--version"]);
    assert!(reported.ends_with("0.1.3"), "prev reports {reported}");
    assert_eq!(fx.stamp("web.prev").as_deref(), Some("0.1.3"));
    // The rotation names both versions, so a deploy log is readable
    // afterwards.
    assert!(stdout(&out).contains("(0.1.3) ->"), "{}", stdout(&out));
}

#[test]
fn an_explicit_older_version_pins_the_fetch_which_is_the_rollback_path() {
    let fx = Fixture::new("pin");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    fx.publish("0.1.4", "0.1.4", "0.1.4"); // moves `latest`
    let out = fx.install(&["--version", "0.1.3"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(fx.installed_binary_version().as_deref(), Some("0.1.3"));
    assert_eq!(fx.stamp("web").as_deref(), Some("0.1.3"));
    // `latest` was never consulted.
    assert!(!stdout(&out).contains("resolved latest"));
}

#[test]
fn dry_run_touches_nothing() {
    let fx = Fixture::new("dryrun");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    let out = fx.install(&["--dry-run"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("[dry-run]"));
    assert!(fx.installed_binary_version().is_none());
    assert!(!fx.share_dir().join("web").exists());
}

// ---- the refusals ---------------------------------------------------

#[test]
fn a_tampered_binary_is_refused_and_nothing_lands() {
    let fx = Fixture::new("tamper-bin");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    fx.tamper("0.1.3", &format!("klams-view-{}", target_suffix()));

    let out = fx.install(&[]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("checksum MISMATCH"),
        "{}",
        stderr(&out)
    );
    assert!(fx.installed_binary_version().is_none());
    assert!(!fx.share_dir().join("web").exists());
}

#[test]
fn a_tampered_bundle_refuses_the_binary_too() {
    // The property that matters for a two-asset deploy: a bad bundle
    // must not leave a new binary in place, or the host runs a build
    // serving someone else's frontend.
    let fx = Fixture::new("tamper-web");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    fx.tamper("0.1.3", "klams-view-web.tar.gz");

    let out = fx.install(&[]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("checksum MISMATCH"),
        "{}",
        stderr(&out)
    );
    assert!(
        fx.installed_binary_version().is_none(),
        "the binary landed despite a bad bundle"
    );
}

#[test]
fn a_mislabelled_binary_is_refused_which_no_checksum_could_catch() {
    // Published as 9.9.9, reports 0.0.1. The checksums are all valid —
    // only running it catches this, and it is precisely the signal a
    // version floor reads.
    let fx = Fixture::new("mislabelled");
    fx.publish("9.9.9", "0.0.1", "9.9.9");
    let out = fx.install(&[]);
    assert!(!out.status.success());
    let e = stderr(&out);
    assert!(e.contains("reports version 0.0.1"), "{e}");
    assert!(e.contains("published as 9.9.9"), "{e}");
    assert!(e.contains("store labelling is wrong, not this host"), "{e}");
    assert!(fx.installed_binary_version().is_none());
}

#[test]
fn a_bundle_stamped_with_a_different_version_is_refused() {
    // Binary 0.1.3, bundle stamped 0.1.2: two different builds published
    // into one version directory. Both checksums verify; the stamps are
    // the only thing that disagrees.
    let fx = Fixture::new("mixed-build");
    fx.publish("0.1.3", "0.1.3", "0.1.2");
    let out = fx.install(&[]);
    assert!(!out.status.success());
    let e = stderr(&out);
    assert!(e.contains("stamped 0.1.2"), "{e}");
    assert!(e.contains("different builds"), "{e}");
    assert!(fx.installed_binary_version().is_none());
}

#[test]
fn an_unset_store_url_names_the_variable_rather_than_failing_as_curl() {
    let fx = Fixture::new("nostore");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    let out = Command::new("bash")
        .arg(installer_path())
        .env_remove("KLAMS_STORE_URL")
        .env("BIN_DST_DIR", fx.bin_dir())
        .env("SHARE_DIR", fx.share_dir())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("KLAMS_STORE_URL"), "{}", stderr(&out));
}

#[test]
fn an_unpublished_version_says_so_instead_of_guessing() {
    let fx = Fixture::new("missing");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    let out = fx.install(&["--version", "0.9.9"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("fetch failed"), "{}", stderr(&out));
    assert!(fx.installed_binary_version().is_none());
}

#[test]
fn an_unwritable_destination_names_the_user_not_root() {
    // The precondition is a writable directory, not uid 0 — BIN_DST_DIR
    // is overridable precisely so a host can install into a user prefix.
    let fx = Fixture::new("unwritable");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    let out = Command::new("bash")
        .arg(installer_path())
        .env("KLAMS_STORE_URL", fx.store_url())
        .env("BIN_DST_DIR", fx.root.join("does-not-exist"))
        .env("SHARE_DIR", fx.share_dir())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(stderr(&out).contains("does not exist"), "{}", stderr(&out));
}

#[test]
fn an_unknown_argument_is_refused_rather_than_ignored() {
    let fx = Fixture::new("badarg");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    let out = fx.install(&["--reboot"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("unknown argument"),
        "{}",
        stderr(&out)
    );
}

// ---- the published installer is the one that was tested -------------

#[test]
fn the_installer_published_into_the_artifact_dir_is_this_repos_copy() {
    // The bootstrap on a repo-less host fetches the script out of the
    // store and checks it against the same SHA256SUMS as the payload.
    // That only means anything if what gets published is what is tested.
    let fx = Fixture::new("selfsame");
    fx.publish("0.1.3", "0.1.3", "0.1.3");
    let published = fs::read(fx.version_dir("0.1.3").join("install-from-store.sh")).unwrap();
    let repo = fs::read(installer_path()).unwrap();
    assert_eq!(published, repo);
    // And it is covered by the checksums the bootstrap verifies.
    let sums = fs::read_to_string(fx.version_dir("0.1.3").join("SHA256SUMS")).unwrap();
    assert!(sums.contains("install-from-store.sh"), "{sums}");
}
