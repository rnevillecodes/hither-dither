fn main() {
    // tauri-build embeds the icons (e.g. the Windows .ico resource) but only reruns when
    // tauri.conf.json changes, so regenerated icons would otherwise be ignored.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
