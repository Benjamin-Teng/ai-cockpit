//! `cockpit-files`：檔案瀏覽與 Review 的純邏輯 crate（file-review design D1）。
//!
//! 只放安全邊界會用到的邏輯（根目錄推算、相對路徑解析與界限檢查、列目錄、中繼資料、
//! icon 對照、Markdown 渲染）；不知道 HTTP、HERDR、投影與 runtime 設定。

mod capped_read;
mod error;
mod icon;
mod list;
mod meta;
mod relpath;
mod render;
mod root;
#[cfg(test)]
mod test_support;

pub use capped_read::read_capped;
pub use error::FilesError;
pub use icon::{IconTheme, IconThemeError};
pub use list::{Entry, Kind, Listing, list_dir};
pub use meta::{FileMeta, Viewer, classify_viewer_bytes, file_meta};
pub use relpath::{RelPath, resolve};
pub use render::{render_markdown, render_markdown_str};
pub use root::{Root, find_root};
