fn main() {
    slint_build::compile("ui/main.slint").expect("failed to compile Slint UI");

    // Embed the icon and version info into auris.exe so Explorer, the taskbar,
    // and shortcuts show the Auris logo instead of the generic program icon.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=assets/auris.ico");
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/auris.ico")
            .set("ProductName", "Auris")
            .set("FileDescription", "Auris launcher")
            .set("LegalCopyright", "MIT License");
        resource.compile().expect("failed to embed Windows resources");
    }
}
