fn main() {
    // A Windows application manifest has to be supplied explicitly here. The
    // MSVC toolchain provides one; mingw does not, and without it the process
    // is not marked as aware of display scaling. See the comment in
    // windows-app-manifest.xml for what that looks like from the outside.
    let attributes = tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new()
            .app_manifest(include_str!("windows-app-manifest.xml")),
    );

    tauri_build::try_build(attributes).expect("failed to run the Tauri build script")
}
