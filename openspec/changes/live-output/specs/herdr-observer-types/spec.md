# herdr-observer-types（delta）

## MODIFIED Requirements

### Requirement: pane.read 型別（供 change 3）

系統必須提供 `pane.read` 的參數（`pane_id`、`source`、`format`、`lines`、`strip_ansi`）與
結果（`pane_id`、`workspace_id`、`tab_id`、`source`、`format`、`text`、`revision`、
`truncated`）型別，`source` 值域為 `visible`、`recent`、`recent_unwrapped`、`detection`
（設計文件 §2.6），`format` 值域為 `text`、`ansi`（schema `ReadFormat`），兩者都不得序列化出值域外的
字串。結果型別必須能解析真機回應。`revision` 照 schema 收下，但它在已測版本（HERDR 0.8.2、0.9.0）恆為 0、
內容改變時也不變（`docs/research/2026-09-19/pane-read-probe.md` 第 2 節），文件必須註明呼叫端不得拿它
判斷內容是否改變。

#### Scenario: 序列化 pane.read request

- **WHEN** 序列化 `source` 為 `recent`、`lines` 為 200 的 `pane.read` request
- **THEN** 通過兩份 schema 的 `request` 根 schema

#### Scenario: 解析真機 pane.read 回應

- **GIVEN** 一份從真機 HERDR 擷取的 `pane.read` 成功回應 fixture（內容為測試用的已知文字）
- **WHEN** 解析為結果型別
- **THEN** 解析成功，`text` 等於該已知文字，`source`、`format`、`truncated` 與 fixture 一致
