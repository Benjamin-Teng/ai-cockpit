//! 子程序 stdio 橋接連線（spec「子程序 stdio 橋接連線」；design D3、D9）。

use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::BufReader;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

use super::stream::{read_one_line, write_line};
use super::{ConnectError, Connector, NdjsonStream};

/// stderr 尾端保留行數上限，供 `ServerNotRunning` 錯誤訊息使用（design D3）。
const STDERR_TAIL_LINES: usize = 20;

/// 判定子程序已結束後，等待 stderr 背景 task 把剩餘輸出讀完的時間上限（fix round 1
/// finding 2）：子程序已經結束代表它那端的 stderr 寫入端也關了，背景 task 的
/// `read_one_line` 應該很快就會拿到 EOF；給一個遠大於正常情況的上限避免真的卡住。
const STDERR_DRAIN_BUDGET: Duration = Duration::from_millis(200);

/// 全分支最終 review finding 3：`recv_line` 已經收到過至少一行（`received_any_line`）之後
/// 遇到 stdout EOF 時，判定子程序是否非零退出的有限等待上限——跟 D3 判定用的 100ms
/// （`exited_status`）分開命名，語意不同：D3 是「連線根本沒建立起來」，這裡是「串流讀到
/// 一半，子程序才剛結束」，stdout EOF 通知跟子程序結束通知互相追上可能需要多一點時間。
///
/// Codex 最終 review 二次確認 finding：逾時**不再**視為「還在跑」而默默放行成正常 EOF——
/// nc／relay 這類橋接目標在對面 socket EOF 後都會立刻結束，「關了 stdout 卻在這個預算內
/// 還沒退出」本身就是異常（例如橋接程序被卡在清理或排程延遲），逾時一律轉譯成
/// `ConnectionAborted` 類 I/O 錯誤，見 `PostStreamExit::StillRunning` 與 `recv_line` 的處理。
const POST_STREAM_EXIT_WAIT: Duration = Duration::from_millis(500);

/// `exited_status_after_stream` 的判定結果。跟舊版用 `Option<ExitStatus>`（`None` 同時代表
/// 「還在跑」）不同，這裡把「已確認結束」與「逾時仍未結束」分成兩個變體——Codex 最終 review
/// 二次確認 finding：舊版把這兩種情況併成同一個分支，等於把「stdout 已關閉、子程序卻遲遲
/// 不退出」這種橋接故障誤報成正常串流結束。
///
/// fix round 3 finding：`try_wait`／`wait` 本身失敗（作業系統或 process handle 層的錯誤）
/// 不是這裡的第三個變體，而是 `exited_status_after_stream` 回傳型別外層的
/// `io::Result::Err`——不會被塞進 `StillRunning`，避免 wait 錯誤被逾時那個固定訊息蓋掉。
enum PostStreamExit {
    /// 子程序已結束，帶著結束狀態（exit 0 或非零都算）。
    Exited(std::process::ExitStatus),
    /// `POST_STREAM_EXIT_WAIT` 逾時仍未結束（`try_wait` 回 `Ok(None)`、逾時的 `wait` 也沒能
    /// 在時限內給出結果；兩者本身失敗則不會走到這個變體，見上）。
    StillRunning,
}

/// Windows `CREATE_NO_WINDOW`：啟動子程序不出現主控台視窗，不與 `DETACHED_PROCESS` 併用
/// （ADR-0002、design D9）。直接硬寫常數，不為它引入 `windows-sys`。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 以任意指令列啟動子程序，把子程序的 stdin／stdout 當作一條 NDJSON 連線
/// （spec「子程序 stdio 橋接連線」；設計文件 §5.1）。
pub struct ChildStdioConnector {
    command: String,
    args: Vec<String>,
}

impl ChildStdioConnector {
    pub fn new(
        command: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            command: command.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    /// spawn 子程序並回傳具體型別。`Connector::connect` 只回 `Box<dyn NdjsonStream>`；
    /// 這個方法額外公開給呼叫端在需要具體型別時使用（例如測試要查 `pid()` 驗證
    /// `kill_on_drop`）。
    pub async fn spawn_stream(&self) -> Result<ChildStdioStream, ConnectError> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd.spawn().map_err(|e| ConnectError::Spawn {
            detail: e.to_string(),
        })?;

        let stdin = child.stdin.take().expect("stdin 應為 piped");
        let stdout = child.stdout.take().expect("stdout 應為 piped");
        let stderr = child.stderr.take().expect("stderr 應為 piped");

        let stderr_tail = Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_TAIL_LINES)));
        let stderr_task = spawn_stderr_logger(stderr, stderr_tail.clone());

        Ok(ChildStdioStream {
            child,
            reader: BufReader::new(stdout),
            writer: stdin,
            stderr_tail,
            stderr_task: Some(stderr_task),
            received_any_line: false,
        })
    }
}

#[async_trait]
impl Connector for ChildStdioConnector {
    async fn connect(&self) -> Result<Box<dyn NdjsonStream>, ConnectError> {
        self.spawn_stream()
            .await
            .map(|stream| Box::new(stream) as Box<dyn NdjsonStream>)
    }

    fn describe(&self) -> String {
        if self.args.is_empty() {
            format!("child {}", self.command)
        } else {
            format!("child {} {}", self.command, self.args.join(" "))
        }
    }
}

/// 背景 task：逐行讀 stderr、進 `tracing::debug`，並保留最後 `STDERR_TAIL_LINES` 行供
/// `ChildStdioStream` 判定 `ServerNotRunning` 時組錯誤訊息用。回傳 `JoinHandle`——
/// fix round 1 finding 2：呼叫端要在讀 `tail` 之前先（有上限地）等這個 task 跑到 EOF，
/// 不能讓它單純被丟棄、靠巧合的排程順序祈禱它已經讀完。
fn spawn_stderr_logger(
    stderr: ChildStderr,
    tail: Arc<Mutex<VecDeque<String>>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        loop {
            match read_one_line(&mut reader).await {
                Ok(Some(line)) => {
                    tracing::debug!(target: "herdr_client::connector::child_stdio", "{line}");
                    let mut tail = tail.lock().expect("stderr tail mutex poisoned");
                    if tail.len() == STDERR_TAIL_LINES {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::debug!(
                        target: "herdr_client::connector::child_stdio",
                        "讀取 stderr 失敗: {e}"
                    );
                    break;
                }
            }
        }
    })
}

/// 一條經子程序 stdio 建立的連線；`child` 隨這個值一起 drop，`kill_on_drop(true)` 因此在
/// stream 被丟棄時生效（spec「連線釋放時子程序終止」）。
pub struct ChildStdioStream {
    child: Child,
    reader: BufReader<ChildStdout>,
    writer: ChildStdin,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    /// stderr 背景 task 的 handle；`bridge_exited_error` 組錯誤訊息前會先（有上限地）
    /// `await` 它一次，之後設回 `None`（fix round 1 finding 2）。
    stderr_task: Option<tokio::task::JoinHandle<()>>,
    received_any_line: bool,
}

impl ChildStdioStream {
    /// 子程序 pid；程序已結束且已被 reap 時可能為 `None`。
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// 判定子程序是否已結束：先 `try_wait`（不阻塞），若仍顯示執行中，再給 100ms 讓
    /// stdout EOF 通知與程序結束通知互相追上（兩者在作業系統層不是原子發生的單一事件）；
    /// 逾時仍視為未結束（design D3：只在「還沒收到過任何一行就 EOF、且子程序已結束」時
    /// 才轉譯成 `ServerNotRunning` 類錯誤，不能為了判定這件事無限期等待子程序）。
    async fn exited_status(&mut self) -> Option<std::process::ExitStatus> {
        if let Ok(Some(status)) = self.child.try_wait() {
            return Some(status);
        }
        tokio::time::timeout(Duration::from_millis(100), self.child.wait())
            .await
            .ok()
            .and_then(Result::ok)
    }

    /// design D3 的判定：「還沒收到過任何一行、且子程序已結束」時回傳要轉譯成的
    /// `ConnectionRefused` 類錯誤；否則回 `None`（維持呼叫端原本的行為：`recv_line`
    /// 應回 `Ok(None)`，`send_line` 應回原始的寫入錯誤）。`send_line`／`recv_line` 共用
    /// 這個判定與底下的 stderr 組訊息邏輯（fix round 1 finding 1：兩條路徑必須分類一致）。
    ///
    /// 全分支最終 review finding 3：`received_any_line` 為 `true`（已經進入串流階段）時的
    /// stdout EOF 判定是另一條路徑（`recv_line` 呼叫 `classify_post_stream`），語意不同、
    /// 錯誤 kind 也不同，見那邊的文件註解；這個方法仍然只覆蓋「還沒收到過任何一行」這一側，
    /// `send_line` 也還是依賴這條內部短路（子程序已經開始持續送出事件之後，`send_line`
    /// 的寫入錯誤不該被這個 D3 判定攔截、改判成 `ServerNotRunning`）。
    async fn maybe_bridge_exited_error(&mut self) -> Option<std::io::Error> {
        if self.received_any_line {
            return None;
        }
        let status = self.exited_status().await?;
        Some(self.bridge_exited_error(status).await)
    }

    /// 先（有上限地）等 stderr 背景 task 把剩餘輸出讀完，再讀 tail（fix round 1
    /// finding 2）。子程序已經結束代表 stderr 那端的寫入端也關了，背景 task 的
    /// `read_one_line` 應該很快就 EOF；`STDERR_DRAIN_BUDGET` 只是防止真的卡住的保險，不是
    /// 預期路徑。`bridge_exited_error`（D3）與 `recv_line`（finding 3，組好 tail 後交給
    /// `classify_post_stream` 純函式判定）共用這段抽出來的邏輯。
    async fn drain_stderr_tail(&mut self) -> String {
        if let Some(handle) = self.stderr_task.take() {
            let _ = tokio::time::timeout(STDERR_DRAIN_BUDGET, handle).await;
        }
        self.stderr_tail
            .lock()
            .expect("stderr tail mutex poisoned")
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// design D3：「還沒收到過任何一行、且子程序已結束」的錯誤——`ConnectionRefused` kind，
    /// 讓 `Client` 對應成 `RequestError::Connect(ConnectError::ServerNotRunning)`。
    async fn bridge_exited_error(&mut self, status: std::process::ExitStatus) -> std::io::Error {
        let tail = self.drain_stderr_tail().await;
        std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            format!("bridge exited ({status}): {tail}"),
        )
    }

    /// 全分支最終 review finding 3：`recv_line` 已經收到過至少一行之後遇到 stdout EOF 時，
    /// 用這個方法（有限等待 `POST_STREAM_EXIT_WAIT`）確認子程序的結束狀態——跟
    /// `exited_status`（D3 判定用，100ms）分開，等待預算不同（見 `POST_STREAM_EXIT_WAIT`
    /// 的文件註解）。
    ///
    /// Codex 最終 review 二次確認 finding：逾時回傳 `Ok(PostStreamExit::StillRunning)`，不再
    /// 悄悄併入「還在跑、視為正常 EOF」——呼叫端（`recv_line`）要把這個結果轉譯成錯誤，不能
    /// 沿用舊版把逾時跟「已確認結束」用同一個 `None`／`Some` 分支處理的做法。
    ///
    /// fix round 3 finding：回傳型別從 `PostStreamExit` 改成 `io::Result<PostStreamExit>`。
    /// 舊版用 `if let Ok(Some(status)) = try_wait()` 靜默略過 `try_wait` 的 `Err`，之後
    /// `child.wait()` 若也回 `Err`，兩者都被逾時那個萬用分支（`_ =>`）併成
    /// `StillRunning`——`recv_line` 因而回報「did not exit within 500ms」，即使實際上是
    /// OS／process-handle 層的 wait 錯誤，底層失效原因就此永久丟失。現在 `try_wait`／
    /// `wait` 各自的 `Err` 都原樣往外傳，只有「確實逾時」才產生 `StillRunning`。
    async fn exited_status_after_stream(&mut self) -> std::io::Result<PostStreamExit> {
        match self.child.try_wait() {
            Ok(Some(status)) => return Ok(PostStreamExit::Exited(status)),
            Ok(None) => {}
            Err(e) => return Err(e),
        }
        match tokio::time::timeout(POST_STREAM_EXIT_WAIT, self.child.wait()).await {
            Ok(Ok(status)) => Ok(PostStreamExit::Exited(status)),
            Ok(Err(e)) => Err(e),
            Err(_timed_out) => Ok(PostStreamExit::StillRunning),
        }
    }

    /// 測試專用：等子程序真正結束並回收其結束狀態，用 `Child::wait`（實際 poll／reap，
    /// 不是外部 `tasklist`／`kill -0` 這種可能把 Unix zombie（已退出但還沒被
    /// `wait`／`try_wait` 回收）誤判成「還活著」的 OS 層輪詢；fix round 2 finding B）。
    /// 只在 `test-support` feature 下編譯，不是公開 API 的一部分。
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub async fn wait_for_exit(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<std::process::ExitStatus> {
        tokio::time::timeout(timeout, self.child.wait())
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "子程序未在時限內結束")
            })?
    }
}

#[async_trait]
impl NdjsonStream for ChildStdioStream {
    async fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        // fix round 1 finding 1：spec 的 Scenario 是先送一行、再讀一行；子程序已經結束
        // 時，寫入 stdin 常常會先撞上 BrokenPipe／ConnectionReset（管線讀取端已關閉），
        // 這裡跟 recv_line 共用同一套「還沒收到任何一行、子程序已結束」判定，避免把
        // 這種情況誤報成普通 Io 錯誤。其餘寫入錯誤（例如子程序還在跑、只是暫時忙碌）
        // 原樣回傳，不做分類。
        match write_line(&mut self.writer, line).await {
            Ok(()) => Ok(()),
            Err(e) => {
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                ) && let Some(classified) = self.maybe_bridge_exited_error().await
                {
                    Err(classified)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn recv_line(&mut self) -> std::io::Result<Option<String>> {
        let outcome = read_one_line(&mut self.reader).await?;
        if outcome.is_some() {
            self.received_any_line = true;
            return Ok(outcome);
        }

        if !self.received_any_line {
            // design D3：還沒收到過任何一行就 EOF、且子程序已結束 -> ServerNotRunning 類
            // 錯誤，附上 stderr 尾端內容。
            return match self.maybe_bridge_exited_error().await {
                Some(err) => Err(err),
                None => Ok(None),
            };
        }

        // 全分支最終 review finding 3：已經收到過至少一行（例如 `subscription_started`）
        // 之後的 stdout EOF，不能再無條件當成乾淨 EOF——橋接目標（`wsl.exe`／`nc`）可能在
        // 中途因故障非零退出，這時要把子程序的結束狀態＋stderr 尾端包成一個 I/O 錯誤，讓
        // `EventStream::next` 回報 `StreamError::Io`（design D6），而不是誤判成「對面正常
        // 關閉連線」。
        //
        // Codex 最終 review 二次確認 finding：`PostStreamExit::StillRunning`（逾時
        // `POST_STREAM_EXIT_WAIT` 仍未結束）也要回報成 I/O 錯誤，不能再默默當成正常 EOF——
        // 「關了 stdout 卻在這個預算內還沒退出」本身就是異常，只有 exit 0 才是真正的正常
        // 結束。
        //
        // fix round 3 finding：`exited_status_after_stream` 現在回 `io::Result`，`try_wait`／
        // `wait` 本身的錯誤不會再被吞掉、也不會被誤併成 `StillRunning` 的逾時訊息——分類
        // 決策交給不做 I/O 的 `classify_post_stream`（`#[cfg(test)]` 有單元測試覆蓋四條分支）。
        let result = self.exited_status_after_stream().await;
        let tail = self.drain_stderr_tail().await;
        classify_post_stream(result, &tail)
    }
}

/// fix round 3 finding：把 `exited_status_after_stream` 的結果（含 `try_wait`／`wait` 本身
/// 失敗的情況）＋已擷取好的 stderr tail 轉譯成 `recv_line` 的回傳值。抽成不帶 `self`、不做
/// I/O 的純函式，方便用 `#[cfg(test)]` 單元測試直接覆蓋四條分支，不必真的去跑一個會逾時或
/// 讓 wait 失敗的子程序。
///
/// Codex finding：舊版 `exited_status_after_stream` 用 `if let Ok(Some(status)) = try_wait()`
/// 靜默略過 `try_wait` 的 `Err`，之後 `child.wait()` 若也回 `Err`，兩者都被逾時那個萬用分支
/// 併成 `StillRunning`，`recv_line` 因而回報「did not exit within 500ms」——即使實際上是
/// OS／process-handle 層的 wait 錯誤，不是真的逾時。這裡把 wait 錯誤獨立成一個分支，訊息
/// 保留原始錯誤原因（`{e}`），跟逾時的固定訊息（"did not exit within 500ms"）區分開來。
fn classify_post_stream(
    result: std::io::Result<PostStreamExit>,
    stderr_tail: &str,
) -> std::io::Result<Option<String>> {
    match result {
        Ok(PostStreamExit::Exited(status)) if status.success() => Ok(None),
        // finding 3 原本的「非零退出」分支：kind 用 `ConnectionAborted`（不是
        // `bridge_exited_error` 的 `ConnectionRefused`：那個 kind 專屬於 D3「連線根本沒建立
        // 起來」，這裡是「串流讀到一半，子程序才因故障中斷」，語意不同）。`EventStream::next`
        // 只在乎 `StreamError::Io` 的原因文字非空（design D6），不檢查 kind。
        Ok(PostStreamExit::Exited(status)) => Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            format!("bridge exited ({status}): {stderr_tail}"),
        )),
        Ok(PostStreamExit::StillRunning) => Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            "bridge closed stdout but did not exit within 500ms",
        )),
        Err(e) => Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            format!("failed to wait for bridge after stdout EOF: {e}"),
        )),
    }
}

#[cfg(test)]
mod post_stream_tests {
    use super::{PostStreamExit, classify_post_stream};

    /// 建構任意結束狀態，不必真的去跑子程序（`ExitStatusExt::from_raw` 在 unix／windows
    /// 兩邊都是 std 的穩定 API）。`code` 在 unix 是 wait status（`0` 代表成功結束、非 0
    /// 代表帶著該退出碼結束——僅測試用途，不需要處理 signal 相關的 bit pattern）；在
    /// windows 就是行程的退出碼本身。
    #[cfg(unix)]
    fn exit_status(code: i32) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code)
    }

    #[cfg(windows)]
    fn exit_status(code: i32) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }

    #[test]
    fn exit_success_is_clean_eof() {
        let result = Ok(PostStreamExit::Exited(exit_status(0)));
        let outcome = classify_post_stream(result, "");
        assert!(
            matches!(outcome, Ok(None)),
            "預期 Ok(None)，實得 {outcome:?}"
        );
    }

    #[test]
    fn nonzero_exit_reports_status_and_tail() {
        let result = Ok(PostStreamExit::Exited(exit_status(3)));
        let err =
            classify_post_stream(result, "boom stderr line").expect_err("非零退出必須回傳錯誤");
        let msg = err.to_string();
        assert!(msg.contains("bridge exited"), "訊息應含結束狀態：{msg}");
        assert!(
            msg.contains("boom stderr line"),
            "訊息應含 stderr tail：{msg}"
        );
    }

    #[test]
    fn still_running_reports_timeout_message() {
        let err = classify_post_stream(Ok(PostStreamExit::StillRunning), "")
            .expect_err("逾時仍未結束必須回傳錯誤");
        let msg = err.to_string();
        assert!(
            msg.contains("did not exit within"),
            "訊息應含逾時字樣：{msg}"
        );
    }

    #[test]
    fn wait_error_is_not_mistaken_for_timeout() {
        let wait_err = std::io::Error::other("boom");
        let err = classify_post_stream(Err(wait_err), "").expect_err("wait 本身失敗必須回傳錯誤");
        let msg = err.to_string();
        assert!(
            msg.contains("failed to wait"),
            "訊息應標明是 wait 失敗：{msg}"
        );
        assert!(msg.contains("boom"), "訊息應保留原始錯誤原因：{msg}");
        assert!(
            !msg.contains("did not exit within"),
            "wait 失敗不該被誤植成逾時訊息：{msg}"
        );
    }
}
