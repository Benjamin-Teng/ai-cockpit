"""change 1b task 4.1 Scenario A（Attach）自動化部分。

用法（repo 根，先 `cargo build --release -p cockpit`，並確認 Windows 端 HERDR 與 WSL 端測試 server 都在跑）：
    uv run --no-project python docs/research/2026-09-15/attach-check.py

做的事（全程對 HERDR 唯讀）：
1. 以 repo 根的 cockpit.toml 啟動 target/release/cockpit.exe（自己 spawn，結束時依 PID 終止）。
2. 輪詢 http://127.0.0.1:7770/api/state 直到設定檔內每個 runtime 的 connection.state 都是 connected（上限 60 s）。
3. 對每個 runtime 取該側 `herdr api snapshot`（Windows 直接跑；WSL 經 wsl.exe），用 acceptance_common.compare_projection
   逐欄位比對 workspace／tab／pane／focused（規則見該模組 docstring）。
4. 印出去識別化摘要（只有 id、數量、狀態，不印路徑），任一比對不符就非零結束。
"""

from __future__ import annotations

import sys

import acceptance_common as ac


def main() -> int:
    ac.configure_stdout()
    cfg = ac.load_config()
    runtimes = cfg["runtime"]
    proc = ac.start_cockpit()
    try:
        state, dt = ac.wait_for(lambda s: ac.all_connected(s) and len(s["runtimes"]) == len(runtimes), 60, interval=0.5)
        if state is None:
            state = ac.api()
        ac.check(state is not None, "/api/state 有回應")
        if state is None:
            return 2
        print("version", state["version"], "runtimes", [(r["id"], r["connection"]["state"]) for r in state["runtimes"]], f"({dt:.1f}s)")
        for r in state["runtimes"]:
            ac.check(r["connection"]["state"] == "connected", f"runtime {r['id']} 為 connected")
            if r["connection"]["state"] == "connected":
                conn = r["connection"]
                print(f"  {r['id']}: server {conn['server_version']} protocol {conn['protocol']} warning {conn['protocol_warning']}")
        for rt_cfg, rt in zip(runtimes, state["runtimes"]):
            ac.check(rt_cfg["id"] == rt["id"], f"runtime 順序與設定一致（{rt_cfg['id']}）")
            snap = ac.herdr_snapshot(rt_cfg)
            print(f"  {rt['id']}: projection {ac.summarize(rt)}")
            print(f"  {rt['id']}: herdr      workspaces={len(snap['workspaces'])} tabs={len(snap['tabs'])} panes={len(snap['panes'])}")
            diffs = ac.compare_projection(rt, snap)
            for d in diffs:
                print("    diff:", d)
            ac.check(not diffs, f"{rt['id']} 投影與 herdr api snapshot 逐欄位一致（workspace／tab／pane／focused）")
    finally:
        ac.stop_cockpit(proc)
    return ac.result()


if __name__ == "__main__":
    sys.exit(main())
