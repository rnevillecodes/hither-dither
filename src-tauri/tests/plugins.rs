//! Integration tests for VST3 hosting, using the plugins in `test-plugins/`.

mod common;

use common::test_plugins::{self, TestPlugin};

#[test]
fn test_plugin_bundles_have_the_platform_layout() {
    let dir = tempfile::tempdir().unwrap();
    let bundles = test_plugins::bundle_all(dir.path());
    assert_eq!(bundles.len(), 3);
    for (bundle, plugin) in bundles.iter().zip(TestPlugin::ALL) {
        let name = plugin.package();
        assert_eq!(bundle, &dir.path().join(format!("{name}.vst3")));
        let contents = bundle.join("Contents");
        let binary = if cfg!(target_os = "macos") {
            assert!(contents.join("Info.plist").is_file());
            contents.join("MacOS").join(name)
        } else if cfg!(windows) {
            contents
                .join(test_plugins::binary_dir())
                .join(format!("{name}.vst3"))
        } else {
            contents
                .join(test_plugins::binary_dir())
                .join(format!("{name}.so"))
        };
        let size = std::fs::metadata(&binary).map(|m| m.len()).unwrap_or(0);
        assert!(size > 0, "{} is missing or empty", binary.display());
    }
    if !cfg!(target_os = "macos") {
        let arch_os = test_plugins::binary_dir();
        let (arch, os) = arch_os.split_once('-').unwrap();
        assert!(!arch.is_empty());
        assert_eq!(os, if cfg!(windows) { "win" } else { "linux" });
    }
}
