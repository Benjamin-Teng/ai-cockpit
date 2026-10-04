//! 把應用程式圖示與版本資訊嵌入 `cockpit.exe`、`cockpit-launch.exe`（change app-icon design D3）。
//!
//! 圖示是 `packaging/icon/app.ico`（由同目錄的 `gen-icon.js` 從 SVG 母檔產生並提交，建置不需要瀏覽器）。
//! 版本欄位沿用 winresource 預設：取 `CARGO_PKG_VERSION`，所以執行檔版本＝crate 版本＝發版 tag。
//!
//! 只在目標為 Windows 時嵌入；判斷用 `CARGO_CFG_TARGET_OS` 而不是 `#[cfg(target_os)]`——build script 在 host 上
//! 編譯，`cfg` 看到的是 host（winresource README）。編譯資源失敗就讓建置失敗，不靜默略過：發布的執行檔不得少了圖示。

const ICON: &str = "../packaging/icon/app.ico";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={ICON}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon(ICON)
        .set("ProductName", "AI Agent Cockpit")
        .set("FileDescription", "AI Agent Cockpit")
        .set("LegalCopyright", "Copyright (c) 2026 Benjamin-Teng");
    if let Err(e) = res.compile() {
        panic!("無法把圖示與版本資訊編成 Windows 資源（需要 Windows SDK 的 rc.exe）：{e}");
    }
}
