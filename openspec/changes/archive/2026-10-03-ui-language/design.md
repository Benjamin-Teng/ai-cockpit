# design：ui-language

## Context

動機與範圍見 `proposal.md`；行為契約見 `specs/ui-language/spec.md`。現況（2026-10-03 盤點）：

- 前端沒有任何多語機制。`index.html` 以 8 個 classic `<script>`（各自是 IIFE，以 `window.*` 互通）依序載入
  `output.js → viewers.js → git.js → files.js → notify.js → render.js → actions.js → channel.js`；每個檔由
  `cockpit/src/http.rs` 的 `app_asset()` 以白名單 `include_str!` 內嵌，沒有 build step。
- 約 196 條繁中字面值（git.js 118、files.js 48、render.js 35、viewers.js 26、notify.js 19、output.js 7、index.html 8、
  actions.js 2 句），其中約 30 處是「前半＋變數＋後半」拼接。很多字串在模組載入時就定死（`ERROR_TEXT` 表、`*_TEXT` 常數、
  通知設定面板只建一次）。
- 後端原文顯示的訊息約 45 條：`Rejection`（8）、`WriteError`（6）、進度與 Live Output 端點自身（約 9）、連線 `reason` 與
  `protocol_warning`（約 15，同一欄位本來就混著 `herdr-client` 的英文 thiserror）、project 警告（1）。檔案與 git 端點已用
  `code` 查前端 `ERROR_TEXT`，不需要動後端。
- 啟動器訊息框約 33 條（`cockpit/src/bin/cockpit-launch.rs`、`cockpit/src/launch.rs`）。
- 宣傳頁 `site/` 已有同一套語言規則的實作可參考（`site/index.html` 的 head 腳本）。

## Goals / Non-Goals

**Goals：**

- 一份字典、一個查詢函式，所有介面文字經它取得；英文介面不殘留繁中字典字串。
- 後端只新增欄位，不改既有欄位與狀態碼；既有測試與驗收腳本在繁中預設下不受影響。

**Non-Goals：**

- 不做熱切換、不引入 build step 或第三方 i18n 函式庫、不翻終端機與日誌訊息。

## Decisions

### D1 字典放在新的 `i18n.js`，第一個載入

`cockpit/assets/app/i18n.js` 定義 `window.cockpitI18n = { lang, t(key, params), setLang(lang) }`，內含 `zh` 與 `en` 兩份
物件字典，鍵以模組為前綴（`render.*`、`files.*`、`git.*`、`viewers.*`、`output.*`、`notify.*`、`actions.*`、`index.*`、
`msg.*` 給後端代碼）。`t()` 以 `{name}` 具名佔位符代入參數；鍵不存在時回傳鍵本身並 `console.warn` 一次（開發期能發現漏字，
畫面也不會空白）。`index.html` 在其他腳本之前載入它；`http.rs` 白名單、`index.html` 的 `<script>` 順序與路由測試同步更新。

- **替代方案：每個模組各自帶字典**。鍵分散在 8 個檔，難以檢查兩種語言是否對齊；不採用。
- **替代方案：第三方 i18n 函式庫**。專案沒有 build step、前端全是內嵌 classic script，引入函式庫得 vendor 一份並處理載入，
  需求只有兩種語言與具名佔位符，自寫 30 行即可（鐵則 KISS）；不採用。

### D2 語言在 `i18n.js` 載入時決定，和宣傳頁同一套規則

規則見 spec。`i18n.js` 是第一個執行的腳本，所以其他模組在載入時讀到的 `t()` 已是正確語言；模組層級的常數表（`ERROR_TEXT`
等）可以照舊在載入時建立，只是內容改由 `t()` 取得。`index.html` 的靜態文字以 `data-i18n`（文字）與 `data-i18n-attr`（屬性）
標記，由 `i18n.js` 套用。`i18n.js` 放在 `<head>`（task 5.1，審查 Stage1 M1：避免英文使用者先看到一幀繁中）：語言與 `<html lang>`
在第一次繪製前決定，繁中以外的語言先以 `i18n-pending` 隱藏待換的靜態節點，套用完成（或逾時保險）後才顯示。

### D3 切換一律重新載入頁面

模組載入時定死的字串與只建一次的通知面板很多，熱切換容易漏改；頁面狀態（開著的分頁、通知設定、選取的 pane）本來就存在
`localStorage` 並在載入時還原。所以切換＝寫 `cockpit.lang`＋`location.reload()`；其他視窗監聽 `storage` 事件的
`cockpit.lang` 變化後同樣 reload。按鈕由 `render.js` 在頂列繪製（與鈴鐺同一區，每次重畫重建），點擊經 `actions.js` 的
`data-action` 分派，與鈴鐺同一套機制。

### D4 後端訊息代碼：錯誤本體加 `params`，投影加 `*_msg` 欄位

- **錯誤本體**：沿用 `coded_error_response` 的 `{"error", "code"}`，新增可省略的 `params`（字串對字串）。代碼一律 snake_case：
  `Rejection` 各變體 → `already_last_stage`、`already_first_stage`、`already_marked`、`task_not_in_workstream`、
  `runtime_not_registered`、`runtime_not_connected`、`pane_not_found`、`pane_exited`；`WriteError` → `unknown_project`、
  `unknown_task`、`unknown_workstream`（參數 `id`）、`persist_failed`（參數 `detail`）、`pane_not_bound`（既有）、
  `internal_error`；進度端點的非法操作 → `invalid_op`（參數 `op`）；Live Output → `runtime_not_found`（參數 `runtime`）、
  `read_timeout`、`pane_gone`（參數 `pane`）、`output_read_failed`（參數 `detail`）。已存在的 `code`（例如 `forbidden_source`、
  `pane_not_bound`）不改名。實作時以 `Rejection::code()` 等方法集中定義，不在 handler 裡散寫字串。
- **投影**：`cockpit-core` 新增 `MessageCode { code: String, params: BTreeMap<String, String> }`（序列化為
  `{"code", "params"}`）。連線狀態的 `reason` 旁加 `reason_msg`、`protocol_warning` 旁加 `protocol_warning_msg`、project 的
  `warnings` 旁加等長的 `warning_msgs`。代碼例：`wsl_distro_not_running`（參數 `distro`）、`wsl_probe_failed`（`detail`）、
  `snapshot_failed`（`detail`）、`protocol_untested`（`protocol`、`tested`）、`event_stream_ended`、
  `task_stage_reset`（`task`、`stage`、`start`）、無法歸類的原文 → `raw`（`text`）。完整清單於實作 task 中定案並寫進
  `cockpit/README.md`。原文欄位照舊產生，兩者由同一處建構，避免不一致。
- **前端**：`t("msg." + code, params)` 有對應鍵就用；沒有就顯示原文欄位。參數值（pane id、distro、HERDR 原文）一律原樣代入，
  不翻譯。

- **替代方案：後端依 `Accept-Language` 直接回英文**。投影是 `/ws` 對所有視窗廣播的同一份 JSON，不同視窗可能是不同語言；
  而且使用者選擇存在瀏覽器端，後端不知道；不採用。
- **替代方案：前端把中文原文對映成英文**。後端一改字就靜默失效（專案 memory「前端複算後端規則」同一類機制）；不採用。

### D5 「done 不是完成」的禁字守門改到字典

`cockpit/tests/http.rs` 原本斷言 `render.js` 原始碼不含「完成」、`notify-check.js` 斷言通知面板文字不含「完成」。字串搬進字典後，
這兩個守門要改成檢查字典：描述 HERDR `done` 的鍵（集中在 `render.agentDone*`、`notify.kind.done*` 等前綴，實作時定案）繁中
不得含「完成」、英文不得含 `complete`／`completed`／`finished`；以 Rust 測試讀 `i18n.js` 原始碼檢查。

### D6 啟動器以 `GetUserDefaultUILanguage` 決定語言

`cockpit/src/launch.rs` 新增 `LaunchLang { Zh, En }` 與 `fn launch_lang_from_langid(langid: u16) -> LaunchLang`（純函式、可測）：
主要語言 `LANG_CHINESE`（`0x04`）且完整 LANGID 為 `0x0404`（台灣）、`0x0C04`（香港）、`0x1404`（澳門）、`0x7C04`（繁體）、
`0x0804`（中國）、`0x0004`（簡體）時為 `Zh`，其他為 `En`。`GetUserDefaultUILanguage` 以 `unsafe extern "system"` 宣告
（與既有的 `MessageBoxW` 同一做法，不新增相依套件）；非 Windows 建置固定 `En`。訊息改由 `fn text(lang, LaunchText) -> String`
提供兩種語言；既有測試以 `Zh` 斷言原本的中文內容不變。

### D7 驗收

- 新增 `docs/research/2026-10-03/i18n-check.js`（Node、headless Chrome）：① 解析 `i18n.js` 檢查兩份字典鍵與佔位符一致；
  ② 以 `cockpit.lang=en` 開 `ui_preview`，逐一開啟 spec 列出的畫面，收集所有文字節點與 `title`／`aria-label`，斷言不含任何繁中
  字典值；③ 語言判斷四個 scenario（以 CDP 設定時區與 `--lang`，並直接在頁面上驗判斷函式處理 `zh-SG`、`zh-Hant-SG` 等
  Chrome `--lang` 會改寫的值）；④ 切換按鈕與兩個視窗的同步；⑤ 英文介面 1536／1100／700 三種寬度截圖，交 frontend-design 審核。
- 既有 14 支驗收腳本：啟動 Chrome 時一律帶 `--lang=zh-TW`（task 5.2），第一順位語言即決定繁中，不依賴執行機器的時區；照常全部通過。

## Risks / Trade-offs

- 英文字串較長，頂列燈號、按鈕、分頁標題與 Factory Floor 欄位都有 ellipsis 與固定寬高設計；以 D7 ⑤ 的截圖與設計審核處理，
  必要時縮短英文用字而不是改版面。
- 驗收腳本在英文模式下只有 `i18n-check.js` 覆蓋；既有腳本只驗繁中。英文模式的互動行為與繁中共用同一份程式碼，只差字串，
  風險可接受。
- `reason` 欄位的 HERDR 英文原文在繁中介面照舊夾英文（不翻 HERDR 原文），與現況相同。
