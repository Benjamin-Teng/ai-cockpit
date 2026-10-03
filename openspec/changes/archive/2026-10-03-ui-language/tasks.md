# tasks：ui-language

> 執行路徑：SDD（`superpowers:subagent-driven-development`）｜理由：跨前後端 9 個模組、可獨立驗收的 task 多、字串搬移量大
> （約 240 條）需要逐任務審查；品質 gate 與審查（本專案 Opus 視同 Codex）照常。

通則：Rust 修改以 TDD 進行（先紅後綠，記錄證據）；改 `.rs` 跑 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、
`cargo test --workspace`；改前端跑 `cargo test -p cockpit --example ui_preview`；改 `.md` 在 repo 根跑 `markdownlint-cli2 "**/*.md"`；
跑瀏覽器腳本前確認 7770 沒人用（不是自己開的程序不准砍），一律前景逐支跑；自己開的瀏覽器一律 headless。字串搬移時繁中文字
逐字保留（既有驗收腳本以繁中字串為選取器）。

## 1. 字典骨架與語言決定

- [x] 1.1 新增 `cockpit/assets/app/i18n.js`（design D1、D2）：語言決定、`t(key, params)`、`<html lang>`、`data-i18n`／
  `data-i18n-attr` 套用；`http.rs` 白名單、`index.html` 載入順序（第一個）與路由測試；`index.html` 的靜態文字改為 `data-i18n`。
  新增 `docs/research/2026-10-03/i18n-check.js` 的 ① 字典一致性段與 ③ 語言判斷段。驗收＝路由測試與 ui_preview 測試綠、
  `i18n-check.js` ①③ PASS、繁中畫面與改動前相同（`visual-check.js` PASS）
- [x] 1.2 頂列語言切換按鈕（design D3）：`render.js` 繪製、`actions.js` 分派、`storage` 事件同步；`i18n-check.js` ④ 段。
  驗收＝④ PASS、`notify-check.js` 與 `ui-fixes-check.js` PASS

## 2. 前端字串搬進字典

- [x] 2.1 `render.js`、`actions.js`、`output.js`：全部介面字串改用 `t()`，拼接句改成具名佔位符範本，英文字典補齊
- [x] 2.2 `files.js`、`viewers.js`：同上（含 `ERROR_TEXT`、PDF 工具列、檔案大小與頁碼句）
- [x] 2.3 `git.js`：同上（約 100 條，含 `ERROR_TEXT`、筆數與行數句、分組標題）
- [x] 2.4 `notify.js`：同上（設定面板、權限文字、桌面通知標題與內文、`names.join` 的分隔符依語言）；design D5 的禁字守門改為
  Rust 測試檢查字典，移除只看 `render.js` 原始碼的舊斷言

  2.1–2.4 各自驗收＝`i18n-check.js` ① PASS；該 task 涉及模組的既有驗收腳本 PASS（繁中不變）；以 `cockpit.lang=en` 開
  `ui_preview` 檢查該模組畫面沒有繁中字典字串（`i18n-check.js` ② 對應段落）

## 3. 後端訊息代碼

- [x] 3.1 錯誤本體代碼（design D4）：`Rejection::code()`、`WriteError` 代碼、`invalid_op`、Live Output 端點錯誤，回應加
  `params`；Rust 測試斷言每個代碼與參數，既有斷言 `error` 文字的測試不變。驗收＝三項 Rust gate 綠
- [x] 3.2 投影代碼欄位（design D4）：`cockpit-core` 的 `MessageCode`、連線 `reason_msg`／`protocol_warning_msg`、project
  `warning_msgs`；`cockpit-herdr` 與 `cockpit` 產生端同步填；無法歸類 → `raw`。驗收＝三項 Rust gate 綠、`ui_preview` 測試綠
- [x] 3.3 前端改用代碼：`actions.js` 錯誤 banner、`output.js` 錯誤、`render.js` 連線原因／protocol 警告／project 警告依
  `msg.*` 翻譯並退回原文；英文字典補齊所有代碼。驗收＝`i18n-check.js` ② 的錯誤 banner 與 runtime 卡片段 PASS（英文）、
  `actions-check.js`、`live-output-check.js`、`progress-check.js` PASS（繁中）

## 4. 桌面啟動器

- [x] 4.1 `LaunchLang` 與訊息對照表（design D6）：`launch_lang_from_langid` 純函式測試（六個中文 LANGID 為 `Zh`，英文、日文、
  `zh-SG`（`0x1004`）為 `En`）、訊息兩種語言測試、既有中文斷言不變。驗收＝三項 Rust gate 綠

## 5. 收斂

- [x] 5.1 `i18n-check.js` 全段 PASS；英文三種寬度截圖經 frontend-design 審核，採信的建議已修
- [x] 5.2 既有 14 支驗收腳本前景逐支跑，全部 PASS（偶發項依 handover 重跑一次並記錄）
- [x] 5.3 文件：`cockpit/README.md`（語言決定與切換、後端代碼清單、啟動器語言）、`README.md`、`README.zh-TW.md` 提到介面可切中英；
  驗收＝`markdownlint-cli2 "**/*.md"` 0 issues
- [x] 5.4 整支 Opus 審查（視同 Codex）通過；`openspec validate --all` 通過
