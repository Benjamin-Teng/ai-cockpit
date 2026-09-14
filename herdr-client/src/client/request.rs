//! `Request` trait 與 observer 子集提供的 request 型別（design D10；spec「只提供 observer
//! 子集的 method」——只有 `session.snapshot`、`pane.read`，不提供任何會改變 HERDR 狀態的
//! method；`events.subscribe` 由第 4 組第三個 task 加入，見 `herdr-event-subscription` spec）。

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::types::{PaneReadParams, PaneReadResultEnvelope, SessionSnapshotResult};

/// 一個可以送給 HERDR 的 request：固定的 `method` 名稱、序列化後即為 `params` 內容、對應的
/// 回應型別。
///
/// # Sealed（Codex task review fix round 1 finding 1）
///
/// 這是 sealed trait：只有本 crate 內的型別可以實作它。公開型別清單雖然只有
/// `SessionSnapshotRequest`／`PaneReadRequest` 兩種，但如果任何一個 crate 外的型別能自己
/// 實作 `Request`，就能自訂任意 `METHOD`（例如 `"server.stop"`、`"agent.prompt"`）交給
/// `Client::request` 送出——HERDR 的 socket API 沒有認證（ADR-0001），這會直接繞過「只提供
/// observer 子集」這條安全邊界。下面這段示範外部型別嘗試實作 `Request` 會編譯失敗
/// （實測錯誤碼是 `E0277`：`MyEvilRequest` 沒有實作編譯器看得到、但外部程式碼無法命名也
/// 無法實作的 `client::request::private::Sealed`，見 fix round 2 的 TDD 證據，不是用猜的。
/// 附註：fence 上的 `,E0277` 只是給人看的文件標記——實測過 `rustdoc` 的 `compile_fail`
/// 不會真的檢查附加的錯誤碼是否相符，故意寫錯成 `E0603` 一樣會通過，見 fix round 2 報告；
/// 真正驗證錯誤碼是否對的是這段文件註解本身與下面的 TDD 紀錄，不是這個 fence 屬性）：
///
/// ```compile_fail,E0277
/// struct MyEvilRequest;
///
/// impl serde::Serialize for MyEvilRequest {
///     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
///     where
///         S: serde::Serializer,
///     {
///         serializer.serialize_unit()
///     }
/// }
///
/// // 編譯失敗：`private::Sealed` 不是公開路徑，crate 外部無法命名、也就無法實作它。
/// impl herdr_client::client::Request for MyEvilRequest {
///     const METHOD: &'static str = "server.stop";
///     type Response = serde_json::Value;
/// }
/// ```
pub trait Request: private::Sealed + Serialize {
    /// 送出的 `method` 欄位。
    const METHOD: &'static str;
    /// 成功回應 `result` 對應的型別。
    type Response: DeserializeOwned;
}

/// sealed trait 的模組：`pub(crate)`——crate 內任何地方都能命名
/// `crate::client::request::private::Sealed`（Codex task review fix round 2 finding 1），
/// crate 外部一律看不到（`pub(crate)` 的可見範圍就是本 crate，不會外洩）。
///
/// **不是單純 `mod private`**：fix round 1 原本用完全 private 的 `mod private`，註解宣稱
/// 「之後 task 4.3 的 `events.subscribe` 型別如果放在 `client` 底下的子模組，一樣能
/// `use super::private::Sealed`」，但這個說法不成立——module-private 項目只對**定義它的
/// 模組與該模組的子孫模組**可見，`private` 定義在 `client::request` 裡，只有
/// `client::request` 自己與它的子孫（例如 `client::request::foo`）看得到；4.3 預定放的
/// `client::subscribe` 是 `client` 的另一個子模組，跟 `client::request`是手足關係、不是
/// `client::request` 的子孫，對 `client::subscribe` 而言 `super` 解析到的是 `client`，
/// 那裡沒有 `private` 這個名字，`super::private::Sealed` 會編譯失敗（`E0603`：module
/// `private` is private，見 fix round 2 的 RED 證據）。`pub(crate)` 才是正確的邊界：
/// crate 內任何模組都能沿完整路徑命名它，不受模組樹的父子關係限制。
pub(crate) mod private {
    pub trait Sealed {}
}

impl private::Sealed for SessionSnapshotRequest {}
impl private::Sealed for PaneReadRequest {}

/// `session.snapshot`：沒有參數，序列化為空物件 `{}`（schema 的 `EmptyParams`，接受任意物件，
/// 不限制欄位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct SessionSnapshotRequest {}

impl Request for SessionSnapshotRequest {
    const METHOD: &'static str = "session.snapshot";
    type Response = SessionSnapshotResult;
}

/// `pane.read`：change 1 不對真機呼叫（design D10），型別與 `Request` 實作只供合約測試驗證
/// 序列化、以及 change 3 之後直接使用。
///
/// `PaneReadRequest` 是單一欄位的 newtype：`#[derive(Serialize)]` 對只有一個未命名欄位的
/// struct 會產生「newtype struct」序列化，serde_json 會原樣序列化內層值——也就是說
/// `PaneReadRequest(params)` 序列化出來的 JSON 與直接序列化 `params` 完全相同，正好符合
/// `RequestEnvelope.params` 需要的形狀（`PaneReadParams` 本身就是扁平的請求參數物件）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaneReadRequest(pub PaneReadParams);

impl Request for PaneReadRequest {
    const METHOD: &'static str = "pane.read";
    type Response = PaneReadResultEnvelope;
}
