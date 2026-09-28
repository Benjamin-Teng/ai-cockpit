# tasks：html-charset

> 執行路徑：直接 apply（不走 SDD）｜理由：只改 `cockpit` 一個函式的 content-type 決定與對應測試、腳本一段、文件一處，
> 任務線性且範圍小；品質 gate 與 Codex review 照常。

通則：Rust 修改以 TDD 進行（先紅後綠，回報附證據）；改 `.rs` 跑 `cargo fmt --check`、`cargo clippy -p cockpit --all-targets -- -D warnings`、
`cargo test -p cockpit`；改 `.md` 在 repo 根跑 `markdownlint-cli2 "**/*.md"`；跑瀏覽器腳本前先 `cargo build -p cockpit --example ui_preview`、
確認 7770 沒人用（不是自己開的程序不准砍），跑完確認無殘留。

## 1. 先寫會失敗的測試

- [x] 1.1 `cockpit/tests/files_endpoint.rs`：依 spec 更新斷言 `.html`／`.htm` 為 `text/html` 的既有測試（scenario「HTML 帶 sandbox」
  的 UTF-8 內容改為期望 `text/html; charset=utf-8`），新增「沒有宣告編碼的 UTF-8 HTML」（無 meta、含中文 → 帶 charset）、
  「非 UTF-8 的 HTML」（Big5 位元組＋`<meta charset="big5">` → 不帶 charset、本體與檔案位元組相同）、只有前 8192 位元組是
  UTF-8 而後段不是的 HTML（→ 不帶 charset，鎖住 design D2）、UTF-8 BOM（→ 帶 charset）與 `.HTM` 大寫副檔名；驗收＝
  `cargo test -p cockpit --test files_endpoint` 中這些測試 FAIL（RED 輸出記錄下來），其餘測試仍通過
- [x] 1.2 `docs/research/2026-09-27/files-check.js` 新增段 `file-review/沒有宣告編碼的 UTF-8 HTML`：在 ui_preview 暫存副本寫入
  無 meta、無 BOM 的 UTF-8 `nometa.html`（含「檔案瀏覽」），從檔案樹開啟，經 CDP 讀 iframe 內文字，斷言含「檔案瀏覽」且原始內容
  回應的 `Content-Type` 為 `text/html; charset=utf-8`；`files-check.md` 段落表補這段；驗收＝對目前程式執行此段 FAIL（RED），
  其餘段落仍 PASS，`markdownlint-cli2` 0 issues

## 2. 實作

- [x] 2.1 `cockpit/src/files.rs`：原始內容回應對 `.html`／`.htm`，以實際讀到的位元組做完整 UTF-8 驗證，合法 → `text/html; charset=utf-8`，
  否則 → `text/html`（design D1、D2）；驗收＝1.1 的測試全部 PASS，`cargo fmt --check`、`cargo clippy -p cockpit --all-targets -- -D warnings`、
  `cargo test -p cockpit` 全綠；1.2 的腳本段 PASS
- [x] 2.2 `cockpit/README.md`「檔案瀏覽與 Review」的原始內容 content-type 說明依 spec 更新；驗收＝`markdownlint-cli2 "**/*.md"` 0 issues

## 3. 收斂

- [x] 3.1 全 gate 與全部腳本：repo 根 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace &&
  cargo test -p cockpit --example ui_preview && markdownlint-cli2 "**/*.md" && openspec validate --all`；`files-check.js`、
  `visual-check.js` 與既有六支腳本全綠（`factory-floor-check.js` 跑完 `git checkout -- docs/research/2026-09-16/task-5.2-scenario-d.png`）；
  以 `codex-companion.mjs adversarial-review --wait --base main` 審整支分支，只採信最後「# Codex Adversarial Review」段，findings 先重現再處理；
  驗收＝gate 與腳本輸出全綠、Codex 結論為 approve 或 findings 已處理
- [x] 3.2 `docs/handover.md` 依 `~/.claude/guides/handover-template.md` 更新（本 change 完成、第 5 節移除 charset 待決事項）；
  驗收＝`markdownlint-cli2 "**/*.md"` 0 issues
