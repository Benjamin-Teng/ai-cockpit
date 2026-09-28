# design：html-charset

## Context

動機見 `proposal.md`；行為契約見本 change 的 `specs/file-review/spec.md`。現況：原始內容端點先以中繼資料取得界限與 viewer
分類，再以有上限的讀取（change 5a 的 `read_capped`，archive `2026-09-28-file-review` 的 design D3 與 sdd-ledger task 2.6）
把整個檔案讀進記憶體後回傳；`.html`／`.htm` 的 content-type 是寫死的 `text/html`。HTML 檢視器以 iframe `src` 載入這個端點
（同 archive 的 design D8、裁決 R30），因此 iframe 的解碼只受這個回應標頭與檔案內宣告影響。

## Goals / Non-Goals

**Goals：**

- 只動原始內容回應的 content-type 決定方式；端點其他行為（界限、上限、錯誤碼、CSP、nosniff）不變。

**Non-Goals：**

- 不在前端做任何處理（不改 `viewers.js`）。

## Decisions

### D1 依內容決定是否帶 charset，而不是一律帶

`.html`／`.htm` 的回應位元組是合法 UTF-8 → `text/html; charset=utf-8`；否則 → 不帶 charset 的 `text/html`。

- **替代方案：一律帶 `charset=utf-8`**。HTTP 標頭的 charset 優先於檔內 `<meta charset>`，已正確宣告 Big5 的舊檔會因此
  變成亂碼。使用者 2026-09-28 選定依內容決定。
- **替代方案：偵測 Big5 等其他編碼並轉碼或標示**。需要編碼偵測函式庫、判斷不保證正確，且非 UTF-8 的 HTML 本來就能靠
  檔內宣告正確顯示；不做。
- 只含 ASCII 的檔案也是合法 UTF-8，會帶上 `charset=utf-8`；ASCII 在 UTF-8 與常見舊編碼中位元組相同，顯示不受影響。
- UTF-8 BOM 屬於合法 UTF-8，照樣帶 charset；UTF-16 BOM 的檔案不是合法 UTF-8，不帶 charset，瀏覽器依 BOM 判斷。

### D2 檢查「實際回傳的位元組」，不用中繼資料的前 8192 位元組

判斷放在有上限讀取之後、以同一份位元組做完整的 UTF-8 驗證。

- 中繼資料的 viewer 分類只看前 8192 位元組、容許結尾截斷；拿它當依據，檔案後段若有非 UTF-8 位元組就會誤標 charset。
- 同一份位元組也避免「判斷之後檔案被改寫」的不一致（與 change 5a 對大小上限的處理相同）。
- 成本：原始內容上限 50 MiB，完整 UTF-8 驗證是線性掃描，與讀檔同量級，可忽略。

## Risks / Trade-offs

- [非 UTF-8 的 HTML 剛好整份位元組都是合法 UTF-8] → 只有極短、或只用到兩種編碼重疊範圍的內容才可能發生；此時多半只剩 ASCII，
  顯示不受影響。接受。
- [既有測試斷言 `.html` 為 `text/html`] → spec 已改變，依新 scenario 更新斷言，並在 commit 訊息逐條列出。
