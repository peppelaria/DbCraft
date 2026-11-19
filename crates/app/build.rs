//! Embeds the app icon in dbcraft.exe (Windows). The icons come from packaging/icon/dbcraft.svg
//! (packaging/icon/build.sh).

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/windows/dbcraft.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/windows/dbcraft.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=the icon was not embedded: {e}");
        }
    }
}
