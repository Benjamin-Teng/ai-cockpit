# ADR-0001 Observer first：MVP 對 HERDR 完全唯讀

- Status: Accepted
- Date: 2026-09-13

## Context

spec §3、§17、§21 已排除「用另一個 LLM 定期詢問 agent 進度」與「一開始就做
orchestration」。HERDR 的 socket API 沒有認證，任何連上的程式都能下 `agent.prompt`、
`server.stop` 等會改變狀態的指令；其中 `herdr server stop` 會殺掉所有 pane。

## Decision

change 1 只呼叫 `session.snapshot` 與 `events.subscribe`。`herdr-client` 的 observer
子集不包含任何寫入 method 的型別。Phase 2 要加寫入時，`AgentRuntime` 以 capability
宣告，且 UI 對 destructive 動作要求確認並記錄來源。

## Consequences

- Cockpit 不可能因為 bug 誤觸 HERDR 狀態。
- 「任務完成」無法由 HERDR 狀態推得，必須由 Cockpit 規則或人工給。
- 真機測試不得用 `herdr server stop` 製造斷線。
