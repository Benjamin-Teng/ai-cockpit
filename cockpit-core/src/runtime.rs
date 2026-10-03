//! `AgentRuntime` 抽象與圍繞它的錯誤、事件流型別：驅動器與狀態庫只透過這個抽象與
//! runtime 互動，看不到 runtime 內部有幾條連線（例如 HERDR 的 gRPC 連線與子程序）。

use std::time::Duration;

use serde::Serialize;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::types::{PaneId, RuntimeEvent, RuntimeId, RuntimeSnapshot};

/// pane 輸出的格式標記。目前只有純文字一種；帶樣式的內容另放在 `PaneOutput::segments`，
/// `format` 欄位本身不變（design（live-output-color）D2）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// `text` 欄位是不含控制序列的純文字（樣式另放在 `segments`；design D2）。
    Text,
}

/// 16 色終端機色票（design D2）；序列化為 snake_case（`red`、`bright_red`…）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnsiColor {
    /// 黑。
    Black,
    /// 紅。
    Red,
    /// 綠。
    Green,
    /// 黃。
    Yellow,
    /// 藍。
    Blue,
    /// 洋紅。
    Magenta,
    /// 青。
    Cyan,
    /// 白。
    White,
    /// 亮黑（灰）。
    BrightBlack,
    /// 亮紅。
    BrightRed,
    /// 亮綠。
    BrightGreen,
    /// 亮黃。
    BrightYellow,
    /// 亮藍。
    BrightBlue,
    /// 亮洋紅。
    BrightMagenta,
    /// 亮青。
    BrightCyan,
    /// 亮白。
    BrightWhite,
}

/// 一段輸出文字的樣式；`Default` 為全空（無色、無屬性）。序列化時 `None` 與 `false` 省略。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SegmentStyle {
    /// 前景色。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<AnsiColor>,
    /// 背景色。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<AnsiColor>,
    /// 粗體。
    #[serde(skip_serializing_if = "is_false")]
    pub bold: bool,
    /// 淡色。
    #[serde(skip_serializing_if = "is_false")]
    pub dim: bool,
    /// 斜體。
    #[serde(skip_serializing_if = "is_false")]
    pub italic: bool,
    /// 底線。
    #[serde(skip_serializing_if = "is_false")]
    pub underline: bool,
    /// 前景背景反轉。
    #[serde(skip_serializing_if = "is_false")]
    pub reverse: bool,
}

// serde 的 `skip_serializing_if` 要求以引用呼叫的函式。
fn is_false(value: &bool) -> bool {
    !*value
}

/// 一段帶樣式的輸出文字；序列化時 `style` 攤平到同一層（`{"text":"x","fg":"red"}`）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutputSegment {
    /// 這一段的文字。
    pub text: String,
    /// 這一段的樣式。
    #[serde(flatten)]
    pub style: SegmentStyle,
}

/// 讀取 pane 輸出的結果。
///
/// 不變式（由欄位私有與兩個建構函式維持，不靠各 runtime 自律；design（live-output-color）D2）：
/// `segments` 依序串接等於 `text`、不含空片段、相鄰片段樣式不同。`format` 目前恆為
/// [`OutputFormat::Text`]。
///
/// 刻意不含任何 runtime 專屬的修訂號或版本欄位（例如 HERDR 的 `revision`，實測恆為 0）：
/// 放進這個型別只會誘導呼叫端依賴一個沒有意義的值（design（live-output）D3）。去重與捲動狀態
/// 由前端自行處理，以片段（含樣式）的 JSON 字串比較去重，不是只比 `text`
/// （design（live-output-color）D7）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PaneOutput {
    format: OutputFormat,
    text: String,
    segments: Vec<OutputSegment>,
    truncated: bool,
}

impl PaneOutput {
    /// 無樣式的輸出：`segments` 為單一段，`text` 為空時為空陣列。給測試替身與純文字的假 runtime。
    pub fn plain(text: impl Into<String>, truncated: bool) -> Self {
        let text = text.into();
        let segments = if text.is_empty() {
            Vec::new()
        } else {
            vec![OutputSegment {
                text: text.clone(),
                style: SegmentStyle::default(),
            }]
        };
        Self {
            format: OutputFormat::Text,
            text,
            segments,
            truncated,
        }
    }

    /// 由片段建立輸出：丟棄空片段、合併相鄰同樣式片段，`text` 由片段串接而得。
    pub fn from_segments(
        segments: impl IntoIterator<Item = OutputSegment>,
        truncated: bool,
    ) -> Self {
        let mut merged: Vec<OutputSegment> = Vec::new();
        for segment in segments {
            if segment.text.is_empty() {
                continue;
            }
            match merged.last_mut() {
                Some(last) if last.style == segment.style => last.text.push_str(&segment.text),
                _ => merged.push(segment),
            }
        }
        let text = merged.iter().map(|s| s.text.as_str()).collect();
        Self {
            format: OutputFormat::Text,
            text,
            segments: merged,
            truncated,
        }
    }

    /// 輸出的格式。
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    /// 輸出內容（純文字）。
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 帶樣式的片段。
    pub fn segments(&self) -> &[OutputSegment] {
        &self.segments
    }

    /// 是否還有更早的內容未回傳（超過讀取行數上限）。
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

/// `read_output` 實作回 `RuntimeError::Failed` 時，原因字串固定以這段開頭（`cockpit-herdr` 組字串、
/// `cockpit` 的 HTTP 層取 `params.detail` 時剝掉；ui-language design D4：前綴是繁中，英文介面的範本
/// 不該夾著它）。集中在這裡，兩端不各寫一份。
pub const READ_OUTPUT_FAILED_PREFIX: &str = "讀取 pane 輸出失敗：";

/// 呼叫 `AgentRuntime` 時可能發生的錯誤。
///
/// `Display` 的輸出直接就是可以拿來當 `ConnectionState::Disconnected.reason` 用的字串。
///
/// **給人看的原因（`Unavailable.reason`、`Failed` 的字串）必須由 [`crate::Message`] 的 `text()` 產生**
/// （ui-language design D4）：投影以 `Message::classify(原文)` 反推 `reason_msg`，英文介面才能依代碼翻譯；
/// 直接 `format!` 出新的繁中原文不會有任何測試失敗，但那則原因會被歸成 `raw`、英文介面退回顯示繁中原文。
/// 要新增一種失敗原因，先在 `message.rs` 新增 `Message` 變體（`text`／`msg`／`classify`），並補前端字典
/// 的 `msg.<code>`（`cockpit/tests/http.rs` 的對帳測試會擋住缺鍵）。
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// runtime 目前無法使用，但附上建議的重試間隔（例如 HERDR 還沒啟動、正在重連）。
    #[error("{reason}")]
    Unavailable {
        /// 無法使用的原因，供顯示與記錄。
        reason: String,
        /// 建議在這麼久之後重試。
        retry_after: Duration,
    },
    /// 呼叫本身失敗，沒有固定的重試間隔建議。
    #[error("{0}")]
    Failed(String),
    /// 指定的 pane 不存在。
    #[error("pane 不存在：{pane_id}")]
    PaneNotFound {
        /// 不存在的 pane id。
        pane_id: PaneId,
    },
}

impl RuntimeError {
    /// 取得建議的重試間隔；只有 `Unavailable` 會回傳 `Some`，其餘一律 `None`。
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            RuntimeError::Unavailable { retry_after, .. } => Some(*retry_after),
            RuntimeError::Failed(_) => None,
            RuntimeError::PaneNotFound { .. } => None,
        }
    }
}

/// 持有一組背景 task 的 `JoinHandle`，drop 時把它們全部 `abort()`。
///
/// 用來保證「釋放事件流即關閉連線與子程序」：使用者只要 drop `RuntimeEvents`，
/// 不需要自己記得收拾背景任務。
struct AbortOnDrop(Vec<JoinHandle<()>>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        for handle in &self.0 {
            handle.abort();
        }
    }
}

/// 一條已合併的 runtime 事件流；每個項目是一個 `RuntimeEvent` 或是帶原因的 `RuntimeError`。
///
/// 只提供 `next()`，不實作 `futures::Stream`（`cockpit-core` 不引入 `futures`）。
///
/// 唯一的建構方式是 [`RuntimeEvents::channel`]：容量固定為 [`RuntimeEvents::CAPACITY`]，
/// 呼叫端無法用任意容量的 receiver 拼出一個 `RuntimeEvents`。用法：
///
/// ```ignore
/// let (tx, unstarted) = RuntimeEvents::channel();
/// let task = tokio::spawn(async move { /* 用 tx 送事件 */ });
/// let events = unstarted.start(vec![task]);
/// ```
pub struct RuntimeEvents {
    rx: mpsc::Receiver<Result<RuntimeEvent, RuntimeError>>,
    _guard: AbortOnDrop,
}

impl RuntimeEvents {
    /// 事件通道的容量；`channel()` 建出來的通道一律是這個容量。
    pub const CAPACITY: usize = 1024;

    /// 建立一組容量固定為 `CAPACITY` 的事件通道。呼叫端先用回傳的 `Sender`
    /// spawn 產生事件的背景 task，等拿到這些 task 的 `JoinHandle` 之後，
    /// 再呼叫 [`UnstartedEvents::start`] 組出真正的 `RuntimeEvents`。
    pub fn channel() -> (
        mpsc::Sender<Result<RuntimeEvent, RuntimeError>>,
        UnstartedEvents,
    ) {
        let (tx, rx) = mpsc::channel(Self::CAPACITY);
        (tx, UnstartedEvents { rx })
    }

    /// 取得下一筆事件；事件流關閉時回傳 `None`。
    pub async fn next(&mut self) -> Option<Result<RuntimeEvent, RuntimeError>> {
        self.rx.recv().await
    }
}

/// [`RuntimeEvents::channel`] 回傳的另一半：已經拿到固定容量的 receiver，但還沒有
/// 綁定產生事件的背景 task。呼叫 [`UnstartedEvents::start`] 把這些 task 的
/// `JoinHandle` 交給它，換回可以使用的 `RuntimeEvents`。
pub struct UnstartedEvents {
    rx: mpsc::Receiver<Result<RuntimeEvent, RuntimeError>>,
}

impl UnstartedEvents {
    /// 綁定產生事件的背景 task，組出完整的 `RuntimeEvents`；這些 task 會在
    /// `RuntimeEvents` 被 drop 時一併 `abort()`。
    pub fn start(self, tasks: Vec<JoinHandle<()>>) -> RuntimeEvents {
        RuntimeEvents {
            rx: self.rx,
            _guard: AbortOnDrop(tasks),
        }
    }
}

/// 一個可以被驅動器與狀態庫消費的 agent runtime 抽象（例如 HERDR）。
///
/// 實作者要能回報自己的 `RuntimeId`、提供一份 `RuntimeSnapshot`，以及建立一條
/// 已合併的事件流。
#[async_trait::async_trait]
pub trait AgentRuntime: Send + Sync {
    /// 這個 runtime 的識別碼。
    fn id(&self) -> &RuntimeId;

    /// 取得目前的完整狀態快照。
    async fn snapshot(&self) -> Result<RuntimeSnapshot, RuntimeError>;

    /// 建立一條已合併的事件流，供狀態庫持續套用增量更新。
    async fn subscribe(&self) -> Result<RuntimeEvents, RuntimeError>;

    /// 讀取一個 pane 目前的輸出，最多回傳 `max_lines` 行。不經過狀態庫，呼叫這個方法不會
    /// 影響投影 version（design D2、D4；spec `runtime-model`「讀取輸出不動狀態庫」）。
    async fn read_output(&self, pane: &PaneId, max_lines: u32) -> Result<PaneOutput, RuntimeError>;
}
