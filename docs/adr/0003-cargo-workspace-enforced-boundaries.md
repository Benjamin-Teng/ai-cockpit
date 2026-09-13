# ADR-0003 四個 crate 的 cargo workspace，邊界由依賴方向強制

- Status: Accepted
- Date: 2026-09-13

## Context

spec §9 與 §15 要求 Cockpit 的 domain model 不得依賴 HERDR 型別，否則換 runtime 或
HERDR 改版時整個 Cockpit 跟著壞。單一 crate 分 module 時，這條線只靠 review 守。

## Decision

```text
cockpit  →  cockpit-herdr  →  herdr-client
                           →  cockpit-core
cockpit  →  cockpit-core
```

- `herdr-client`：只懂 HERDR 協定與 transport，不依賴任何 `cockpit-*`。
- `cockpit-core`：Cockpit 自己的 runtime 模型、`AgentRuntime` trait、狀態庫、投影；
  不依賴 `herdr-client`。change 2 的 domain 也放這裡。
- `cockpit-herdr`：唯一同時看得到兩邊的 crate，負責翻譯與連線迴圈。
- `cockpit`：程式本體與網頁。

## Consequences

- 誰在 `cockpit-core` 引用 HERDR 型別，編譯直接失敗。
- 多幾份 `Cargo.toml` 與樣板；每個 crate 小而專一，AI agent 改碼時較不迷路。
- `herdr-client` 可獨立重用。
