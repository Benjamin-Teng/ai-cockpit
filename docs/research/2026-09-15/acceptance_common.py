"""change 1b 真機驗收腳本（task 4.1–4.3）共用工具：起停 cockpit、讀 /api/state、取兩側
`herdr api snapshot`、對 WSL 端測試 server 送 JSON-RPC，以及「投影 vs snapshot」逐欄位比對。

比對規則照 `cockpit-herdr/src/translate.rs`（snapshot → RuntimeSnapshot）與
`cockpit-core/src/projection.rs`（RuntimeStore → ProjectedState）：
- workspace：id 集合相等；每個 workspace 的 label（空字串視為 null）、number、agent_status、focused 相等；
- tab：每個 workspace 底下的 tab id 集合 = snapshot 中 workspace_id 相同的 tab；number、agent_status、focused 相等
  （TabInfo.label 翻譯時丟棄，不比）；
- pane：每個 tab 底下的 pane id 集合 = snapshot 中 tab_id 相同的 pane；agent、agent_status、title、cwd、
  label（空字串視為 null）、focused 相等，exited 必須是 false（snapshot 沒有這欄，翻譯固定 false；
  terminal_title、revision、scroll 翻譯時丟棄，不比）；
- focused：投影的 focused.{workspace_id,tab_id,pane_id} = snapshot 的 focused_*_id。
不符時的描述只印 id 與欄位名，路徑類欄位（cwd、title、label）不印值，維持去識別化。
"""

from __future__ import annotations

import io
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from collections.abc import Callable
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
EXE = REPO / "target" / "release" / ("cockpit.exe" if os.name == "nt" else "cockpit")
API = "http://127.0.0.1:7770/api/state"
WSL_WRITE_OPT_IN = "HERDR_CLIENT_TEST_ALLOW_WSL_WRITES"
REDACTED_FIELDS = {"cwd", "title", "label"}
failures: list[str] = []


def check(cond: bool, label: str) -> bool:
    print(("ok   " if cond else "FAIL ") + label)
    if not cond:
        failures.append(label)
    return cond


def configure_stdout() -> None:
    if isinstance(sys.stdout, io.TextIOWrapper):  # Windows 主控台預設 cp950
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")


def api() -> dict | None:
    try:
        with urllib.request.urlopen(API, timeout=2) as r:
            return json.load(r)
    except (urllib.error.URLError, OSError, ValueError):
        return None


def load_config() -> dict:
    """讀 repo 根的 cockpit.toml（stdlib tomllib）。"""
    import tomllib

    with open(REPO / "cockpit.toml", "rb") as f:
        return tomllib.load(f)


def wsl_runtime(cfg: dict) -> dict:
    """設定檔中第一筆帶 `wsl` 的 runtime。"""
    return next(r for r in cfg["runtime"] if "wsl" in r)


def runtime(state: dict, rid: str) -> dict:
    return next(r for r in state["runtimes"] if r["id"] == rid)


def wsl_bash(distro: str, script: str, timeout: float = 60) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["wsl.exe", "-d", distro, "-e", "bash", "-lc", script],
        capture_output=True, encoding="utf-8", errors="replace", timeout=timeout, check=False,
    )


def wsl_rpc(distro: str, socket: str, rid: str, method: str, params: dict) -> dict:
    """對 WSL 端 HERDR 送一行 JSON-RPC、讀一行回應（server 回應後關連線，nc 自行結束）。"""
    line = json.dumps({"id": rid, "method": method, "params": params}) + "\n"
    r = subprocess.run(
        ["wsl.exe", "-d", distro, "-e", "nc", "-U", socket],
        input=line, capture_output=True, encoding="utf-8", errors="replace", timeout=30, check=False,
    )
    out = (r.stdout or "").strip().splitlines()
    if not out:
        raise RuntimeError(f"{method} 沒有回應（stderr: {r.stderr.strip()[:200]}）")
    return json.loads(out[0])


def herdr_snapshot(runtime_cfg: dict) -> dict:
    """該 runtime 那一側的 `herdr api snapshot`（唯讀）：Windows 直接跑，WSL 經 wsl.exe。"""
    if "wsl" in runtime_cfg:
        cmd = ["wsl.exe", "-d", runtime_cfg["wsl"]["distro"], "-e", "bash", "-lc", "~/.local/bin/herdr api snapshot"]
    else:
        cmd = ["herdr", "api", "snapshot"]
    out = subprocess.run(cmd, capture_output=True, encoding="utf-8", errors="replace", timeout=30, check=True).stdout
    return json.loads(out.strip().splitlines()[0])["result"]["snapshot"]


def wsl_server_status(distro: str) -> dict | None:
    """WSL 端 `herdr status server --json`；沒在跑或輸出不是 JSON 物件時回 None。"""
    r = wsl_bash(distro, "~/.local/bin/herdr status server --json", timeout=30)
    try:
        data = json.loads((r.stdout or "").strip())
    except ValueError:
        return None
    return data if isinstance(data, dict) else None


def start_cockpit() -> subprocess.Popen[bytes]:
    if api() is not None:
        raise SystemExit("127.0.0.1:7770 已有服務在跑，請先停掉")
    return subprocess.Popen([str(EXE)], cwd=REPO, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def stop_cockpit(proc: subprocess.Popen[bytes]) -> None:
    """只終止自己 spawn 的那個 PID（含子程序），並確認 API 不再回應。"""
    if proc.poll() is None:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(proc.pid), "/T", "/F"], capture_output=True, check=False)
        else:
            proc.kill()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            check(False, "cockpit 未在 10 s 內結束")
    check(api() is None, "cockpit 已停止（API 不再回應）")


def wait_for(pred: Callable[[dict], bool], timeout: float, interval: float = 0.1) -> tuple[dict | None, float]:
    """輪詢 /api/state 直到 pred(state) 為真；回傳 (state 或 None, 經過秒數)。"""
    t0 = time.time()
    while time.time() - t0 < timeout:
        st = api()
        if st is not None and pred(st):
            return st, time.time() - t0
        time.sleep(interval)
    return None, time.time() - t0


def all_connected(state: dict) -> bool:
    return all(r["connection"]["state"] == "connected" for r in state["runtimes"])


def panes_of(rt: dict) -> dict[str, dict]:
    return {p["id"]: p for w in rt["workspaces"] for t in w["tabs"] for p in t["panes"]}


def _label(value: str | None) -> str | None:
    return value if value else None


def _diff_fields(diffs: list[str], subject: str, actual: dict, expected: dict) -> None:
    for field, want in expected.items():
        got = actual.get(field)
        if got != want:
            if field in REDACTED_FIELDS:
                diffs.append(f"{subject} {field}：投影與 snapshot 不同（值不印）")
            else:
                diffs.append(f"{subject} {field}：投影 {got!r} vs snapshot {want!r}")


def compare_projection(rt: dict, snap: dict) -> list[str]:
    """投影的一筆 runtime 對 `herdr api snapshot` 逐欄位比對；回傳不符描述清單（空＝一致）。"""
    diffs: list[str] = []
    ws_p = {w["id"]: w for w in rt["workspaces"]}
    ws_s = {w["workspace_id"]: w for w in snap["workspaces"]}
    tabs_s = {t["tab_id"]: t for t in snap["tabs"]}
    panes_s = {p["pane_id"]: p for p in snap["panes"]}
    if set(ws_p) != set(ws_s):
        diffs.append(f"workspace id 集合：投影 {sorted(ws_p)} vs snapshot {sorted(ws_s)}")
    for wid in sorted(set(ws_p) & set(ws_s)):
        w, s = ws_p[wid], ws_s[wid]
        _diff_fields(diffs, f"workspace {wid}", w, {
            "label": _label(s.get("label")), "number": s["number"],
            "agent_status": s["agent_status"], "focused": s["focused"],
        })
        tabs_p = {t["id"]: t for t in w["tabs"]}
        want_tabs = {tid for tid, t in tabs_s.items() if t["workspace_id"] == wid}
        if set(tabs_p) != want_tabs:
            diffs.append(f"workspace {wid} tab id 集合：投影 {sorted(tabs_p)} vs snapshot {sorted(want_tabs)}")
        for tid in sorted(set(tabs_p) & want_tabs):
            t, st = tabs_p[tid], tabs_s[tid]
            _diff_fields(diffs, f"tab {tid}", t, {
                "number": st["number"], "agent_status": st["agent_status"], "focused": st["focused"],
            })
            panes_p = {p["id"]: p for p in t["panes"]}
            want_panes = {pid for pid, p in panes_s.items() if p["tab_id"] == tid}
            if set(panes_p) != want_panes:
                diffs.append(f"tab {tid} pane id 集合：投影 {sorted(panes_p)} vs snapshot {sorted(want_panes)}")
            for pid in sorted(set(panes_p) & want_panes):
                p, sp = panes_p[pid], panes_s[pid]
                # snapshot 沒有 exited 欄位，translate::pane 固定給 false；投影若殘留 PaneExited 的 true 就是不一致。
                _diff_fields(diffs, f"pane {pid}", p, {
                    "agent": sp.get("agent"), "agent_status": sp["agent_status"], "title": sp.get("title"),
                    "cwd": sp.get("cwd"), "label": _label(sp.get("label")), "focused": sp["focused"],
                    "exited": False,
                })
    _diff_fields(diffs, "focused", rt["focused"], {
        "workspace_id": snap.get("focused_workspace_id"), "tab_id": snap.get("focused_tab_id"),
        "pane_id": snap.get("focused_pane_id"),
    })
    return diffs


def summarize(rt: dict) -> str:
    panes = panes_of(rt)
    return (f"workspaces={len(rt['workspaces'])} tabs={sum(len(w['tabs']) for w in rt['workspaces'])} "
            f"panes={len(panes)} agents={sorted(p['agent'] for p in panes.values() if p.get('agent'))}")


def result() -> int:
    if failures:
        print(f"RESULT: FAIL ({len(failures)})")
        return 2
    print("RESULT: PASS")
    return 0
