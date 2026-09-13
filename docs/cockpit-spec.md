# AI Agent Cockpit — Concept & Architecture Spec

> Status: Draft / 暫時定案  
> Date: 2026-09-10  
> Primary runtime: HERDR  
> Control plane: Rust  
> Working name: **AI Agent Cockpit / Agent Operations Console**

## 1. 文件目的

本文件記錄 2026-09-10 關於 AI coding agent 工作可視化的討論、思考演進、公開資料查證結果，以及目前暫時定案的產品與技術方向，作為後續實作 `AI Agent Cockpit` 的起始 specification。

本文件刻意保留「為什麼最後走到這個架構」的推理，而不只記錄最終 solution，避免日後實作時失去原始設計意圖。

---

## 2. 問題起點

最初問題：

> Codex / Claude Code 長時間自主工作時，能否另外建立一個 localhost HTML dashboard，監聽 agent 的工作進度並視覺化？這是否會造成大量額外 token 消耗？

核心需求並不是另一個 code editor，而是：

- 不必一直盯 terminal。
- 不必逐行閱讀 agent 產出的 code。
- 能快速知道 AI **現在正在做什麼**。
- 能知道工作流程目前「加工到哪一站」。
- 能辨識測試失敗、修改、重試、等待、完成等狀態。
- 多個 agents 平行工作時，仍能理解整體 project 的進度與依賴關係。

---

## 3. 第一個重要結論：Observer，不是 Supervisor LLM

### 不採用

讓另一個 LLM 每隔數秒詢問：

```text
What are you doing now?
Summarize your progress.
Estimate completion percentage.
```

這會：

1. 額外消耗 token。
2. 反覆讀取 context。
3. 干擾 coding agent 原本的 reasoning loop。
4. 產生不可靠的「70% 完成」等主觀估計。

### 採用

使用 **旁路 Observer / Event-driven observability**：

```text
Codex / Claude Code
        │
        │ tool / terminal / state events
        ▼
Local Observer
        │
        │ WebSocket / SSE
        ▼
Cockpit UI
```

Observer 本身不需要 LLM。

因此：

> Telemetry / task state / terminal projection 的額外 LLM token 成本應接近 0。

只有未來若加入 semantic summarization，才可能選擇性使用 LLM。

---

## 4. 從 Dashboard 到 Agent Cockpit

最初概念只是：

```text
Status
Current task
Files changed
Tests
Tokens
Terminal output
```

後來發現若持續加入：

- pipeline
- tasks
- agents
- git state
- tests
- live output
- workstreams
- parallel agents

產品會逐漸接近 IDE。

因此重新定位：

> **不要再做一個 IDE。**

Cockpit 的主要使用者不是「正在手寫 code 的工程師」，而是：

> **正在管理 AI 工作的人。**

傳統層次可理解為：

```text
VS Code
人寫 code
    ↓
Cursor-like IDE
人 + AI 寫 code
    ↓
Codex / Claude Code
AI 寫 code，人下指令
    ↓
Agent Cockpit
人管理 AI 的工作
```

Cockpit 不以 source editor 為核心。

需要人工深入時，再跳回 HERDR / VS Code / Cursor。

---

## 5. 初始視覺概念

最初 dashboard prototype 的設計重點：

- Dark futuristic UI
- Pipeline 橫向流動
- 每一種 message / event 有不同 visual identity
- Live Output 位於中央
- 左側 Project / Tasks
- 右側 Files / Tests / Token Usage
- Pipeline node 清楚區分 Done / Running / Pending
- 用「流水線加工」而非 chat conversation 表達 agent 工作

對應概念圖：

**`cockpit-dashboard-concept.png`**

此圖應與本 spec 一起保留，作為 UI design reference，而不是最終 UI specification。

---

## 6. HERDR 出現後的架構轉折

進一步構想：

> 不自己管理 Codex / Claude Code process，而以 **HERDR 作為底層 runtime**，Cockpit 只是 HERDR 外的一層 visual orchestration shell。

這使系統責任可以明確分離。

### HERDR

定位：

> **Agent Runtime / Execution Fabric**

負責：

- agent terminal/runtime
- workspace / space organization
- tabs / panes
- process lifecycle
- Codex / Claude Code 等 agent integration
- terminal output
- agent state
- runtime events

### Cockpit

定位：

> **Observability + Orchestration + Pipeline UI**

負責：

- Project
- Pipeline
- DAG
- Stage
- Workstream
- Task
- Runtime binding
- progress visualization
- dependency visualization
- multi-agent parallel collaboration visualization
- historical state projection

---

## 7. 已查證的 HERDR 能力

查證日期：**2026-09-10**

公開資料來源：

- HERDR Socket API documentation: <https://herdr.dev/docs/socket-api/>
- HERDR GitHub repository: <https://github.com/herdrdev/herdr>

查證到 HERDR 的 Raw Socket API 適合 custom tools、protocol clients 與 long-lived event subscribers。

公開 API 能力包含（實作前仍應依當時 schema 再確認）：

```text
session.snapshot

workspace.list
workspace.get

tab.list

pane.list
pane.get
pane.read
pane.process_info

agent.list
agent.get
agent.read
agent.prompt
agent.wait

events.subscribe
```

可觀察事件包括：

```text
workspace.created
workspace.updated

tab.created
tab.focused

pane.created
pane.updated
pane.closed
pane.exited

pane.agent_detected
pane.agent_status_changed

worktree.created
worktree.opened
worktree.removed
```

另有 reporting / metadata 類能力，例如：

```text
pane.report_metadata
```

這代表 Cockpit 不需要透過高頻 CLI polling 監控 HERDR，可以採：

```text
HERDR
   │
   │ event stream
   ▼
Rust Control Plane
   │
   ├─ State projection
   └─ WebSocket / SSE
          │
          ▼
       Web UI
```

HERDR 亦可輸出 API schema（實作時應再次確認目前 CLI syntax），可考慮用於 Rust protocol type validation / generation。

---

## 8. 暫時定案的總體架構

```text
                    ┌────────────────────────────┐
                    │       Pipeline UI          │
                    │                            │
                    │ Research ─┬─ Backend       │
                    │           ├─ Tests         │
                    │           └─ Docs          │
                    │                ↓           │
                    │         Integration        │
                    └─────────────┬──────────────┘
                                  │
                         WebSocket / SSE
                                  │
                    ┌─────────────▼──────────────┐
                    │    Rust Control Plane      │
                    │                            │
                    │ Pipeline Engine            │
                    │ Event Aggregator           │
                    │ HERDR Adapter              │
                    │ State Store                │
                    │ Message Classifier         │
                    └─────────────┬──────────────┘
                                  │
                         HERDR Socket API
                                  │
              ┌───────────────────▼───────────────────┐
              │                 HERDR                 │
              │                                       │
              │ Space A          Space B       Space C│
              │ ├ Claude         ├ Codex       ├ Codex│
              │ ├ Tests          ├ Claude       └ Shell│
              │ └ Shell          └ Shell               │
              └───────────────────────────────────────┘
```

核心原則：

> HERDR 負責「AI 在哪裡工作」；Cockpit 負責「這些工作在整體流程中代表什麼」。

---

## 9. Runtime Model 與 Domain Model 必須分離

### 錯誤方向

不要硬性規定：

```text
HERDR Workspace == Pipeline
HERDR Tab       == Stage
HERDR Pane      == Task
```

這會讓 Cockpit domain model 被 HERDR UI/runtime hierarchy 綁死。

### Runtime Model

```text
HERDR
├ Workspace / Space
├ Tab
├ Pane
└ Agent
```

### Cockpit Domain Model

```text
Project
├ Pipeline
│  ├ Stage
│  └ Dependency
├ Workstream
├ Task
├ Artifact
└ RuntimeBinding
```

透過 binding 連接：

```text
Task
 ├ runtime       = herdr
 ├ workspace_id  = ...
 ├ pane_id       = ...
 └ agent         = codex
```

Rust 草案：

```rust
struct TaskRuntimeBinding {
    runtime: RuntimeId,
    workspace_id: String,
    pane_id: String,
}
```

如此未來才能支援：

- 一個 stage 多個 HERDR spaces
- 一個 workstream 多個 agents
- agent 跨 stage 工作
- 甚至未來加入非 HERDR runtime adapter

---

## 10. Pipeline 從線性流程演化成 DAG

第一版視覺可以是：

```text
SPEC → PLAN → CODE → TEST → REVIEW → COMPLETE
```

但 domain model 應從一開始允許 DAG：

```text
                   ┌→ Research ────────────┐
                   │                       │
SPEC ─→ PLAN ──────┼→ Backend ─→ Tests ───┼→ Review
                   │                       │
                   └→ Frontend ─→ UI Test ─┘
```

Pipeline 不只是 progress bar，而是：

> **工作依賴圖 + runtime state projection。**

---

## 11. Spaces 作為第二維度：AI Factory Floor

HERDR 的 Spaces 概念可以讓 Cockpit 從單一 pipeline 進化為多維度視覺。

推薦 conceptual model：

- X 軸：Pipeline Stage
- Y 軸：Workstream / Space
- Node：Task / Agent
- Edge：Dependency / handoff

例如：

```text
                       PIPELINE STAGE
              Plan    Build    Test    Review

Research    ─── ● ───── ● ───── ○ ───── ○
Backend     ─── ✓ ───── ● ───── ○ ───── ○
Frontend    ─── ✓ ───── ✓ ───── ● ───── ○
Docs        ─── ✓ ───── ● ───── ○ ───── ○
```

更具體：

```text
                 AI FACTORY

           ┌──────── Research ●───────┐
           │                          │
Spec ✓ → Plan ✓ → Backend ● → Test ○ ├→ Review ○
           │                          │
           └──────── Frontend ●───────┘
```

這是 Cockpit 與一般「多 terminal agent manager」最重要的差異之一。

Cockpit 不只是告訴使用者：

> 有 5 個 agent 正在跑。

而是：

> 哪些 agent 正在什麼 workstream、哪個 pipeline stage 工作，彼此如何依賴，以及哪條線阻塞了整體交付。

---

## 12. Live Output 的定位

Dashboard 中央的 Live Output：

```text
22:14:12 TEST   Running pytest
22:14:13 TOOL   bash pytest tests/
22:14:16 TEST   test_auth.py PASSED
22:14:18 ERROR  test_integration.py FAILED
22:14:19 AGENT  Analyzing failure...
22:14:20 READ   tests/test_integration.py
```

**不應成為另一套 terminal runtime。**

它只是：

> **HERDR terminal / agent output 的 projection。**

資料可來自 HERDR 的 pane / agent read capabilities。

流程：

```text
User selects Agent Node
        ↓
RuntimeBinding
        ↓
HERDR pane / agent
        ↓
read output
        ↓
ANSI / event parsing
        ↓
Live Output UI
```

第一版甚至可以只忠實呈現 output。

Message classification：

```text
AGENT
TOOL
READ
EDIT
TEST
ERROR
GIT
INFO
```

可以在第二階段逐步加入。

---

## 13. Event → State Projection

HERDR 知道的是 runtime state：

```text
agent = working
pane = exited
```

Cockpit 要表達的是 domain state：

```text
Backend / Implementation = Running
Integration Test = Failed
Review = Blocked
```

因此不能直接把 HERDR status 當 Pipeline status。

應採：

```text
HERDR Event
      ↓
RuntimeBinding
      ↓
Task
      ↓
Pipeline Engine
      ↓
Projected Domain State
      ↓
UI
```

例如：

```text
HERDR:
pane.agent_status_changed = working

Binding:
pane w2:p4 → task backend-auth

Domain:
backend-auth belongs to IMPLEMENT stage

Projection:
IMPLEMENT = Running
```

---

## 14. Pipeline State 草案

```rust
enum StageStatus {
    Pending,
    Ready,
    Running,
    Blocked,
    Failed,
    Completed,
}
```

Task：

```rust
struct Task {
    id: TaskId,

    pipeline_id: PipelineId,
    stage_id: StageId,
    workstream_id: WorkstreamId,

    runtime_binding: Option<TaskRuntimeBinding>,

    status: StageStatus,
}
```

Project state：

```rust
struct AppState {
    spaces: HashMap<SpaceId, SpaceState>,
    pipelines: HashMap<PipelineId, Pipeline>,
    agents: HashMap<PaneId, AgentState>,
}
```

實際 type 應在 implementation design 階段再正規化。

---

## 15. Runtime Adapter

Cockpit 不應讓 UI 或 Pipeline Engine 直接依賴 HERDR protocol。

抽象：

```rust
trait AgentRuntime {
    async fn snapshot(&self) -> Result<RuntimeSnapshot>;
    async fn subscribe(&self) -> Result<EventStream>;
    async fn read_output(&self, pane: PaneId) -> Result<String>;
    async fn prompt(&self, pane: PaneId, prompt: String) -> Result<()>;
}
```

第一個 implementation：

```rust
struct HerdrRuntime {
    socket_path: PathBuf,
}
```

好處：

```text
Cockpit Domain
      │
AgentRuntime
      │
 ┌────┴─────┐
HERDR      Future Runtime
```

即使 MVP 只支援 HERDR，也不要把 domain types 寫成 HERDR types。

---

## 16. Rust 技術方向

目前指定語言：**Rust**

這個選擇合理，尤其：

- HERDR 本身是 Rust project。
- event-driven daemon 很適合 Tokio。
- long-lived socket / WebSocket 很適合 async Rust。
- single local binary deployment 很符合 local cockpit。
- memory / concurrency safety 適合長時間監控多 agent。

暫定 stack：

```text
Rust
├ tokio
├ axum
├ serde
├ serde_json
├ tracing
├ WebSocket / SSE
└ SQLite / sqlx     # 非 MVP 必要，可後加
```

前端第一版可以保持簡單：

```text
Rust/Axum
    ↓
static HTML/CSS/JS
    +
WebSocket/SSE
```

暫時沒有必要導入大型 frontend framework。

---

## 17. Token 成本原則

Cockpit 的 telemetry layer 應遵守：

> **Observe behavior, not Chain-of-Thought.**

### Token ≈ 0 的資訊

- agent state
- process state
- tool execution
- terminal output
- files changed
- test output
- git state
- stage mapping
- runtime event
- elapsed time

### 可能消耗 token 的功能

未來 optional：

- semantic progress summary
- 自動判斷 task meaning
- 自動建立 pipeline
- failure explanation
- cross-agent semantic synthesis

這些都不應是 MVP 的必要條件。

禁止使用「每 N 秒 prompt agent 回報進度」作為主要 telemetry mechanism。

---

## 18. 不讀取 Chain-of-Thought

Cockpit 不應嘗試顯示或攔截 agent 私有 reasoning。

UI 關注：

```text
Input
Action
Tool
Artifact
State transition
Output
Failure
Retry
Result
```

而不是：

```text
Hidden reasoning / Chain-of-Thought
```

產品哲學：

> **監控 Agent 的行為與產出，而不是監控它腦中在想什麼。**

---

## 19. Cockpit UI 核心資訊架構

暫定主要區塊：

### Project

- project name
- overall state
- elapsed time
- branch/worktree
- optional aggregate progress

### Pipeline / Factory Floor

- stages
- workstreams
- task nodes
- dependencies
- active agent
- blocked / failed state
- parallel activity

### Live Output

依 message type 視覺區分：

```text
AGENT
TOOL
READ
EDIT
TEST
ERROR
GIT
INFO
```

### Tasks

- pending
- running
- blocked
- failed
- completed

### Files / Artifacts

- modified files
- created files
- reports
- test artifacts

### Tests

- passed
- failed
- skipped
- currently running

### Runtime

- HERDR connection
- active spaces
- agents
- panes
- process state

### Usage

如果底層 agent/runtime 有可靠資料來源，再顯示：

- input tokens
- output tokens
- context usage

不可為了 dashboard 而額外呼叫 LLM 推估 token。

---

## 20. MVP Scope

第一版刻意保持 observer-first。

### 必做

1. Connect HERDR Raw Socket API.
2. Initial runtime snapshot.
3. Subscribe HERDR events.
4. Discover Spaces / workspaces / panes / agents.
5. Maintain in-memory projected state.
6. 定義 Project / Pipeline / Stage / Workstream / Task。
7. 建立 Task ↔ HERDR runtime binding。
8. Browser localhost dashboard。
9. Pipeline / Factory Floor visualization。
10. Agent live state。
11. 點擊 agent/task 顯示 HERDR Live Output。
12. Running / Done / Failed / Blocked 等視覺狀態。
13. Event-driven UI update。

### 可以簡化

- Pipeline config 可以先由 YAML/TOML/JSON 手動定義。
- Workstream ↔ Space mapping 可以先人工設定。
- State history 第一版只保存在 memory。
- message classifier 可以只做 deterministic rules。

---

## 21. MVP 明確不做

第一版不要做：

- ❌ 重做 IDE
- ❌ source code editor
- ❌ 重做 terminal emulator/runtime
- ❌ fork HERDR
- ❌ 修改 Codex
- ❌ 修改 Claude Code
- ❌ 自己做另一套 agent framework
- ❌ planner agent
- ❌ supervisor agent
- ❌ router agent
- ❌ memory agent
- ❌ 自動 semantic progress LLM polling
- ❌ Chain-of-Thought capture
- ❌ 強迫 Workspace = Pipeline
- ❌ 強迫 Pane = Task
- ❌ 一開始就做完整 autonomous orchestration

原則：

> **先看懂 AI factory，再決定要不要控制 AI factory。**

---

## 22. Phase 2

MVP 穩定後：

### Interaction

- prompt agent
- pause / resume
- retry failed task
- assign task
- spawn agent（若 HERDR API 適合）

### Pipeline

- task dependencies
- DAG scheduler
- blocked propagation
- automatic readiness
- handoff visualization

### History

- SQLite event log
- replay
- timeline
- task duration
- failure history

### UI

- Space view
- Pipeline view
- Agent view
- Timeline view
- Artifact view

---

## 23. Phase 3

較長期才考慮：

### Semantic layer

Optional LLM：

- summarize current state
- explain blocker
- classify unknown output
- generate pipeline from project spec
- summarize cross-agent results

### Multi-runtime

```text
AgentRuntime
├ HERDR
├ Remote HERDR
└ Other future runtimes
```

### Orchestration

Cockpit 可能逐步從：

```text
Observability
```

演進到：

```text
Observability
     +
Orchestration
```

但 orchestration 必須建立在穩定 runtime projection 上。

---

## 24. Reliability / Safety Principles

### Source of truth

Runtime facts：

> HERDR

Pipeline facts：

> Cockpit domain state

不要混淆。

### Event loss

Cockpit reconnect 後應：

1. 重新取得 snapshot。
2. reconciliation。
3. 再繼續 subscribe events。

不可假設 event stream 永遠完整。

### Unknown states

無法映射時：

```text
Unknown
```

不要自行猜測 Completed。

### Commands

未來加入 prompt / spawn / terminate 等控制功能時：

- read-only action 與 mutating action 明確區分。
- destructive action 需要 UI confirmation。
- logging command origin。
- runtime adapter 負責 capability checking。

---

## 25. Open Questions

後續 implementation 前需要確認：

1. HERDR 最新版本中 Space 與 Workspace 的正式 domain terminology 與 API mapping。
2. Raw Socket API 的 transport 在 Windows / macOS / Linux 各自如何處理。
3. HERDR event 是否包含足夠穩定的 agent status。
4. pane.read 的 streaming / incremental strategy。
5. terminal ANSI parsing 是否直接沿用現成 Rust crate。
6. worktree 與 project/workstream 的最佳 mapping。
7. pipeline config format：TOML / YAML / JSON？
8. 是否需要第一版就保存 event history。
9. HERDR metadata 是否適合保存 Cockpit task/stage binding，或只作 display metadata。
10. 如何避免 Cockpit metadata 與 HERDR native state 產生 ownership conflict。
11. token/context usage 是否能從各 agent integration 得到可靠 telemetry。
12. UI 的 Space / Workstream 是否允許 many-to-many mapping。

---

## 26. Acceptance Criteria — MVP

當以下 scenario 成立，MVP 可視為成功：

### Scenario A — Attach

```text
Given HERDR 正在執行
And HERDR 中已有多個 Spaces / agents

When Cockpit 啟動

Then Cockpit 自動連接 HERDR
And 取得目前 runtime snapshot
And 顯示所有已知 agent
```

### Scenario B — Live state

```text
Given Codex 正在某 HERDR pane 工作

When HERDR agent status 改變

Then Cockpit 在不額外 prompt Codex 的情況下
即時更新該 agent/task 狀態
```

### Scenario C — Pipeline projection

```text
Given Task A 綁定某 HERDR pane
And Task A 屬於 IMPLEMENT stage

When 該 agent 進入 working state

Then UI 顯示 IMPLEMENT / Task A = Running
```

### Scenario D — Parallel collaboration

```text
Given Backend、Frontend、Tests 位於不同 workstreams
And 各自綁定不同 HERDR agents

When 多個 agents 同時工作

Then Factory Floor 同時呈現各 workstream 的 active node
And 使用者可以理解各自所在 pipeline stage
```

### Scenario E — Live Output

```text
Given 使用者點擊某 agent/task

When Cockpit 取得該 HERDR pane output

Then Live Output 顯示對應 terminal/agent activity
And 不啟動第二個 LLM summarizer
```

### Scenario F — Reconnect

```text
Given Cockpit 與 HERDR socket 暫時斷線

When connection 恢復

Then Cockpit 重新 snapshot + reconcile
And 不因漏掉 events 而錯誤宣告 task 完成
```

---

## 27. 暫時定案

截至 2026-09-10，方向暫定：

### Product

<!-- markdownlint-disable-next-line MD036 -->
**AI Agent Cockpit / Agent Operations Console**

不是 IDE，而是：

> **Visual orchestration and observability layer for HERDR.**

### Runtime

<!-- markdownlint-disable-next-line MD036 -->
**HERDR**

不 fork。

### Backend / Control Plane

<!-- markdownlint-disable-next-line MD036 -->
**Rust**

預計：

```text
Tokio + Axum + Serde + HERDR Raw Socket API
```

### Architecture

```text
Web Cockpit
     ↓
Rust Control Plane
     ↓
AgentRuntime abstraction
     ↓
HERDR Adapter
     ↓
HERDR
     ↓
Codex / Claude Code / Shell / ...
```

### UI model

不是單一 chat，也不是 terminal grid。

核心視覺：

> **Pipeline × Workstream × Space × Agent**

形成多維度的 **AI Factory Floor**。

### MVP philosophy

> Observer first, orchestrator later.

先做到：

```text
HERDR 正常使用
      ↓
Cockpit 自動 attach
      ↓
Spaces / Agents 自動出現
      ↓
Pipeline 即時映射
      ↓
平行工作可視化
      ↓
點 Node 看 Live Output
```

在這個基礎可靠之後，再加入 agent control、DAG scheduling 與 semantic AI layer。

---

## 28. 一句話產品定義

> **Cockpit 是建立在 HERDR 之上的 Rust-based AI Agent Operations Console，把原本散落在 Spaces、panes 與 terminals 中的 Codex / Claude Code 活動，投影成可觀察、可追蹤、可理解的多維度 Pipeline / AI Factory Floor。**
