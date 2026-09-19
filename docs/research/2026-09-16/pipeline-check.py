"""change `pipeline-projection` task 7.2：真機 Scenario C／D（WSL 端）自動化驗收。

用法（repo 根，先 `cargo build --release -p cockpit`，並先重啟 WSL 端 headless 測試 server 清補推積壓）：
    HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 uv run --no-project python docs/research/2026-09-16/pipeline-check.py

未設定 opt-in 環境變數時只印說明就結束。設定後**只對 WSL 端測試 server** 寫入（Windows 端完全不碰：暫存設定
只含 WSL runtime）：
1. `workspace.create`（label 為本腳本專用標記）→ `pane.split` 出共四個 pane → `pane.rename` 三個 pane 為
   `backend`／`frontend`／`tests`，第四個為 `extra`（覆蓋目標，不符合任何 binding）。
2. `tests` pane 先 report working，再以暫存目錄中的設定（一個 project、三條以 workspace 標籤＋`pane_label`
   綁定的 workstream、四個 task）與暫存狀態檔啟動 release 版 cockpit（`--config`）。嚴格時限：啟動後
   6 s 內（spec 沉降上限 5 s＋1 s 量測餘裕）三條 bound(auto)、qa1 running，之後 10 s 不倒退。
3. Scenario C／D：JSON-RPC `pane.report_agent` 讓 backend／frontend 變 working，對應 task 3 s 內
   `running`，記錄秒數、10 s 不倒退；驗 binding JSON 逐欄位、各 task 的目前 Stage。
4. HTTP 推進／標記（帶合法 `Host`）：204／409／403 與 version 遞增／不遞增、依賴解除、狀態檔內容。
5. 設覆蓋 → 停 cockpit（依 PID）→ 重啟 → 進度與覆蓋保留；嚴格時限：重啟後 6 s 內 be2／fe1 running、
   10 s 不倒退。
6. `pane.close` 覆蓋指向的 pane → 覆蓋失效（投影回到自動解析、狀態檔中覆蓋被刪）；3 s 內其餘 task
   running 且 10 s 不倒退（觀察 S 訂閱重開是否重播）。
7. 再 `pane.split` 一次：記錄新 pane id 是否重用剛關掉的 id（design Risks），並同樣觀察 10 s 不倒退。
未在時限內成立時印出完整時間線（每次投影變化的 version、task status、binding），並繼續記錄到實際穩定為止
（只作診斷，判定仍是 FAIL）。
清理（finally）：`workspace.close` 本腳本建立的 workspace（另以 snapshot 掃標記補漏）、依 PID 停 cockpit、
刪暫存目錄。輸出只含 id、狀態、秒數，不含路徑。
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "2026-09-15"))
import acceptance_common as ac

WS_LABEL = "cockpit-7-2"  # workspace 標籤＝本腳本專用標記
SOURCE = "cockpit_7_2"  # report_agent 的 source
PORT = 7770
HOST = f"127.0.0.1:{PORT}"
BASE = f"http://{HOST}"
PROJECT = "p"
LABELS = {"backend": "backend", "frontend": "frontend", "tests": "tests"}  # workstream → pane label
EXTRA_LABEL = "extra"
CONNECT_DEADLINE = 6.0  # 啟動／重啟：spec「連線後沉降重拿」上限 5 s＋1 s 量測餘裕
STAGES = ["Plan", "Implement", "Test"]
TASKS = [  # (id, workstream, stage, depends_on)
    ("be1", "backend", "Implement", []),
    ("fe1", "frontend", "Plan", []),
    ("qa1", "tests", "Test", []),
    ("be2", "backend", "Plan", ["be1"]),
]


def rpc(wsl: dict, method: str, params: dict) -> dict:
    return ac.wsl_rpc(wsl["distro"], wsl["socket"], method, method, params)


def ok(resp: dict) -> bool:
    return "result" in resp and "error" not in resp


def write_config(tmp: Path, wsl: dict) -> tuple[Path, Path]:
    state_path = tmp / "cockpit.state.json"
    lines = [
        "[server]",
        f'listen = "{HOST}"',
        "",
        "[[runtime]]",
        'id = "wsl"',
        'kind = "herdr"',
        f"wsl = {{ distro = '{wsl['distro']}', socket = '{wsl['socket']}' }}",
        "",
        "[state]",
        f"path = '{state_path}'",
        "",
        "[[project]]",
        f'id = "{PROJECT}"',
        'name = "Task 7.2"',
        "stages = " + json.dumps(STAGES),
        "",
    ]
    for ws, label in LABELS.items():
        lines += [
            "[[project.workstream]]",
            f'id = "{ws}"',
            f'binding = {{ runtime = "wsl", workspace = "{WS_LABEL}", pane_label = "{label}" }}',
            "",
        ]
    for tid, ws, stage, deps in TASKS:
        lines += [
            "[[project.task]]",
            f'id = "{tid}"',
            f'workstream = "{ws}"',
            f'stage = "{stage}"',
            "depends_on = " + json.dumps(deps),
            "",
        ]
    cfg = tmp / "cockpit.toml"
    cfg.write_text("\n".join(lines), encoding="utf-8")
    return cfg, state_path


def start_cockpit(cfg: Path, tmp: Path, n: int) -> tuple[subprocess.Popen[bytes], Path]:
    if ac.api() is not None:
        raise SystemExit(f"{HOST} 已有服務在跑，請先停掉")
    log = tmp / f"cockpit-{n}.log"
    with open(log, "wb") as f:
        proc = subprocess.Popen([str(ac.EXE), "--config", str(cfg)], cwd=tmp, stdout=f, stderr=subprocess.STDOUT)
    return proc, log


def http(method: str, path: str, body: dict | None = None, headers: dict | None = None) -> tuple[int, dict | None]:
    data = json.dumps(body).encode() if body is not None else None
    h = {"Host": HOST}
    if body is not None:
        h["Content-Type"] = "application/json"
    h.update(headers or {})
    req = urllib.request.Request(BASE + path, data=data, method=method, headers=h)
    try:
        with urllib.request.urlopen(req, timeout=5) as r:
            raw = r.read()
            return r.status, (json.loads(raw) if raw else None)
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, (json.loads(raw) if raw else None)
        except ValueError:
            return e.code, {"raw": raw.decode("utf-8", "replace")[:200]}


def api_now() -> dict:
    """/api/state 此刻必須有回應；沒有就視為 cockpit 異常，直接中止（finally 會清理）。"""
    st = ac.api()
    if st is None:
        raise RuntimeError("/api/state 沒有回應（cockpit 可能已結束）")
    return st


def project(state: dict) -> dict:
    return next(p for p in state["projects"] if p["id"] == PROJECT)


def task(state: dict, tid: str) -> dict:
    return next(t for t in project(state)["tasks"] if t["id"] == tid)


def binding(state: dict, ws: str) -> dict:
    return next(w for w in project(state)["workstreams"] if w["id"] == ws)["binding"]


def settle(pred, timeout: float, stable: float, interval: float = 0.1) -> tuple[dict | None, float | None, float | None, list[float]]:
    """輪詢到 pred 成立且連續 stable 秒不變為止。回傳 (最後狀態, 第一次成立秒數, 穩定成立起點秒數, 倒退時刻清單)。

    WSL 0.8.2 在新訂閱時會重播歷史事件（見驗收文件 task 7.2 小節），投影可能「先對、再倒退、定期重拿後
    才回來」；倒退時刻逐一記錄，不當作通過。
    """
    t0 = time.time()
    first: float | None = None
    since: float | None = None
    regressions: list[float] = []
    st = None
    while time.time() - t0 < timeout:
        st = ac.api()
        now = time.time() - t0
        good = st is not None and pred(st)
        if good:
            first = now if first is None else first
            since = now if since is None else since
            if now - since >= stable:
                return st, first, since, regressions
        elif since is not None:
            regressions.append(round(now, 2))
            since = None
        time.sleep(interval)
    return None, first, since, regressions


def fmt(v: float | None) -> str:
    return "-" if v is None else f"{v:.2f}"


def summary(s: dict | None) -> str:
    """時間線用的一行摘要：各 task status 與各 workstream binding（state／pane／source／agent_status）。"""
    if s is None or not s.get("projects"):
        return "(no state)"
    ts = {t["id"]: t["status"] for t in project(s)["tasks"]}
    bs = {w["id"]: "/".join(str(w["binding"].get(k, "")) for k in ("state", "pane_id", "source", "agent_status"))
          for w in project(s)["workstreams"]}
    conn = ",".join(r["connection"]["state"] for r in s["runtimes"])
    return f"v={s['version']} conn={conn} tasks={ts} bindings={bs}"


def strict_hold(label: str, pred, t0: float, deadline: float = 3.0, hold: float = 10.0, interval: float = 0.1) -> bool:
    """嚴格時限判定（task 6.6 修正後）：自 t0 起 deadline 秒內 pred 成立，之後持續 hold 秒每次取樣都成立。

    倒退逐筆印出時刻與摘要（時間線證據）。未在期限內成立時另以 settle() 追 45 s 記錄實際收斂時間，只作診斷，
    不影響判定。
    """
    first: float | None = None
    last_summary = None
    timeline: list[str] = []
    while time.time() - t0 < deadline:
        st = ac.api()
        now = time.time() - t0
        cur = summary(st)
        if cur != last_summary:
            timeline.append(f"    t+{now:.2f} {cur}")
            last_summary = cur
        if st is not None and pred(st):
            first = now
            break
        time.sleep(interval)
    if first is None:
        # 診斷：繼續記錄時間線直到 pred 成立並穩定 2 s（上限 45 s），不影響判定。
        since: float | None = None
        while time.time() - t0 < 45:
            st = ac.api()
            now = time.time() - t0
            cur = summary(st)
            if cur != last_summary:
                timeline.append(f"    t+{now:.2f} {cur}")
                last_summary = cur
            if st is not None and pred(st):
                since = now if since is None else since
                if now - since >= 2.0:
                    break
            else:
                since = None
            time.sleep(interval)
        print(f"TIMELINE {label}（未在 {deadline:.0f}s 內成立；診斷：{fmt(since)}s 起成立並穩定）:")
        print("\n".join(timeline))
        ac.check(False, f"{label}：{deadline:.0f}s 內成立（實際 {fmt(since)}s 才成立）")
        return False
    regressions: list[str] = []
    t_hold = time.time()
    while time.time() - t_hold < hold:
        st = ac.api()
        if st is None or not pred(st):
            regressions.append(f"    t+{time.time() - t0:.2f} {summary(st)}")
        time.sleep(interval)
    print(f"OBSERVE {label}: first={first:.2f}s hold={hold:.0f}s regressions={len(regressions)}")
    print(f"TIMELINE {label}（成立前）:")
    print("\n".join(timeline))
    if regressions:
        print("\n".join(regressions[:8]))
    return ac.check(first <= deadline and not regressions,
                    f"{label}：{first:.2f}s 內成立（≤{deadline:.0f}s），之後 {hold:.0f}s 無倒退（倒退樣本 {len(regressions)}）")


def read_state_file(path: Path) -> dict | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def marked_workspaces(wsl_cfg: dict) -> set[str]:
    snap = ac.herdr_snapshot(wsl_cfg)
    return {w["workspace_id"] for w in snap["workspaces"] if w.get("label") == WS_LABEL}


def cleanup_workspaces(wsl_cfg: dict, wsl: dict, known: str | None) -> None:
    targets: set[str] = {known} if known else set()
    try:
        targets |= marked_workspaces(wsl_cfg)
    except (subprocess.SubprocessError, ValueError, KeyError, OSError) as e:
        print(f"cleanup：snapshot 取不到（{e}），只關已知 workspace")
    for wid in sorted(targets):
        try:
            resp = rpc(wsl, "workspace.close", {"workspace_id": wid})
            print(f"cleanup: workspace.close {wid} →", resp.get("result", {}).get("type") or resp.get("error"))
        except (RuntimeError, ValueError, subprocess.SubprocessError, OSError) as e:
            print(f"cleanup: workspace.close {wid} 失敗：{e}")
    try:
        residual = marked_workspaces(wsl_cfg)
    except (subprocess.SubprocessError, ValueError, KeyError, OSError) as e:
        ac.check(False, f"cleanup：清理後無法確認（{e}）；請在 WSL 端找 label={WS_LABEL} 的 workspace 手動關閉")
        return
    ac.check(not residual, f"cleanup：WSL 端沒有殘留 label={WS_LABEL} 的 workspace（殘留：{sorted(residual)}）")


def stop(proc: subprocess.Popen[bytes] | None, log: Path | None) -> None:
    if proc is None:
        return
    crashed = proc.poll() is not None
    ac.stop_cockpit(proc)
    if crashed and log is not None and log.exists():
        print(f"cockpit 提前結束（exit {proc.returncode}），日誌最後幾行（去識別化前請勿外貼）：")
        print("\n".join(log.read_text(encoding="utf-8", errors="replace").splitlines()[-10:]))


def main() -> int:
    ac.configure_stdout()
    if os.environ.get(ac.WSL_WRITE_OPT_IN) != "1":
        print(f"未設定 {ac.WSL_WRITE_OPT_IN}=1：不對 WSL 端寫入，直接結束（唯讀）。")
        return 0
    wsl_cfg = ac.wsl_runtime(ac.load_config())  # 只取 WSL 那一筆的 distro／socket
    wsl = wsl_cfg["wsl"]
    status = ac.wsl_server_status(wsl["distro"])
    if status is None or not ac.check(bool(status.get("running")), "WSL 端測試 server 在跑（herdr status server --json）"):
        ac.check(status is not None, "WSL 端 herdr status server --json 有回應")
        return ac.result()
    print(f"WSL server version={status.get('version')} protocol={status.get('protocol')}")

    leftovers = marked_workspaces(wsl_cfg)
    if leftovers:
        print(f"注意：起始就有 label={WS_LABEL} 的 workspace {sorted(leftovers)}（上次殘留），先關掉")
        cleanup_workspaces(wsl_cfg, wsl, None)

    tmp = Path(tempfile.mkdtemp(prefix="cockpit-7-2-"))
    proc: subprocess.Popen[bytes] | None = None
    log: Path | None = None
    ws_id: str | None = None
    try:
        # ---- 1. WSL 端建 workspace 與四個 pane ----
        created = rpc(wsl, "workspace.create", {"label": WS_LABEL, "focus": False})
        ws_id = created["result"]["workspace"]["workspace_id"]
        panes = {"backend": created["result"]["root_pane"]["pane_id"]}
        for name, target in [("frontend", "backend"), ("tests", "frontend"), (EXTRA_LABEL, "tests")]:
            r = rpc(wsl, "pane.split", {"target_pane_id": panes[target], "direction": "right", "focus": False})
            panes[name] = r["result"]["pane"]["pane_id"]
        for name, pid in panes.items():
            ac.check(ok(rpc(wsl, "pane.rename", {"pane_id": pid, "label": LABELS.get(name, EXTRA_LABEL)})), f"pane.rename {pid} → {name}")
        print(f"workspace {ws_id} panes: {panes}")

        # tests pane 在 cockpit 啟動前就 working：驗「啟動後 6 s 內 task running 且不被重播蓋掉」。
        resp = rpc(wsl, "pane.report_agent", {"pane_id": panes["tests"], "source": SOURCE, "agent": "claude", "state": "working"})
        ac.check(ok(resp) and resp["result"].get("type") == "ok", f"啟動前 pane.report_agent({panes['tests']}, working) 回 ok")
        time.sleep(0.5)

        cfg, state_path = write_config(tmp, wsl)
        proc, log = start_cockpit(cfg, tmp, 1)
        t_start = time.time()

        # ---- 2. 三條 workstream 自動解析為 bound(auto) ----
        def all_bound(s: dict) -> bool:
            return ac.all_connected(s) and bool(s.get("projects")) and all(
                binding(s, ws) == {**binding(s, ws), "state": "bound", "pane_id": panes[ws], "source": "auto"} for ws in LABELS
            )

        def startup_ok(s: dict) -> bool:
            return (all_bound(s) and task(s, "qa1")["status"] == "running"
                    and task(s, "be1")["status"] == "ready" and task(s, "fe1")["status"] == "ready")

        strict_hold("啟動：三條 bound(auto)、qa1 running、be1／fe1 ready", startup_ok, t_start, deadline=CONNECT_DEADLINE)
        st = ac.api()
        if st is None or not st.get("projects") or not all_bound(st):
            print("啟動後狀態不符，無法繼續：", summary(st))
            return 2
        ac.check(all(task(st, t)["status"] == "ready" for t in ("be1", "fe1")), "report 前 be1／fe1 皆 ready（pane 無 agent）")
        ac.check(task(st, "be2")["status"] == "pending", "be2 依賴未完成的 be1 → pending")
        ac.check(state_path.exists() is False, "尚未有寫入操作 → 狀態檔不存在（不立即建立）")
        time.sleep(0.5)  # ReopenStatus 200 ms 去抖動（handover §4）

        # ---- 3. Scenario C／D：report working ----
        t_report: dict[str, float] = {}
        for ws in ("backend", "frontend"):
            t_report[ws] = time.time()
            resp = rpc(wsl, "pane.report_agent", {"pane_id": panes[ws], "source": SOURCE, "agent": "claude", "state": "working"})
            ac.check(ok(resp) and resp["result"].get("type") == "ok", f"pane.report_agent({panes[ws]}, working) 回 ok")
        latencies: dict[str, float] = {}
        seen: set[str] = set()
        deadline = time.time() + 10
        st = None
        while time.time() < deadline and len(seen) < 2:
            st = ac.api()
            if st is not None:
                for tid, ws, _, _ in TASKS[:2]:
                    if tid not in seen and task(st, tid)["status"] == "running":
                        seen.add(tid)
                        latencies[tid] = time.time() - t_report[ws]
            time.sleep(0.05)
        for tid, _, _, _ in TASKS[:2]:
            lat = latencies.get(tid)
            ac.check(lat is not None and lat <= 3.0,
                     f"{tid} 變 running，距 report_agent {lat:.2f}s（≤3s）" if lat is not None else f"{tid} 10s 內未變 running")
        print("running latency (s):", {k: round(v, 2) for k, v in latencies.items()})
        strict_hold("report 後三個 task running 持續", lambda s: all(task(s, t)["status"] == "running" for t in ("be1", "fe1", "qa1")),
                    time.time(), deadline=1.0)
        st, _ = ac.wait_for(lambda s: all(task(s, t)["status"] == "running" for t in ("be1", "fe1", "qa1")), 5)
        if st is not None:
            ac.check(binding(st, "backend") == {"state": "bound", "runtime": "wsl", "pane_id": panes["backend"], "source": "auto", "agent": "claude", "agent_status": "working"},
                     f"Scenario C：backend binding JSON 逐欄位相符（實際 {binding(st, 'backend')}）")
            stages = {t: task(st, t)["stage"] for t in ("be1", "fe1", "qa1")}
            ac.check(stages == {"be1": "Implement", "fe1": "Plan", "qa1": "Test"}, f"Scenario D：三個 running task 各自在 Implement／Plan／Test（實際 {stages}）")
            ac.check(all(task(st, t)["mark"] == "none" for t in ("be1", "fe1", "qa1")), "Runtime working 不改 mark")
            ac.check(task(st, "be2")["status"] == "pending", "be2 綁定 pane working 但依賴未完成 → 仍 pending")

        # ---- 4. HTTP 推進／標記 ----
        v0 = api_now()["version"]
        code, body = http("POST", f"/api/projects/{PROJECT}/tasks/fe1/advance")
        ac.check(code == 204, f"POST fe1/advance → {code}（預期 204）")
        st, dt = ac.wait_for(lambda s: s["version"] > v0 and task(s, "fe1")["stage"] == "Implement", 3)
        ac.check(st is not None, f"advance 後 {dt:.2f}s 內 version>{v0} 且 fe1 在 Implement（實際 version {st['version'] if st else '?'}）")

        v1 = api_now()["version"]
        code, body = http("POST", f"/api/projects/{PROJECT}/tasks/be1/complete", headers={"Origin": BASE})
        ac.check(code == 204, f"POST be1/complete（Host＋同源 Origin）→ {code}（預期 204）")
        st, dt = ac.wait_for(lambda s: s["version"] > v1 and task(s, "be1")["mark"] == "completed", 3)
        ac.check(st is not None, f"complete 後 {dt:.2f}s 內 version>{v1}、be1 mark=completed")
        if st is not None:
            ac.check(task(st, "be1")["status"] == "completed" and task(st, "be1")["stage"] == "Implement", "be1 status=completed、stage 仍 Implement（done 不是 Completed，標記才是）")
            ac.check(task(st, "be2")["status"] == "running", f"依賴 be1 完成後 be2（backend 綁定 working）→ running（實際 {task(st, 'be2')['status']}）")

        v2 = api_now()["version"]
        code, body = http("POST", f"/api/projects/{PROJECT}/tasks/qa1/advance")
        ac.check(code == 409 and isinstance(body, dict) and "error" in body, f"POST qa1/advance（已在最後一站）→ {code} {body}")
        code, _ = http("POST", f"/api/projects/{PROJECT}/tasks/qa1/complete", headers={"Origin": "https://evil.example"})
        ac.check(code == 403, f"跨站 Origin 的 POST qa1/complete → {code}（預期 403）")
        code, _ = http("POST", f"/api/projects/{PROJECT}/tasks/nope/complete")
        ac.check(code == 404, f"POST 不存在的 task → {code}（預期 404）")
        time.sleep(1.0)
        st = api_now()
        ac.check(st["version"] == v2, f"409／403／404 不遞增 version（前 {v2}，1 s 後 {st['version']}）")
        ac.check(task(st, "qa1")["stage"] == "Test" and task(st, "qa1")["mark"] == "none", "qa1 未被改動")

        sf = read_state_file(state_path)
        want_tasks = {"stage": "Implement", "mark": "completed"}
        ac.check(sf is not None and sf.get("version") == 1 and sf["projects"][PROJECT]["tasks"].get("be1") == want_tasks
                 and sf["projects"][PROJECT]["tasks"].get("fe1", {}).get("stage") == "Implement",
                 f"狀態檔 version=1、be1={want_tasks}、fe1 stage=Implement（實際 {sf}）")

        # ---- 5. 設覆蓋 → 重啟 → 進度與覆蓋保留 ----
        v3 = api_now()["version"]
        code, body = http("PUT", f"/api/projects/{PROJECT}/workstreams/tests/override", {"runtime": "wsl", "pane_id": panes[EXTRA_LABEL]})
        ac.check(code == 204, f"PUT tests/override → {panes[EXTRA_LABEL]}：{code}（預期 204）")
        st, dt = ac.wait_for(lambda s: s["version"] > v3 and binding(s, "tests").get("source") == "override", 3)
        ac.check(st is not None and binding(st, "tests").get("pane_id") == panes[EXTRA_LABEL],
                 f"覆蓋生效：tests bound(override) → {panes[EXTRA_LABEL]}（{dt:.2f}s）")
        if st is not None:
            ac.check(task(st, "qa1")["status"] == "ready", f"覆蓋到沒有 agent 的 pane → qa1 ready（實際 {task(st, 'qa1')['status']}）")
        sf = read_state_file(state_path)
        ac.check(sf is not None and sf["projects"][PROJECT].get("overrides", {}).get("tests") == {"runtime": "wsl", "pane_id": panes[EXTRA_LABEL]},
                 "狀態檔含 tests 的覆蓋")

        stop(proc, log)
        proc = None
        print("--- cockpit 已停止，重新啟動 ---")
        proc, log = start_cockpit(cfg, tmp, 2)
        t_restart = time.time()

        def restored(s: dict) -> bool:
            return (ac.all_connected(s) and bool(s.get("projects")) and binding(s, "tests").get("source") == "override"
                    and binding(s, "backend").get("state") == "bound")

        st, _ = ac.wait_for(restored, 60)
        ac.check(st is not None, f"重啟後 {time.time() - t_restart:.2f}s 內 connected 且 tests 覆蓋仍在（進度來自狀態檔）")
        if st is not None:
            got = {t: (task(st, t)["stage"], task(st, t)["mark"]) for t in ("be1", "fe1", "qa1", "be2")}
            want = {"be1": ("Implement", "completed"), "fe1": ("Implement", "none"), "qa1": ("Test", "none"), "be2": ("Plan", "none")}
            ac.check(got == want, f"重啟後進度保留（實際 {got}）")
            ac.check(binding(st, "tests").get("pane_id") == panes[EXTRA_LABEL], "重啟後 tests 覆蓋仍指向 extra pane")
            ac.check(project(st)["warnings"] == [], f"重啟後 project warnings 為空（實際 {project(st)['warnings']}）")
        def restart_ok(s: dict) -> bool:
            return (restored(s) and task(s, "be2")["status"] == "running" and task(s, "fe1")["status"] == "running"
                    and binding(s, "backend").get("pane_id") == panes["backend"]
                    and binding(s, "frontend").get("pane_id") == panes["frontend"])

        strict_hold("重啟：be2／fe1 running、tests 覆蓋仍在", restart_ok, t_restart, deadline=CONNECT_DEADLINE)

        # ---- 6. 關覆蓋指向的 pane → 覆蓋失效 ----
        closed_id = panes[EXTRA_LABEL]
        v4 = api_now()["version"]
        t_close = time.time()
        resp = rpc(wsl, "pane.close", {"pane_id": closed_id})
        ac.check(ok(resp), f"pane.close {closed_id} 回 ok")
        auto_back = {"state": "bound", "runtime": "wsl", "pane_id": panes["tests"], "source": "auto", "agent": "claude", "agent_status": "working"}

        def override_dropped(s: dict) -> bool:
            sfile = read_state_file(state_path)
            return (s["version"] > v4 and binding(s, "tests").get("source") != "override" and sfile is not None
                    and "tests" not in sfile["projects"][PROJECT].get("overrides", {}))

        st, dt = ac.wait_for(override_dropped, 35, interval=0.1)
        ac.check(st is not None, f"關 pane 後 {dt:.2f}s 內覆蓋失效：投影 tests 不再是 override、狀態檔 overrides 不含 tests")
        if st is not None:
            print(f"  覆蓋失效當下 tests binding = {binding(st, 'tests')}")
        def all_running(s: dict) -> bool:
            return (binding(s, "tests") == auto_back and all(task(s, t)["status"] == "running" for t in ("be2", "fe1", "qa1"))
                    and binding(s, "backend").get("pane_id") == panes["backend"]
                    and binding(s, "frontend").get("pane_id") == panes["frontend"])

        strict_hold("關 pane 後：tests 回 bound(auto)，be2／fe1／qa1 running 不倒退", all_running, t_close)
        s = ac.api()
        if s is not None:
            ac.check(task(s, "qa1")["status"] == "running", f"回到自動解析後 qa1（tests pane working）→ running（實際 {task(s, 'qa1')['status']}）")
            drifts = [e["detail"] for e in s["recent_events"] if e["kind"] == "drift"]
            print(f"drift events after close: {len(drifts)}")

        # ---- 7. 觀察：pane id 是否重用 ----
        t_split = time.time()
        r = rpc(wsl, "pane.split", {"target_pane_id": panes["tests"], "direction": "right", "focus": False})
        new_id = r["result"]["pane"]["pane_id"] if ok(r) else None
        print(f"OBSERVE pane id reuse: closed={closed_id} new_split={new_id} reused={new_id == closed_id}")
        strict_hold("建立 pane 後：tests 仍 bound(auto)，be2／fe1／qa1 running 不倒退", all_running, t_split)
        s = ac.api()
        if s is not None:
            ac.check(binding(s, "tests").get("source") == "auto", "新 split 的 pane 不會復活已刪除的覆蓋")
    finally:
        try:
            cleanup_workspaces(wsl_cfg, wsl, ws_id)
        finally:
            try:
                stop(proc, log)
            finally:
                shutil.rmtree(tmp, ignore_errors=True)
                ac.check(not tmp.exists(), "暫存設定／狀態檔目錄已刪除")
    return ac.result()


if __name__ == "__main__":
    sys.exit(main())
