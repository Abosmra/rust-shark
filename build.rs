// Embeds the Rust Shark icon and version info into the .exe (Explorer, taskbar, Alt+Tab).
fn main() {
    println!("cargo:rerun-if-changed=assets/logo.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/logo.ico");
        res.set("ProductName", "Rust Shark");
        res.set("FileDescription", "Rust Shark");
        res.compile().expect("failed to embed the Windows icon");
    }
}
