# tasks：output-color-tuning

> 執行路徑：直接 `opsx:apply`（不上 SDD）｜理由：兩個小改動（一個整數判定式、一個 CSS 值）加對應測試與截圖，線性且可一次驗收。
> **審查**：本專案 Opus 5.5 審查視同 Codex（使用者 2026-10-02 指示），收尾前審一次 `main..HEAD`。

通則：Rust 改動跑 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`；改 `.md` 跑 `markdownlint-cli2 "**/*.md"`；
改 `openspec/` 跑 `openspec validate --all`；改前端或 `ui_preview` 先 `cargo build -p cockpit --example ui_preview` 再跑腳本；腳本一律前景、依序跑。

## 1. 實作

- [x] 1.1 無彩門檻（design D1）：`cockpit-herdr/src/ansi.rs` 改判定式與文件註解；`cockpit-herdr/tests/ansi.rs` 更新界線測試（舊的 63／64 界線改為
  新規則的兩種界線：spec「無彩門檻的界線」四個值），新增 spec「深色主題 diff 底色不被歸成灰」；先紅後綠。驗收：`cargo test -p cockpit-herdr`
- [x] 1.2 粗體 700（design D2）：`style.css` 的 `.ansi-bold`；`docs/research/2026-10-02/output-color-check.js` 的粗體斷言改 700（先紅後綠，腳本修改與產品修改
  分開 commit）。驗收：`output-color-check.js` PASS、`ui_preview` 測試若有依舊門檻預期的標籤一併更新並通過
- [x] 1.3 截圖：`node docs/research/2026-10-02/output-color-check.js --screenshots` 重拍三張，逐張看圖（遮罩與無真名）

## 2. 驗收與收尾

- [x] 2.1 全 gate：Rust gate、`cargo test -p cockpit --example ui_preview`、markdownlint、`openspec validate --all`、12 支驗收腳本、`deid-check.js` 0 命中
- [x] 2.2 Opus 審查 `main..HEAD`，findings 實測後才改
- [ ] 2.3 squash 併回 `main`、archive、更新交接手冊、推送（推送前 `deid-check.js --history` 0 命中）
