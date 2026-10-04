# AGENTS.md

給所有 coding agent（Claude Code、Codex）的最短指引。詳細規則在下列權威文件，不在這裡複製。

## 先讀

- 架構設計：`docs/superpowers/specs/2026-09-13-cockpit-mvp-design.md`
- 詞彙表：`CONTEXT.md`
- 決策理由：`docs/adr/`
- 目前進度與坑：`docs/handover.md`
- 計畫層：`openspec/`（`openspec status --change <slug>` 看進度）

## 品質 gate（0 error 才算完成）

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
markdownlint-cli2 "**/*.md"    # 在 repo 根執行
```

推送任何分支或對 `main` 開 PR 時，GitHub Actions 的 `.github/workflows/ci.yml` 會跑同一組 gate，另加
`cargo test -p cockpit --example ui_preview` 與 `openspec validate --all`。本機綠不等於 CI 綠，推送後看結果。

## 給 review 用的零寫入指令

Codex adversarial review 跑在唯讀沙箱，cargo 的 build、clippy、test 都要寫 `target/`，
在沙箱裡一定失敗，這不是環境壞掉。唯讀環境只能跑：

```bash
cargo fmt --check
```

其餘 findings 以靜態推導提出，由執行者實測後才採信。

## 硬性約束

- 對 HERDR 完全唯讀：只送唯讀 method，可送的 method 由 `herdr-client` 的 sealed `Request`
  trait 在編譯期限定（清單見 `herdr-client/src/client/request.rs`）。
  測試鷹架對 WSL 端測試 server 的 tab create／report_agent 需以
  `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1` 明確 opt-in，預設唯讀。
- Windows 端不得用 `herdr server stop` 測試斷線，因為會殺掉使用者所有 pane。WSL 端 headless
  測試 server 可以停，重啟指令見 `docs/research/2026-09-19/live-output-acceptance.md`。
- `cockpit-core` 不得依賴 `herdr-client` 或任何 `cockpit-*`；`cockpit` 不直接依賴
  `herdr-client`（ADR-0003）。
- HERDR 的 `done` 不是任務完成；沒有全域 agent 狀態訂閱，只能每 pane 各訂一筆。
