# tasks：ws-source-check

> 執行路徑：直接 apply（不走 SDD）｜理由：只在 `cockpit` 路由表加兩個 `route_layer` 與對應測試、文件，任務線性且範圍小；
> 品質 gate 與審查（本專案 Opus 視同 Codex）照常。

通則：Rust 修改以 TDD 進行（先紅後綠，記錄證據）；改 `.rs` 跑 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、
`cargo test --workspace`；改 `.md` 在 repo 根跑 `markdownlint-cli2 "**/*.md"`；跑瀏覽器腳本前確認 7770 沒人用（不是自己開的程序
不准砍），一律前景逐支跑。

## 1. 先寫會失敗的測試

- [x] 1.1 `cockpit/tests/http.rs`：`/api/state` 對 `Host: evil.example:<port>`、外站 `Origin`、重複 `Host` 回 403 且 `code` 為
  `forbidden_source`、本體不含投影；同源與只帶 `Host: localhost:<port>` 回 200。`cockpit/tests/ws.rs`：外站 `Origin` 升級 `/ws`
  回 403、沒有升級；同源 `Origin` 照常收到現況。`cockpit/tests/idle_exit.rs`：唯一連線關閉後只有被拒的 `/ws`／`/api/state`
  請求時，後端照常在閒置期限到時結束。驗收＝新測試 FAIL（RED 記錄），其餘照常通過

## 2. 實作

- [x] 2.1 `cockpit/src/http.rs`：`/api/state`、`/ws` 加 `route_layer(source_check)`（design D1），更新路由表與
  `cockpit/src/source_check.rs` 的模組文件；驗收＝1.1 全綠，三項 Rust gate 全綠
- [x] 2.2 文件：`cockpit/README.md` 來源檢查說明補上兩條路由；`README.md`、`README.zh-TW.md` 移除「尚未檢查」的揭露
  （`site/index.html`、`site/i18n.js` 查過本來就沒有這段揭露，未變更）；驗收＝`markdownlint-cli2 "**/*.md"` 0 issues

## 3. 審查修正（fix round 1）

- [x] 3.0 I1 `real_attach.rs` 回填實際埠；I2 `listen` 驗證收緊（`config.rs`、`cockpit-config` delta、README、範例設定）；M1 `GET /`
  防嵌入標頭（design D4）；M2 `desktop-launch` delta；M3／nit：補 HEAD `/api/state`、`/ws` 壞 Host／重複 Host 測試與 doc 措辭

## 4. 收斂

- [x] 4.1 既有 12 支驗收腳本＋`notify-check.js`、`idle-exit-check.js` 前景逐支跑，全部 PASS（偶發項依 handover 重跑一次並記錄）
- [x] 4.2 Opus 審查（視同 Codex）通過；`openspec validate --all` 通過
