//! 假 HERDR 的可設定行為（design D7、D12）。

use std::collections::{HashMap, HashSet};

use crate::types::Subscription;

use super::script::Step;

/// 一般 request／response method（目前只會用到 `"session.snapshot"`；`"events.subscribe"`
/// 走另一套邏輯，見 `FakeHerdrConfig::with_subscribe_script`）收到 request 後要回什麼。
#[derive(Debug, Clone)]
pub enum MethodResponse {
    /// 成功回應：`result` 欄位的內容（含 `"type"` 標籤，例如
    /// `{"type":"session_snapshot","snapshot":{...}}`）；`id` 一律回實際收到的 request id。
    Success(serde_json::Value),
    /// `error` 物件（spec「回應對應」的「遠端錯誤」情境）。
    RemoteError { code: String, message: String },
    /// 成功回應的 `result` 內容不變，但 `id` 刻意與 request 不同（spec「id 不符」情境）。
    WrongId(serde_json::Value),
    /// 整行不是合法 JSON（spec「回應不是 JSON」情境）。
    NonJson(String),
    /// `result` 的 `"type"` 為 `"pong"`（spec「result 形狀不符」情境的具體例子）。
    PongResult,
    /// 收到 request 後直接關閉連線，不回應（spec「回應前連線中斷」情境）。
    CloseBeforeReply,
}

/// `events.subscribe` 收到的訂閱清單要符合什麼條件，才套用某條 `SubscribeRule`（fix round 1
/// finding 4：讓不同連線可以依訂閱內容重播不同腳本，對照 spec「兩條訂閱連線同時存在」）。
#[derive(Debug, Clone)]
pub enum SubscribeMatcher {
    /// 一律符合。
    Any,
    /// 清單裡沒有任何一筆 `pane.agent_status_changed`（也就是純 24 種生命週期訂閱）。
    LifecycleOnly,
    /// 清單裡至少一筆是 `pane.agent_status_changed`（不論哪個 pane_id）。
    PerPane,
    /// 清單裡至少一筆是指定 `pane_id` 的 `pane.agent_status_changed`。
    ContainsPaneId(String),
}

impl SubscribeMatcher {
    fn matches(&self, subscriptions: &[Subscription]) -> bool {
        match self {
            SubscribeMatcher::Any => true,
            SubscribeMatcher::LifecycleOnly => subscriptions
                .iter()
                .all(|s| !matches!(s, Subscription::PaneAgentStatusChanged { .. })),
            SubscribeMatcher::PerPane => subscriptions
                .iter()
                .any(|s| matches!(s, Subscription::PaneAgentStatusChanged { .. })),
            SubscribeMatcher::ContainsPaneId(id) => subscriptions.iter().any(
                |s| matches!(s, Subscription::PaneAgentStatusChanged { pane_id } if pane_id == id),
            ),
        }
    }
}

/// 一條 `events.subscribe` 分派規則：訂閱清單符合 `matcher` 時，通過探測的連線就重播
/// `steps`。
#[derive(Debug, Clone)]
pub struct SubscribeRule {
    pub matcher: SubscribeMatcher,
    pub steps: Vec<Step>,
}

/// 沒有任何規則符合時的預設腳本：回 `subscription_started` 後單純掛著，直到 `FakeHerdr`
/// 被 drop（fix round 1 finding 4 的預設行為，與探測失敗〔直接關閉〕明確不同）。
const DEFAULT_SUBSCRIBE_STEPS: &[Step] = &[Step::Hold];

/// `FakeHerdr::start` 的設定：一般 method 的回應、`events.subscribe` 的分派規則、訂閱探測
/// 失敗的 `pane_id` 集合（design D12）。用 builder 風格組裝，欄位不公開以留設計彈性。
#[derive(Debug, Clone, Default)]
pub struct FakeHerdrConfig {
    responses: HashMap<String, MethodResponse>,
    subscribe_rules: Vec<SubscribeRule>,
    failing_probe_pane_ids: HashSet<String>,
}

impl FakeHerdrConfig {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 設定某個一般 method（例如 `"session.snapshot"`）收到 request 後的回應行為。
    #[must_use]
    pub fn with_method_response(
        mut self,
        method: impl Into<String>,
        response: MethodResponse,
    ) -> Self {
        self.responses.insert(method.into(), response);
        self
    }

    /// 設定 `"session.snapshot"` 的成功回應：直接給 `result` 欄位的內容。
    #[must_use]
    pub fn with_snapshot_result(self, result: serde_json::Value) -> Self {
        self.with_method_response("session.snapshot", MethodResponse::Success(result))
    }

    /// 設定 `"session.snapshot"` 的成功回應：從整行 fixture（含 `id`／`result` 外層信封，例如
    /// `tests/fixtures/snapshot-p22.json`）取出 `result` 欄位。fixture 缺 `result` 欄位時
    /// panic——這代表 fixture 檔案本身格式不對，測試應該立刻看到，不該悄悄回一個空設定。
    #[must_use]
    pub fn with_snapshot_fixture_line(self, fixture: &serde_json::Value) -> Self {
        let result = fixture
            .get("result")
            .cloned()
            .unwrap_or_else(|| panic!("fixture 缺少 \"result\" 欄位: {fixture}"));
        self.with_snapshot_result(result)
    }

    /// 設定 `events.subscribe` 通過探測後、不論訂閱內容一律重播的腳本（等同
    /// `with_subscribe_rule(SubscribeMatcher::Any, steps)`）。只需要一種腳本的測試用這個
    /// 即可；要讓不同連線依訂閱內容收到不同腳本，改用 `with_subscribe_rule`。
    #[must_use]
    pub fn with_subscribe_script(self, steps: Vec<Step>) -> Self {
        self.with_subscribe_rule(SubscribeMatcher::Any, steps)
    }

    /// 加一條 `events.subscribe` 分派規則：訂閱清單符合 `matcher` 時重播 `steps`。多條規則
    /// 依加入順序比對，第一個符合的生效；都不符合時走 `DEFAULT_SUBSCRIBE_STEPS`（回
    /// `subscription_started` 後單純掛著）。
    #[must_use]
    pub fn with_subscribe_rule(mut self, matcher: SubscribeMatcher, steps: Vec<Step>) -> Self {
        self.subscribe_rules.push(SubscribeRule { matcher, steps });
        self
    }

    /// 設定訂閱探測會失敗的 `pane_id` 集合（design D12）。
    #[must_use]
    pub fn with_failing_probe_pane_ids(
        mut self,
        ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.failing_probe_pane_ids = ids.into_iter().map(Into::into).collect();
        self
    }

    pub(super) fn response_for(&self, method: &str) -> Option<&MethodResponse> {
        self.responses.get(method)
    }

    /// 依訂閱清單挑第一個符合的規則的腳本；沒有規則符合時回預設腳本（見
    /// `DEFAULT_SUBSCRIBE_STEPS`）。
    pub(super) fn subscribe_steps_for(&self, subscriptions: &[Subscription]) -> &[Step] {
        self.subscribe_rules
            .iter()
            .find(|rule| rule.matcher.matches(subscriptions))
            .map(|rule| rule.steps.as_slice())
            .unwrap_or(DEFAULT_SUBSCRIBE_STEPS)
    }

    pub(super) fn failing_probe_pane_ids(&self) -> &HashSet<String> {
        &self.failing_probe_pane_ids
    }
}
