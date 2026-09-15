"""change 1b task 4.3 Scenario F（Reconnect，只在 WSL 端）。

用法（repo 根，先 `cargo build --release -p cockpit`；Windows 端 HERDR 與 WSL 端測試 server 都在跑）：
    HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1 COCKPIT_ACCEPT_STOP_WSL_DISTRO=<distro> \
        uv run --no-project python docs/research/2026-09-15/reconnect-real-check.py

破壞性操作的 guard（缺一就不會動到任何 server）：
1. `HERDR_CLIENT_TEST_ALLOW_WSL_WRITES=1`：與 1a 真機測試、live-state-check.py 相同的 WSL 端寫入 opt-in；
   未設定時只印說明、exit 0（唯讀）。
2. `COCKPIT_ACCEPT_STOP_WSL_DISTRO` 必須等於 cockpit.toml 裡 wsl runtime 的 distro：操作者明確認可
   「要停的就是這個 distro 的 HERDR server」；不符 exit 2。
3. 停之前查 `herdr status server --json`：必須 running，且回報的 socket 與 cockpit.toml 設定的 socket 相同
   （確保停的是 cockpit 正在觀察的那個 server，而不是別的 instance）；不符 exit 2。
本腳本從不碰 Windows 端 HERDR（設計文件 §10.1 的禁令針對 Windows 端）。

流程：起 cockpit → 等兩筆 connected → guard → 對 WSL 端測試 server `herdr server stop`（檢查 return code
與停後狀態）→ 觀察 wsl 卡片的 connection 轉變（原因字串、retry_in_secs 序列）→ 以 handover §1 的
`setsid -f` 指令重啟（檢查重啟後狀態）→ 等回到 connected → 用 acceptance_common.compare_projection 對
WSL 端重新取得的 `herdr api snapshot` 逐欄位比對；Windows 端全程應維持 connected。輸出不含路徑。
"""

from __future__ import annotations

import os
import subprocess
import sys
import time

import acceptance_common as ac

ACK_ENV = "COCKPIT_ACCEPT_STOP_WSL_DISTRO"
START_CMD = "setsid -f ~/.local/bin/herdr server >/tmp/herdr-server.log 2>&1 </dev/null"


def server_running(distro: str) -> bool:
    status = ac.wsl_server_status(distro)
    return status is not None and status.get("status") == "running"


def ensure_running(distro: str) -> bool:
    """WSL 測試 server 沒在跑就用 setsid -f 重啟，回傳最後是否在跑；wsl.exe 卡住或失敗一律回 False，不拋例外。"""
    try:
        if server_running(distro):
            return True
        ac.wsl_bash(distro, START_CMD)
        time.sleep(2)
        return server_running(distro)
    except (subprocess.SubprocessError, OSError) as e:
        print(f"ensure_running：wsl.exe 失敗：{e}")
        return False


def main() -> int:
    ac.configure_stdout()
    if os.environ.get(ac.WSL_WRITE_OPT_IN) != "1":
        print(f"未設定 {ac.WSL_WRITE_OPT_IN}=1：不停任何 server，直接結束（唯讀）。")
        return 0
    cfg = ac.load_config()
    wsl_cfg = ac.wsl_runtime(cfg)
    distro, socket = wsl_cfg["wsl"]["distro"], wsl_cfg["wsl"]["socket"]
    wsl_id = wsl_cfg["id"]
    if os.environ.get(ACK_ENV) != distro:
        print(f"{ACK_ENV} 必須等於 cockpit.toml 的 wsl distro（{distro!r}）才會停該 server；目前 {os.environ.get(ACK_ENV)!r}。")
        return 2
    status = ac.wsl_server_status(distro)
    if status is None or status.get("status") != "running":
        print(f"WSL 端 {distro} 的 HERDR server 不在 running 狀態（{status!r}），不動作。")
        return 2
    if status.get("socket") != socket:
        print("WSL 端 herdr status server 回報的 socket 與 cockpit.toml 設定不同，可能不是 cockpit 觀察的那個 server，不動作。")
        return 2
    print(f"guard ok: distro={distro} server {status.get('version')} protocol {status.get('protocol')} socket 與設定一致")

    proc = ac.start_cockpit()
    try:
        st, dt = ac.wait_for(ac.all_connected, 60, interval=0.3)
        ac.check(st is not None, f"起始：兩筆 runtime 皆 connected（{dt:.1f}s）")
        if st is None:
            return 2

        # 停 WSL 端測試 server（只針對 WSL 端；guard 已過）。
        r = ac.wsl_bash(distro, "~/.local/bin/herdr server stop")
        print("wsl herdr server stop →", (r.stdout or "").strip().splitlines()[:1], "rc", r.returncode)
        ac.check(r.returncode == 0, "herdr server stop 回 0")
        ac.check(not server_running(distro), "停後 herdr status server 不再 running")
        stopped_at = time.time()

        # 觀察 8 秒：記錄 wsl 卡片 connection 的每次變化（去重連續相同），同時確認 win 一直 connected。
        history: list[tuple[float, str, str, int | None]] = []
        win_always_connected = True
        while time.time() - stopped_at < 8:
            s = ac.api()
            if s is not None:
                c = ac.runtime(s, wsl_id)["connection"]
                entry = (round(time.time() - stopped_at, 2), c["state"], c.get("reason", ""), c.get("retry_in_secs"))
                if not history or history[-1][1:] != entry[1:]:
                    history.append(entry)
                if ac.runtime(s, "win")["connection"]["state"] != "connected":
                    win_always_connected = False
            time.sleep(0.1)
        print("wsl connection history after stop (t, state, reason, retry_in_secs):")
        for h in history:
            print("  ", h)
        disconnected = [h for h in history if h[1] == "disconnected"]
        ac.check(len(disconnected) >= 1, "停 server 後 wsl 卡片變 disconnected")
        if disconnected:
            first = disconnected[0]
            shutdown_words = ("shutting down", "server_unavailable", "L ", "S ", "L 連線", "S 連線")
            ac.check(any(w in first[2] for w in shutdown_words), f"首次 disconnected 原因反映對端關閉（L／S 結束或 server 關閉中）：{first[2]!r}")
            ac.check(first[3] == 1, f"首次重試秒數為 1（實際 {first[3]}）")
            later = disconnected[1:]
            ac.check(any("not running" in h[2] or "ServerNotRunning" in h[2] or "拒絕" in h[2] or "refused" in h[2].lower() for h in later),
                     f"重連期間原因含 ServerNotRunning 類描述：{[h[2] for h in later][:3]!r}")
            retries = [h[3] for h in disconnected if h[3] is not None]
            ac.check(retries == sorted(retries), f"重試秒數遞增（退避）：{retries}")
        ac.check(win_always_connected, "Windows 端全程 connected（不受 WSL 端斷線影響）")

        # 重啟 WSL 端測試 server。
        ac.wsl_bash(distro, START_CMD)
        restarted_at = time.time()
        time.sleep(2)
        ac.check(server_running(distro), "setsid -f 重啟後 herdr status server 為 running")
        st, _ = ac.wait_for(lambda s: ac.runtime(s, wsl_id)["connection"]["state"] == "connected", 60, interval=0.3)
        back = st is not None
        ac.check(back, f"重啟後 wsl 卡片在 {time.time() - restarted_at:.1f}s 內回到 connected")
        if st is not None:
            snap = ac.herdr_snapshot(wsl_cfg)
            print(f"projection {ac.summarize(ac.runtime(st, wsl_id))}; herdr workspaces={len(snap['workspaces'])} tabs={len(snap['tabs'])} panes={len(snap['panes'])}")
            diffs = ac.compare_projection(ac.runtime(st, wsl_id), snap)
            for d in diffs:
                print("    diff:", d)
            ac.check(not diffs, "重連後投影與重新取得的 snapshot 逐欄位一致（workspace／tab／pane／focused）")
            ac.check(ac.runtime(st, "win")["connection"]["state"] == "connected", "Windows 端仍 connected")
    finally:
        # 不論成敗都確保 WSL 端測試 server 回到「在跑」的狀態；救不回來算失敗（非零結束）並印人工復原指令。
        # 巢狀 finally：即使 ensure_running 本身出事，自己起的 cockpit 也一定收掉。
        try:
            ac.check(ensure_running(distro),
                     f"cleanup：WSL 測試 server 在跑（必要時已用 setsid -f 重啟）；救不回來請手動執行 wsl.exe -d {distro} -e bash -lc \"{START_CMD}\"")
        finally:
            ac.stop_cockpit(proc)
    return ac.result()


if __name__ == "__main__":
    sys.exit(main())
