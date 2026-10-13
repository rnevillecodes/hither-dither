//! Builds the VST3 test plugins in `test-plugins/` and assembles them into `.vst3` bundles.
//!
//! The plugins are built once per test process into their own target directory, so the
//! nested Cargo never waits on the lock held by the `cargo test` that started us.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestPlugin {
    /// HD Test Gain, HD Test Stereo and HD Test Synth.
    Fx,
    /// Aborts in `GetPluginFactory`.
    Crash,
    /// Never returns from `GetPluginFactory`.
    Hang,
}

impl TestPlugin {
    pub const ALL: [TestPlugin; 3] = [TestPlugin::Fx, TestPlugin::Crash, TestPlugin::Hang];

    /// The Cargo package, which is also the bundle's name.
    pub fn package(self) -> &'static str {
        match self {
            TestPlugin::Fx => "hd-test-fx",
            TestPlugin::Crash => "hd-test-crash",
            TestPlugin::Hang => "hd-test-hang",
        }
    }

    fn library(self) -> String {
        let stem = self.package().replace('-', "_");
        if cfg!(windows) {
            format!("{stem}.dll")
        } else if cfg!(target_os = "macos") {
            format!("lib{stem}.dylib")
        } else {
            format!("lib{stem}.so")
        }
    }
}

/// The bundle subfolder holding the binary for this OS and architecture, relative to
/// `Contents/`.
pub fn binary_dir() -> &'static str {
    if cfg!(target_os = "macos") {
        "MacOS"
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        "x86_64-win"
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        "arm64-win"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64-linux"
    } else {
        "x86_64-linux"
    }
}

/// Builds every test plugin (once per process) and returns the folder holding the libraries.
fn built() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map_or_else(|| manifest.join("target"), PathBuf::from)
            .join("test-plugins");
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut cmd = Command::new(cargo);
        cmd.current_dir(manifest)
            .arg("build")
            .arg("--target-dir")
            .arg(&target);
        for p in TestPlugin::ALL {
            cmd.args(["-p", p.package()]);
        }
        let status = cmd.status().expect("run cargo to build the test plugins");
        assert!(status.success(), "building the test plugins failed");
        target.join("debug")
    })
}

/// Assembles `plugin` as `<dest>/<package>.vst3` with the layout VST3 hosts expect on this
/// OS, and returns the bundle path.
pub fn bundle(dest: &Path, plugin: TestPlugin) -> PathBuf {
    let name = plugin.package();
    let bundle = dest.join(format!("{name}.vst3"));
    let contents = bundle.join("Contents");
    let bin_dir = contents.join(binary_dir());
    std::fs::create_dir_all(&bin_dir).unwrap();
    let binary = if cfg!(target_os = "macos") {
        std::fs::write(contents.join("Info.plist"), info_plist(name)).unwrap();
        bin_dir.join(name)
    } else if cfg!(windows) {
        bin_dir.join(format!("{name}.vst3"))
    } else {
        bin_dir.join(format!("{name}.so"))
    };
    std::fs::copy(built().join(plugin.library()), &binary).unwrap();
    bundle
}

/// Assembles every test plugin into `dest`.
pub fn bundle_all(dest: &Path) -> Vec<PathBuf> {
    TestPlugin::ALL.iter().map(|&p| bundle(dest, p)).collect()
}

fn info_plist(name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>{name}</string>
  <key>CFBundleIdentifier</key><string>dev.hitherdither.tests.{name}</string>
  <key>CFBundleName</key><string>{name}</string>
  <key>CFBundlePackageType</key><string>BNDL</string>
  <key>CFBundleSignature</key><string>????</string>
  <key>CFBundleVersion</key><string>1.0.0</string>
</dict>
</plist>
"#
    )
}
