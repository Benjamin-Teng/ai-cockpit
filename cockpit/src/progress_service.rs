//! 進度寫入服務（design D3、D4；spec `pipeline-progress`「狀態檔格式與持久化」）。
//!
//! 所有會改 Domain 狀態的寫入（進度操作、設定／取消覆蓋、刪除失效覆蓋）都在同一把
//! `tokio::sync::Mutex` 內依序完成：
//!
//! 1. 從 `StoreHandle` 讀目前的 `DomainState`；
//! 2. 在 `DomainState` 的 clone 上呼叫 core 的狀態轉移（`apply_progress`／`set_active`／
//!    `set_override`／`remove_override`，目前 task 的清除規則都在那裡）與 `validate_override`，
//!    被拒絕就回錯、不落檔；
//! 3. 新狀態與目前相同（例如標記已是 none 時 clear、取消不存在的覆蓋）→ 成功、不落檔、不通知；
//! 4. `spawn_blocking` 寫同目錄 `<檔名>.tmp` 再 `std::fs::rename` 取代狀態檔；
//! 5. 落檔成功才 `StoreHandle::set_domain` 生效。
//!
//! 取消安全：整筆交易（取鎖 → 計算 → 落檔 → 生效）跑在服務自己 `tokio::spawn` 的 task 裡，
//! 由那個 task 持鎖到結束；呼叫端（例如客戶端斷線時被 axum drop 的 handler）只是在等它的
//! `JoinHandle`，被取消也不會中途放掉鎖、跳過 `set_domain`，後續寫入不會與仍在跑的落檔交錯。
//!
//! 例外是失效覆蓋的刪除（design D3）：落檔失敗只記 `tracing::error!`，記憶體照樣刪除——保留一筆
//! 已失效的覆蓋沒有意義，下一次任何成功寫入都會把正確狀態寫出。

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cockpit_core::{
    DomainState, Override, ProgressOp, ProjectId, Rejection, StaleOverride, StoreHandle, TaskId,
    TaskProgress, WorkstreamId, domain::binding::validate_override,
};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

use crate::progress::{STATE_FILE_VERSION, StateFile, StateOverride, StateProject, StateTask};

/// 寫入失敗的原因；變體對應 HTTP 狀態碼：`Unknown*` → 404、`PaneNotBound` → 403、`Rejected` → 409、
/// `Persist`／`Internal` → 500。
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// project 不存在。
    #[error("project 不存在：{0}")]
    UnknownProject(ProjectId),
    /// project 存在，但其中沒有這個 task。
    #[error("task 不存在：{0}")]
    UnknownTask(TaskId),
    /// project 存在，但其中沒有這條 workstream。
    #[error("workstream 不存在：{0}")]
    UnknownWorkstream(WorkstreamId),
    /// 操作被 core 規則拒絕；`Display` 即 409 本體的原因字串。
    #[error(transparent)]
    Rejected(#[from] Rejection),
    /// 狀態檔寫入失敗；記憶體狀態維持操作前的值。
    #[error("寫入狀態檔失敗（{}）：{source}", path.display())]
    Persist {
        /// 狀態檔路徑。
        path: PathBuf,
        /// 底層 I/O 錯誤。
        #[source]
        source: std::io::Error,
    },
    /// 身分判定當時依據的覆蓋事實，在寫入鎖內重驗時已不成立（例如判定後使用者改綁或取消覆蓋）；
    /// HTTP 層對應 403 `pane_not_bound`。
    #[error("pane 已不再綁定到該 workstream")]
    PaneNotBound,
    /// 執行寫入交易的 task 異常結束（panic 或 runtime 關閉時被取消）。
    #[error("寫入任務異常結束：{0}")]
    Internal(#[source] tokio::task::JoinError),
}

/// 測試用：落檔 IO 的開始與結束時點，交給 [`WriteHook`]。
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteStage {
    /// 即將在 blocking 執行緒上寫 `.tmp`。
    Start,
    /// `.tmp` 寫入與 `rename` 已結束（不論成功與否）。
    Finish,
}

/// 測試用鉤子：在 blocking 執行緒上於落檔 IO 前後被呼叫，可藉此卡住或觀察寫入。
#[doc(hidden)]
pub type WriteHook = Arc<dyn Fn(WriteStage) + Send + Sync>;

/// agent 端點判定 pane 身分時，那條 workstream 的綁定來源（review M1）。service 在寫入鎖內拿它
/// 比對 Domain 的覆蓋，確認判定依據沒有在「讀投影」與「取得鎖」之間被改綁推翻；只看 Domain
/// 狀態，不依賴 Runtime 層。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingBasis {
    /// 判定時綁定來源是自動解析：要求鎖內仍然沒有覆蓋。
    Auto,
    /// 判定時綁定來源是覆蓋：要求鎖內覆蓋仍等於這筆。
    Override(Override),
}

impl BindingBasis {
    /// 鎖內重驗：`domain` 對 `(project, workstream)` 的覆蓋事實是否仍與判定依據一致。
    fn holds(&self, domain: &DomainState, project: &ProjectId, workstream: &WorkstreamId) -> bool {
        let current = domain
            .overrides
            .get(project)
            .and_then(|m| m.get(workstream));
        match self {
            Self::Auto => current.is_none(),
            Self::Override(expected) => current == Some(expected),
        }
    }
}

struct Inner {
    handle: StoreHandle,
    path: PathBuf,
    /// 序列化所有寫入（design D4）；鎖內容為空，只用來排隊。
    lock: Mutex<()>,
    write_hook: Option<WriteHook>,
}

/// 進度寫入服務；`Clone` 便宜（內部 `Arc`），可以交給 HTTP handler 與失效覆蓋接收任務各持一份。
#[derive(Clone)]
pub struct ProgressService(Arc<Inner>);

impl ProgressService {
    /// 建立寫入服務：`handle` 是要套用新狀態的控制點，`path` 是狀態檔路徑。
    pub fn new(handle: StoreHandle, path: PathBuf) -> Self {
        Self::build(handle, path, None)
    }

    /// 測試用：同 [`ProgressService::new`]，另外在每次落檔 IO 前後呼叫 `hook`。
    #[doc(hidden)]
    pub fn with_write_hook(handle: StoreHandle, path: PathBuf, hook: WriteHook) -> Self {
        Self::build(handle, path, Some(hook))
    }

    fn build(handle: StoreHandle, path: PathBuf, write_hook: Option<WriteHook>) -> Self {
        Self(Arc::new(Inner {
            handle,
            path,
            lock: Mutex::new(()),
            write_hook,
        }))
    }

    /// 對一個 task 套用進度操作（spec `pipeline-progress`「進度寫入端點」）。
    ///
    /// # Errors
    ///
    /// project／task 不存在、操作被拒絕、或狀態檔寫入失敗時回傳 [`WriteError`]；任何錯誤都不改
    /// 記憶體狀態。
    pub async fn apply_progress(
        &self,
        project: &ProjectId,
        task: &TaskId,
        op: ProgressOp,
    ) -> Result<(), WriteError> {
        let project = project.clone();
        let task = task.clone();
        self.write(move |domain| {
            let mut new_domain = domain.clone();
            ensure_task(&mut new_domain, &project, &task)?;
            new_domain.apply_progress(&project, &task, op)?;
            Ok(new_domain)
        })
        .await
    }

    /// 宣告目前 task：把 `task` 設為其所屬 workstream 的目前 task（spec `agent-reporting`「宣告目前
    /// task」；progress-model task 3.2，design D3）。重複宣告同一個 task 時狀態不變，成功、不落檔。
    /// 「這個 pane 是否綁定到該 workstream」的身分判定在 HTTP 層（task 3.4），這裡只在鎖內以
    /// `basis` 重驗判定依據的覆蓋事實（review M1），不符回 [`WriteError::PaneNotBound`]。
    ///
    /// # Errors
    ///
    /// project／task 不存在（[`WriteError::UnknownProject`]／[`WriteError::UnknownTask`]）、task 已有
    /// 標記（[`WriteError::Rejected`]）、或狀態檔寫入失敗時回傳 [`WriteError`]；任何錯誤都不改記憶體。
    pub async fn declare_active(
        &self,
        project: &ProjectId,
        task: &TaskId,
        basis: &BindingBasis,
    ) -> Result<(), WriteError> {
        let project = project.clone();
        let task = task.clone();
        let basis = basis.clone();
        self.write(move |domain| {
            let mut new_domain = domain.clone();
            let workstream = ensure_task(&mut new_domain, &project, &task)?;
            if !basis.holds(domain, &project, &workstream) {
                return Err(WriteError::PaneNotBound);
            }
            new_domain.set_active(&project, &workstream, &task)?;
            Ok(new_domain)
        })
        .await
    }

    /// agent 推進：同一筆交易先推進 `task` 的 stage、再把它設為所屬 workstream 的目前 task（spec
    /// `agent-reporting`「agent 推進」；progress-model task 3.2）。兩步在同一把鎖內、只落檔一次；
    /// 任一步被拒絕整筆不生效。身分判定與 `basis` 重驗同 [`ProgressService::declare_active`]。
    ///
    /// # Errors
    ///
    /// 同 [`ProgressService::declare_active`]；推進規則被拒絕（最後一站、已有標記）也走
    /// [`WriteError::Rejected`]。
    pub async fn agent_advance(
        &self,
        project: &ProjectId,
        task: &TaskId,
        basis: &BindingBasis,
    ) -> Result<(), WriteError> {
        let project = project.clone();
        let task = task.clone();
        let basis = basis.clone();
        self.write(move |domain| {
            let mut new_domain = domain.clone();
            let workstream = ensure_task(&mut new_domain, &project, &task)?;
            if !basis.holds(domain, &project, &workstream) {
                return Err(WriteError::PaneNotBound);
            }
            new_domain.apply_progress(&project, &task, ProgressOp::Advance)?;
            new_domain.set_active(&project, &workstream, &task)?;
            Ok(new_domain)
        })
        .await
    }

    /// 設定一條 workstream 的畫面覆蓋（spec `pipeline-progress`「綁定覆蓋端點」`PUT`）。
    ///
    /// # Errors
    ///
    /// project／workstream 不存在、`validate_override` 拒絕、或狀態檔寫入失敗時回傳
    /// [`WriteError`]；任何錯誤都不改記憶體狀態。
    pub async fn set_override(
        &self,
        project: &ProjectId,
        workstream: &WorkstreamId,
        override_: Override,
    ) -> Result<(), WriteError> {
        let handle = self.0.handle.clone();
        let project = project.clone();
        let workstream = workstream.clone();
        self.write(move |domain| {
            ensure_workstream(domain, &project, &workstream)?;
            handle.with_store(|store| validate_override(&override_, store))?;

            let mut new_domain = domain.clone();
            new_domain.set_override(&project, &workstream, override_);
            Ok(new_domain)
        })
        .await
    }

    /// 取消一條 workstream 的畫面覆蓋（`DELETE`）；覆蓋本來就不存在時成功且不落檔。
    ///
    /// # Errors
    ///
    /// project／workstream 不存在、或狀態檔寫入失敗時回傳 [`WriteError`]。
    pub async fn clear_override(
        &self,
        project: &ProjectId,
        workstream: &WorkstreamId,
    ) -> Result<(), WriteError> {
        let project = project.clone();
        let workstream = workstream.clone();
        self.write(move |domain| {
            ensure_workstream(domain, &project, &workstream)?;
            let mut new_domain = domain.clone();
            new_domain.remove_override(&project, &workstream);
            Ok(new_domain)
        })
        .await
    }

    /// 刪除投影判定失效的覆蓋（design D3）。在鎖內逐筆確認目前覆蓋**仍等於**通知中的那筆才刪
    /// （不等代表使用者已改綁，略過）；有任何刪除才落檔。落檔失敗記 `tracing::error!`，記憶體
    /// 照樣刪除。整筆交易同樣跑在服務自己的 task 裡（取消安全，見模組文件）。
    pub async fn remove_stale(&self, stale: Vec<StaleOverride>) {
        let inner = Arc::clone(&self.0);
        let transaction = tokio::spawn(async move {
            let _guard = inner.lock.lock().await;
            let current = inner.handle.with_domain(Clone::clone);

            let mut new_domain = current.clone();
            for item in &stale {
                let still_same = new_domain
                    .overrides
                    .get(&item.project)
                    .and_then(|m| m.get(&item.workstream))
                    == Some(&item.override_);
                if still_same {
                    new_domain.remove_override(&item.project, &item.workstream);
                }
            }
            if new_domain == current {
                return;
            }

            if let Err(error) = inner.persist(&new_domain).await {
                tracing::error!(%error, "刪除失效覆蓋後寫入狀態檔失敗；記憶體中照樣刪除");
            }
            inner.handle.set_domain(new_domain);
        });
        if let Err(error) = transaction.await {
            tracing::error!(%error, "刪除失效覆蓋的寫入任務異常結束");
        }
    }

    /// 起一個任務持續接收投影送來的失效覆蓋清單並逐批 [`ProgressService::remove_stale`]；
    /// 送端關閉時結束。
    pub fn spawn_stale_remover(
        &self,
        mut rx: mpsc::UnboundedReceiver<Vec<StaleOverride>>,
    ) -> JoinHandle<()> {
        let service = self.clone();
        tokio::spawn(async move {
            while let Some(batch) = rx.recv().await {
                service.remove_stale(batch).await;
            }
        })
    }

    /// 共用寫入路徑：在服務自己的 task 裡「鎖 → 算新狀態（錯誤即回）→ 沒變就結束 → 落檔 →
    /// 成功才生效」；呼叫端只等 `JoinHandle`，被取消也不影響交易完整跑完。
    async fn write(
        &self,
        compute: impl FnOnce(&DomainState) -> Result<DomainState, WriteError> + Send + 'static,
    ) -> Result<(), WriteError> {
        let inner = Arc::clone(&self.0);
        tokio::spawn(async move {
            let _guard = inner.lock.lock().await;
            let current = inner.handle.with_domain(Clone::clone);
            let new_domain = compute(&current)?;
            if new_domain == current {
                return Ok(());
            }
            inner.persist(&new_domain).await?;
            inner.handle.set_domain(new_domain);
            Ok(())
        })
        .await
        .unwrap_or_else(|join_error| Err(WriteError::Internal(join_error)))
    }
}

impl Inner {
    /// 在 blocking 執行緒上把 `domain` 寫成狀態檔（`.tmp` 再 `rename`）。
    async fn persist(&self, domain: &DomainState) -> Result<(), WriteError> {
        let json = serde_json::to_vec_pretty(&to_state_file(domain))
            .expect("StateFile 只含字串與列舉，序列化不會失敗");
        let path = self.path.clone();
        let hook = self.write_hook.clone();
        let result = tokio::task::spawn_blocking({
            let path = path.clone();
            move || {
                if let Some(hook) = &hook {
                    hook(WriteStage::Start);
                }
                let result = write_atomically(&path, &json);
                if let Some(hook) = &hook {
                    hook(WriteStage::Finish);
                }
                result
            }
        })
        .await
        .unwrap_or_else(|join_error| Err(std::io::Error::other(join_error)));
        result.map_err(|source| WriteError::Persist { path, source })
    }
}

fn find_project<'a>(
    domain: &'a DomainState,
    project: &ProjectId,
) -> Result<&'a cockpit_core::ProjectDef, WriteError> {
    domain
        .projects
        .iter()
        .find(|p| &p.id == project)
        .ok_or_else(|| WriteError::UnknownProject(project.clone()))
}

fn ensure_workstream(
    domain: &DomainState,
    project: &ProjectId,
    workstream: &WorkstreamId,
) -> Result<(), WriteError> {
    let def = find_project(domain, project)?;
    if def.workstreams.iter().any(|w| &w.id == workstream) {
        Ok(())
    } else {
        Err(WriteError::UnknownWorkstream(workstream.clone()))
    }
}

/// 確認 project 與 task 存在並回傳 task 所屬的 workstream；`new_domain.progress` 缺這個 task 的項目
/// 時補初始進度，滿足 `DomainState::apply_progress` 的前置條件。
fn ensure_task(
    domain: &mut DomainState,
    project: &ProjectId,
    task: &TaskId,
) -> Result<WorkstreamId, WriteError> {
    let def = find_project(domain, project)?;
    let task_def = def
        .tasks
        .iter()
        .find(|t| &t.id == task)
        .ok_or_else(|| WriteError::UnknownTask(task.clone()))?;
    let workstream = task_def.workstream.clone();
    let initial = TaskProgress::initial(task_def);
    domain
        .progress
        .entry(project.clone())
        .or_default()
        .entry(task.clone())
        .or_insert(initial);
    Ok(workstream)
}

/// `DomainState` → 狀態檔形狀：所有 project 與其全部 task 都寫出，沒有覆蓋的 project 寫 `{}`。
fn to_state_file(domain: &DomainState) -> StateFile {
    let projects = domain
        .projects
        .iter()
        .map(|def| {
            let progress = domain.progress.get(&def.id);
            let tasks = def
                .tasks
                .iter()
                .map(|task| {
                    let current = progress
                        .and_then(|m| m.get(&task.id))
                        .cloned()
                        .unwrap_or_else(|| TaskProgress::initial(task));
                    (
                        task.id.as_str().to_string(),
                        StateTask {
                            stage: current.stage,
                            mark: current.mark,
                        },
                    )
                })
                .collect();
            let overrides = domain
                .overrides
                .get(&def.id)
                .map(|m| {
                    m.iter()
                        .map(|(ws, o)| {
                            (
                                ws.as_str().to_string(),
                                StateOverride {
                                    runtime: o.runtime.as_str().to_string(),
                                    pane_id: o.pane_id.as_str().to_string(),
                                },
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let active = domain
                .active
                .get(&def.id)
                .map(|m| {
                    m.iter()
                        .map(|(ws, task)| (ws.as_str().to_string(), task.as_str().to_string()))
                        .collect()
                })
                .unwrap_or_default();
            (
                def.id.as_str().to_string(),
                StateProject {
                    tasks,
                    overrides,
                    active: Some(active),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    StateFile {
        version: STATE_FILE_VERSION,
        projects,
    }
}

/// 寫同目錄 `<檔名>.tmp`（含 `sync_all`）再 `rename` 取代目標；rename 失敗時盡力刪掉暫存檔。
fn write_atomically(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut tmp_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("狀態檔路徑沒有檔名"))?
        .to_os_string();
    tmp_name.push(".tmp");
    let tmp_path = path.with_file_name(tmp_name);

    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    if let Err(error) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(error);
    }
    Ok(())
}
