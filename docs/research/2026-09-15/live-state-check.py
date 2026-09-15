"""change 1b task 4.2 Scenario B（Live state）的 WSL 端自動化部分。

用法（repo 根，先 `cargo build --release -p cockpit`；Windows 端 HERDR 與 WSL 端測試 server 都在跑）：
    HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 uv run --no-project python docs/research/2026-09-15/live-state-check.py

未設定 opt-in 環境變數時只印說明就結束（對 HERDR 唯讀）。設定後只對 WSL 端 headless 測試 server 的
workspace wD 做：tab.create → pane.report_agent（working → blocked → idle）→ pane.clear_agent_authority
→ tab.close，全程與 1a 真機測試鷹架相同的 JSON-RPC（經 wsl.exe -e nc -U，一連線一 method）。
每一步都觀察 cockpit 的 /api/state：pane 出現（驗 L 事件與 ReopenStatus）、狀態變化的延遲、
recent_events 出現 pane.agent_status_changed、idle 是呈現 idle 還是 done、關 tab 時的事件序列。

清理（finally，不論成敗）：重新取 WSL 端 `herdr api snapshot`，把 workspace wD 裡 label 等於本腳本專用
標記、且不在起始 baseline 的 tab 全部 tab.close——即使 tab.create 的回應遺失也能收掉；清不掉就列出
殘留 id 並非零結束。輸出只含 id、狀態、時間，不含路徑。
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time

import acceptance_common as ac

WORKSPACE = "wD"
SOURCE = "cockpit_4_2"  # tab label 與 report_agent source 共用的唯一標記


def event_kinds(state: dict, pane_id: str | None = None) -> list[str]:
    evs = state["recent_events"]
    if pane_id:
        evs = [e for e in evs if e.get("pane_id") == pane_id]
    return [e["kind"] for e in evs]


SNAPSHOT_ERRORS = (subprocess.SubprocessError, ValueError, KeyError, OSError)


def marked_tabs(wsl_cfg: dict, attempts: int = 3) -> set[str]:
    """WSL 端 snapshot 裡 workspace wD 下、label 為本腳本標記的 tab id；snapshot 取不到時最多重試 attempts 次。"""
    last: Exception | None = None
    for i in range(attempts):
        try:
            snap = ac.herdr_snapshot(wsl_cfg)
            return {t["tab_id"] for t in snap["tabs"] if t["workspace_id"] == WORKSPACE and t.get("label") == SOURCE}
        except SNAPSHOT_ERRORS as e:
            last = e
            if i + 1 < attempts:
                time.sleep(1.0)
    raise RuntimeError(f"WSL 端 snapshot 連續 {attempts} 次取不到：{last}")


def close_tab(distro: str, socket: str, tid: str) -> bool:
    try:
        resp = ac.wsl_rpc(distro, socket, f"cleanup:{tid}", "tab.close", {"tab_id": tid})
        print(f"cleanup: tab.close {tid} →", resp.get("result", {}).get("type") or resp.get("error"))
        return resp.get("result", {}).get("type") == "ok"
    except (RuntimeError, ValueError, subprocess.SubprocessError, OSError) as e:
        print(f"cleanup: tab.close {tid} 失敗：{e}")
        return False


def cleanup_tabs(wsl_cfg: dict, baseline_tabs: set[str], known_tab: str | None) -> None:
    """不論主流程成敗都執行：已知 tab id 先直接關，再用 snapshot 掃 label 補漏；snapshot 一直取不到就
    誠實回報「可能殘留」與人工清理方式並算失敗，不宣稱清乾淨。"""
    distro, socket = wsl_cfg["wsl"]["distro"], wsl_cfg["wsl"]["socket"]
    if known_tab is not None:
        close_tab(distro, socket, known_tab)
    try:
        leftover = marked_tabs(wsl_cfg) - baseline_tabs
    except RuntimeError as e:
        ac.check(False, f"cleanup：{e}；可能殘留 label={SOURCE} 的 tab，請在 WSL 端 herdr api snapshot 找到後以 tab.close 手動關閉")
        return
    for tid in sorted(leftover):
        close_tab(distro, socket, tid)
    try:
        residual = marked_tabs(wsl_cfg) - baseline_tabs
    except RuntimeError as e:
        ac.check(False, f"cleanup：清理後無法重新確認（{e}）；已嘗試關閉 {sorted(leftover)}，請人工確認")
        return
    ac.check(not residual, f"cleanup：WSL 端沒有殘留的測試 tab（殘留：{sorted(residual)}）")


def main() -> int:
    ac.configure_stdout()
    if os.environ.get(ac.WSL_WRITE_OPT_IN) != "1":
        print(f"未設定 {ac.WSL_WRITE_OPT_IN}=1：不對 WSL 端寫入，直接結束（唯讀）。")
        return 0
    cfg = ac.load_config()
    wsl_cfg = ac.wsl_runtime(cfg)
    distro, socket = wsl_cfg["wsl"]["distro"], wsl_cfg["wsl"]["socket"]
    wsl_id = wsl_cfg["id"]

    # 寫入前先記 baseline：之後只清理「帶本腳本標記且不在 baseline」的 tab。
    baseline_tabs = marked_tabs(wsl_cfg)
    if baseline_tabs:
        print(f"注意：起始就有帶標記 {SOURCE} 的 tab {sorted(baseline_tabs)}（上次殘留？），本次不會動它們。")

    proc = ac.start_cockpit()
    tab_id: str | None = None
    try:
        st, _ = ac.wait_for(ac.all_connected, 60)
        ac.check(st is not None, "兩筆 runtime 皆 connected")
        if st is None:
            return 2
        baseline = set(ac.panes_of(ac.runtime(st, wsl_id)))
        print(f"baseline wsl panes: {sorted(baseline)}")

        # 1. 建 tab（帶一個 root pane）→ cockpit 應在 L 事件後把新 pane 放進投影，並重開 S。
        created = ac.wsl_rpc(distro, socket, "tc", "tab.create", {"workspace_id": WORKSPACE, "label": SOURCE, "focus": False})
        tab_id = created["result"]["tab"]["tab_id"]
        pane_id = created["result"]["root_pane"]["pane_id"]
        print(f"created tab {tab_id} pane {pane_id}")
        st, dt = ac.wait_for(lambda s: pane_id in ac.panes_of(ac.runtime(s, wsl_id)), 5)
        ac.check(st is not None, f"新 pane {pane_id} 在 {dt:.2f}s 內出現在投影（L 事件 → PaneUpserted）")
        # ReopenStatus 有 200 ms 去抖動：新 pane 的 S 訂閱要等重開完成才會收到狀態事件；真實 agent 不會在
        # pane 建立後 200 ms 內就變狀態，這裡等 0.5 s 再開始回報，讓觀察對應真實情境。
        time.sleep(0.5)

        # 2. 狀態變化：working → blocked → idle；每一步量延遲，並確認最近事件有 pane.agent_status_changed。
        observed: list[tuple[str, str, float]] = []
        for state_name, accept in [("working", {"working"}), ("blocked", {"blocked"}), ("idle", {"idle", "done"})]:
            resp = ac.wsl_rpc(distro, socket, "rep", "pane.report_agent", {"pane_id": pane_id, "source": SOURCE, "agent": "claude", "state": state_name})
            ac.check(resp.get("result", {}).get("type") == "ok", f"pane.report_agent({state_name}) 回 ok")
            st, dt = ac.wait_for(lambda s, a=accept: ac.panes_of(ac.runtime(s, wsl_id)).get(pane_id, {}).get("agent_status") in a, 3)
            shown = ac.panes_of(ac.runtime(st, wsl_id))[pane_id]["agent_status"] if st else None
            observed.append((state_name, str(shown), dt))
            ac.check(st is not None and dt < 1.0, f"report {state_name} → 投影顯示 {shown}，延遲 {dt:.2f}s（<1s；驗 ReopenStatus 讓新 pane 的狀態即時到達）")
            if st is not None:
                ac.check("pane.agent_status_changed" in event_kinds(st, pane_id), f"最近事件含 pane.agent_status_changed（{pane_id}）")
        print("status sequence (reported → shown, latency):", observed)
        st = ac.api()
        if st is not None:
            kinds = event_kinds(st)
            print("recent event kinds (all runtimes, newest first):", kinds[:20])
            print("workspace_updated seen during status changes:", "workspace_updated" in kinds)

        # 3. 釋放 agent → 觀察狀態（spike 3：release 會再推一筆 unknown）。
        resp = ac.wsl_rpc(distro, socket, "rel", "pane.clear_agent_authority", {"pane_id": pane_id, "source": SOURCE})
        ac.check(resp.get("result", {}).get("type") == "ok", "pane.clear_agent_authority 回 ok")
        st, dt = ac.wait_for(lambda s: ac.panes_of(ac.runtime(s, wsl_id)).get(pane_id, {}).get("agent_status") not in {"idle", "done"}, 3)
        after_release = ac.panes_of(ac.runtime(st, wsl_id))[pane_id]["agent_status"] if st else "(unchanged)"
        print(f"after release: agent_status={after_release} ({dt:.2f}s)")

        # 4. 關 tab → pane 應從投影消失；記錄關 tab 時的事件序列（design D5：有沒有 pane_closed）。
        before_close = ac.api()
        before_kinds = {json.dumps(e, sort_keys=True) for e in before_close["recent_events"]} if before_close else set()
        resp = ac.wsl_rpc(distro, socket, "tclose", "tab.close", {"tab_id": tab_id})
        if ac.check(resp.get("result", {}).get("type") == "ok", "tab.close 回 ok"):
            tab_id = None  # 已關成功；finally 仍會用 snapshot 掃一次 label 補漏
        st, dt = ac.wait_for(lambda s: pane_id not in ac.panes_of(ac.runtime(s, wsl_id)), 5)
        ac.check(st is not None, f"關 tab 後 pane {pane_id} 在 {dt:.2f}s 內從投影消失")
        if st is not None:
            new_events = [e for e in st["recent_events"] if json.dumps(e, sort_keys=True) not in before_kinds]
            print("events after tab.close (newest first):", [(e["kind"], e.get("tab_id") or e.get("pane_id") or e.get("workspace_id")) for e in new_events])
            ac.check(pane_id not in ac.panes_of(ac.runtime(st, wsl_id)), "pane 已不在投影（連鎖刪除）")
        # 5. 收尾狀態：投影必須在限時內與 WSL 端重新取得的 snapshot 逐欄位一致。HERDR 關 tab 後可能自行重建其他
        #    pane（0.8.2 會把 Sidebar pane 換一個 id）、也可能補推舊 tab／pane 的事件（→ Drift → 重拿），所以
        #    不要求 pane 集合等於 baseline，而是輪詢到一致為止並記錄花了多久；上限取定期重拿週期（30 s）加餘裕。
        t_close = time.time()
        last: tuple[str, ...] | None = None
        converged_at: float | None = None
        final = None
        while time.time() - t_close < 35:
            final = ac.api()
            if final is None:
                time.sleep(0.5)
                continue
            diffs = ac.compare_projection(ac.runtime(final, wsl_id), ac.herdr_snapshot(wsl_cfg))
            if tuple(diffs) != last:
                print(f"  t+{time.time() - t_close:.1f}s version={final['version']} diffs={len(diffs)}")
                for d in diffs:
                    print("    diff:", d)
                last = tuple(diffs)
            if not diffs:
                converged_at = time.time() - t_close
                break
            time.sleep(0.5)
        ac.check(converged_at is not None, f"關 tab 後投影在 {converged_at if converged_at is not None else '>35'}s 內與 WSL 端重新取得的 snapshot 逐欄位一致（workspace／tab／pane／focused）")
        if final is not None:
            after = set(ac.panes_of(ac.runtime(final, wsl_id)))
            print(f"pane set delta vs baseline: removed={sorted(baseline - after)} added={sorted(after - baseline)}")
            drifts = [(e["at"], e["detail"]) for e in final["recent_events"] if e["kind"] == "drift" and e["runtime"] == wsl_id]
            print("wsl drift events in recent_events (at, reason):", drifts)
            ac.check(ac.runtime(final, "win")["connection"]["state"] == "connected", "Windows 端全程 connected")
    finally:
        try:
            cleanup_tabs(wsl_cfg, baseline_tabs, tab_id)
        finally:
            ac.stop_cockpit(proc)
    return ac.result()


if __name__ == "__main__":
    sys.exit(main())
