# ADR-0006 新增 `cockpit-files` crate，只放檔案瀏覽的純邏輯

- Status: Accepted
- Date: 2026-09-27

## Context

`file-review` change 要新增讀檔安全邊界（根目錄推算、相對路徑解析與界限檢查、列目錄、
中繼資料與 `viewer` 分類、icon 對照、Markdown 渲染），這段邏輯需要大量單元測試涵蓋
路徑與界限的邊界案例，但不需要起 HTTP 服務、不需要 HERDR 連線、也不需要投影
（design D1）。`cockpit` 已是 workspace 中最大的 crate；ADR-0003 定的依賴方向
（`herdr-client`、`cockpit-core` 不依賴其他 `cockpit-*`）保持不變，本 ADR 只新增一條邊界。

## Decision

```text
cockpit  →  cockpit-files
cockpit  →  cockpit-herdr  →  herdr-client
                           →  cockpit-core
cockpit  →  cockpit-core
```

- `cockpit-files`：純邏輯 crate，只依賴 `comrak`、`ignore`、`serde`／`serde_json`；不知道
  HTTP、HERDR、投影與 runtime 設定。對外提供根目錄推算、相對路徑解析與界限檢查、列目錄、
  中繼資料、icon 對照、Markdown 渲染。
- `cockpit`：唯一依賴 `cockpit-files` 的 crate，負責 pane 的 `cwd` 轉主機路徑（含 WSL 轉換）、
  允許清單、路由與錯誤對應、內嵌 vendored 前端資源。
- `cockpit-core`、`cockpit-herdr`、`herdr-client` 都不依賴 `cockpit-files`；ADR-0003 的邊界
  維持原樣。

## Consequences

- `cockpit-files` 的測試不需要啟動 HTTP 服務或連線 HERDR，可以純粹用暫存目錄跑安全邊界的
  單元測試，執行快、案例好寫。
- 之後 5b（git 唯讀層、diff、Git Graph）若要用同樣的模式，可以比照另開一個純邏輯 crate，
  不必回頭改 `cockpit` 的既有結構。
- 多一份 `Cargo.toml` 與樣板；`cargo tree -p cockpit-core` 等指令可直接驗證邊界沒有被打破。
