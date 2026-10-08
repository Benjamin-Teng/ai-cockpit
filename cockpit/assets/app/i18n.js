// i18n.js：介面語言的決定、字典與查詢（ui-language task 1.1；design D1、D2）。
//
// 第一個載入的腳本，放在 index.html 的 <head>（同步、擋住繪製；其餘 <script> 順序見該檔註解）：語言與 <html lang>
// 在 <body> 解析之前就決定好，其他模組在載入時讀到的 `t()` 就已是正確語言，模組層級的常數表（`ERROR_TEXT` 等）
// 可以照舊在載入時建立，只是內容改由 `t()` 取得。靜態節點（帶 data-i18n）等 DOMContentLoaded 才換成該語言，
// 英文介面在那之前由 `html.i18n-pending` 先藏起來，避免閃出繁中後備文字。
//
// 公開介面（`window.cockpitI18n`）：
//   - `lang`：目前介面語言，`"zh"`（繁體中文）或 `"en"`；載入時決定一次，之後不變（切換一律重新載入頁面，design D3）。
//   - `t(key, params)`：查字典；`{name}` 具名佔位符以 `params.name` 代入（沒給的佔位符原樣留著）；
//     鍵不存在時回傳鍵本身並 `console.warn` 一次（開發期能發現漏字，畫面也不會空白）。
//   - `tn(key, n, params)`：依數量查複數鍵，n 為 1 用 `<key>.one`、其餘用 `<key>.other`，`{n}` 自動代入（見函式註解）。
//   - `tMsg(msgObj, fallbackText)`：後端訊息代碼的翻譯（design D4；規則見函式註解）：英文依 `msg.<code>` 範本與參數顯示，
//     代碼未知、欄位缺漏或參數不齊時退回原文，不 warn；繁中介面顯示後端原文。
//   - `setLang(lang)`：把 `"zh"`／`"en"` 寫入 localStorage 的 `cockpit.lang` 並重新載入頁面（頂列切換按鈕在 render.js、
//     點擊由 actions.js 分派到這裡）；非法值或寫入失敗（儲存不可用）時不動作。
//   - `canPersist`：localStorage 是否可寫（載入時偵測一次）；false 時切換按鈕停用。
//   - 另在載入時監聽 `storage` 事件：其他視窗改了 `cockpit.lang` 且語言與本視窗不同時，本視窗重新載入。
//   - `resolveLang({saved, languages, timeZone})`：純函式，規則見 spec「介面語言的決定」，回傳 `"zh"`／`"en"`；
//     驗收腳本直接呼叫它驗邊界（Chrome 的 --lang 會改寫 zh-SG／zh-HK，端到端測不到）。
//   - `apply(root)`：把 `root`（預設整份文件）底下帶 `data-i18n` 的節點換成目前語言（載入時已對整份文件套用一次，
//     之後動態建立的節點要翻譯可再呼叫）。
//   - `dictionaries`：`{zh, en}` 兩份字典本身（驗收腳本比對鍵與佔位符用；不要在執行期改它）。
//
// 靜態節點標記（index.html）：
//   - `data-i18n="<key>"`：節點的 textContent 換成字典值。
//   - `data-i18n="<key>" data-i18n-attr="<屬性名>"`：改成設定該屬性（`aria-label`、`title`、`placeholder` 等，一次一個），
//     不動 textContent。
//   - `data-i18n-params='{"name":"值"}'`（選用）：該節點的 `t()` 參數（JSON 物件）。
//   節點內原本寫的文字是繁中字典值的逐字副本（首份投影到達前、或腳本沒載入時的後備顯示）。
//
// 鍵命名慣例：`<模組>.<區塊>.<名稱>`，全小寫模組前綴＋camelCase 區段（`index.tab.files`）。模組前綴對應來源檔：
// `index`（index.html）、`render`、`actions`、`output`、`files`、`viewers`、`git`、`notify`，`msg` 給後端訊息代碼
// （design D1、D4）。兩份字典必須有完全相同的鍵，同一個鍵兩種語言的佔位符集合也必須相同
// （`docs/research/2026-10-03/i18n-check.js` ① 檢查）。
//
// 「done 不是完成」守門（design D5）：描述 HERDR `done` 的字典鍵，任何一段（以 `.` 分段）以 `done` 或 `agentDone` 開頭，
// 例如 `render.agentDone.hint`、`notify.kind.done`、`notify.kind.doneBody`。這類鍵的繁中值不得含「完成」，
// 英文值不得含 complete／completed／finished（不分大小寫）；`cockpit/tests/http.rs` 讀本檔原始碼檢查。
// 新增描述 `done` 的文字時務必照這個命名，不然守門抓不到。
//
// 不翻譯的產品詞彙（Project、Live Output、connecting、AI Agent Cockpit、Factory Floor 等）兩種語言都照原文，
// 不放進字典（spec「介面文字涵蓋範圍」）；整句裡夾帶的產品詞彙則照原文寫在兩份字典的句子中。
(function () {
  "use strict";

  var STORAGE_KEY = "cockpit.lang";

  // 時區名單與宣傳頁（site/index.html 的 head 腳本）同一套：台港澳中。
  var ZH_ZONES = [
    "Asia/Taipei", "Asia/Hong_Kong", "Asia/Macau", "Asia/Macao", "Asia/Shanghai", "Asia/Chongqing", "Asia/Chungking",
    "Asia/Harbin", "Asia/Urumqi", "Asia/Kashgar", "PRC", "ROC", "Hongkong"
  ];
  // 第一順位瀏覽器語言：zh、zh-Hant、zh-Hans，其後可再接 -TW／-HK／-MO／-CN 之一（不分大小寫）。
  // zh-SG、zh-Hant-SG 等不在內，落到時區判斷。
  var ZH_LANG_RE = /^zh(-(hant|hans))?(-(tw|hk|mo|cn))?$/;

  // spec「介面語言的決定」：
  //   saved     localStorage 的 cockpit.lang 值（讀不到時給 null／undefined）
  //   languages navigator.languages 風格的陣列，只看第一個；空或沒有時視為沒有語言資訊
  //   timeZone  Intl 解析出的時區名稱（Intl 不可用時給空字串）
  function resolveLang(input) {
    var o = input || {};
    if (o.saved === "zh" || o.saved === "en") return o.saved;
    var first = String((o.languages && o.languages[0]) || "").toLowerCase();
    if (ZH_LANG_RE.test(first)) return "zh";
    if (ZH_ZONES.indexOf(o.timeZone) >= 0) return "zh";
    return "en";
  }

  // 讀環境（localStorage、navigator、Intl 任一不可用都視為沒有該項資訊）。
  function detectLang() {
    var saved = null;
    try {
      saved = window.localStorage.getItem(STORAGE_KEY);
    } catch (e) {
      /* 隱私模式或封鎖網站資料：忽略 */
    }
    var languages = [];
    try {
      var nav = window.navigator || {};
      var first = (nav.languages && nav.languages[0]) || nav.language;
      if (first) languages = [first];
    } catch (e) {
      /* 沒有 navigator：忽略 */
    }
    var timeZone = "";
    try {
      timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone || "";
    } catch (e) {
      /* 沒有 Intl：忽略 */
    }
    return resolveLang({ saved: saved, languages: languages, timeZone: timeZone });
  }

  // 字典。繁中是現行畫面文字的逐字副本（既有驗收腳本以這些字串為選取器），英文是對應譯文。
  var zh = {
    // index.html 的靜態文字與屬性
    "index.leftTabs.aria": "左欄",
    "index.tab.files": "檔案",
    "index.tab.changes": "變更",
    "index.files.empty": "先在 Factory Floor 或 runtime 清單選一個 pane",
    "index.reviewTabs.aria": "分頁",
    "index.events.title": "最近事件",
    "index.channel.title": "瀏覽器 → cockpit 服務：{state}",
    "index.channel.label": "cockpit 服務",
    // render.js 頂列語言切換按鈕：文字、lang、可及名稱都寫「目標語言」的樣子（兩份字典各用目標語言寫，
    // 見 render.js renderLangToggle）。
    "render.lang.text": "EN",
    "render.lang.code": "en",
    "render.lang.aria": "Switch to English",
    "render.lang.needsStorage": "需要瀏覽器儲存空間才能切換語言",
    // render.js、actions.js、output.js 的介面文字（ui-language task 2.1）。拼接句一律是整句範本加具名佔位符；
    // 計數用 tn()，繁中兩個 .one／.other 值相同、英文分單複數。
    // 「看輸出」「綁定到這裡」等按鈕名稱被別的句子引用時以佔位符帶入（{viewOutput}、{bindHere}），不重複寫死。
    "render.lamp.stale": "最後已知",
    "render.lamp.title": "cockpit → HERDR runtime {runtime}：{state}",
    "render.lamp.titleStale": "cockpit → HERDR runtime {runtime}：{state}（最後已知）",
    "render.notify.label": "通知設定",
    "render.pane.focus": "作用中",
    "render.pane.focusTitle": "HERDR 目前聚焦的 pane",
    "render.pane.bindHere": "綁定到這裡",
    "render.warning.count.one": "警告 {n}",
    "render.warning.count.other": "警告 {n}",
    "render.binding.rebound": "改綁",
    "render.binding.unbound": "未綁定",
    "render.binding.ambiguous.one": "歧義（{n}）",
    "render.binding.ambiguous.other": "歧義（{n}）",
    "render.binding.runtimeDisconnected": "runtime 未連線",
    "render.binding.none": "無綁定",
    "render.task.back": "退回",
    "render.task.advance": "推進",
    "render.task.clearMark": "清除標記",
    "render.row.undeclared": "工作中・未宣告 task",
    "render.row.viewOutput": "看輸出",
    "render.row.rebind": "改綁",
    "render.row.undoRebind": "取消改綁",
    "render.row.worktree": "worktree：{name}",
    "render.rebind.banner": "改綁模式：為 {project} / {workstream} 選一個 pane，按該列的「{bindHere}」",
    "render.rebind.cancel": "取消",
    "render.error.close": "關閉",
    "render.projects.empty": "沒有 Project",
    // 偵測到的 repo 區與空狀態（repo-projects task 5.1；spec cockpit-dashboard「Project 切換」：空狀態指向偵測區、不提重啟）。
    "render.floor.empty": "在左欄「偵測到的 repo」按「加入」，即可在這裡看到它的 Factory Floor",
    "render.detected.title": "偵測到的 repo",
    "render.detected.empty": "在 git repo 裡開啟 pane，它所屬的 repo 就會列在這裡",
    "render.detected.paneCount.one": "{n} 個 pane",
    "render.detected.paneCount.other": "{n} 個 pane",
    "render.detected.add": "加入",
    "render.detected.addLabel": "加入 {name}",
    "render.detected.adding": "加入中…",
    "render.detected.addingLabel": "加入中… {name}",
    // Repo Project 的管理選單與對話框（repo-projects task 5.2；spec cockpit-dashboard「Project 切換」）。
    "render.projectMenu.label": "管理 {name}",
    "render.projectMenu.rename": "改名",
    "render.projectMenu.editStages": "編輯 stage",
    "render.projectMenu.remove": "移除",
    "actions.dialog.cancel": "取消",
    "actions.dialog.save": "儲存",
    "actions.dialog.rename.title": "改名：{name}",
    "actions.dialog.rename.label": "名稱",
    "actions.dialog.stages.title": "編輯 stage：{name}",
    "actions.dialog.stages.hint": "順序就是 Factory Floor 由左到右的欄。被刪除的 stage 裡的 task 會移到第一個 stage。",
    "actions.dialog.stages.nameLabel": "第 {n} 個 stage 的名稱",
    "actions.dialog.stages.up": "上移",
    "actions.dialog.stages.upLabel": "上移第 {n} 個 stage",
    "actions.dialog.stages.down": "下移",
    "actions.dialog.stages.downLabel": "下移第 {n} 個 stage",
    "actions.dialog.stages.delete": "刪除",
    "actions.dialog.stages.deleteLabel": "刪除第 {n} 個 stage",
    "actions.dialog.stages.add": "新增 stage",
    "actions.dialog.stages.from": "原為 {from}",
    "actions.dialog.stages.added": "新增的 stage",
    "actions.dialog.remove.title": "移除 Repo Project",
    "actions.dialog.remove.text": "要移除「{name}」嗎？它的 stage 設定與所有 task 的進度會一併刪除，repo 會回到「偵測到的 repo」，之後可以再加入。",
    "actions.dialog.remove.confirm": "移除",
    "actions.dialog.invalid.blankName": "名稱不能空白",
    "actions.dialog.invalid.blankStage": "第 {n} 個 stage 的名稱不能空白",
    "actions.dialog.invalid.duplicateStage": "stage 名稱重複：{name}",
    "actions.dialog.invalid.noStages": "至少要有一個 stage",
    "actions.dialog.stale.stages": "stages 已在別處變更，請關閉後重新開啟",
    "actions.dialog.stale.gone": "這個 Repo Project 已不存在（可能已在別處移除），請關閉對話框",
    // 按「加入」時送出的預設 stages（依介面語言；spec cockpit-dashboard「Project 切換」）。
    "actions.defaultStage.plan": "規劃",
    "actions.defaultStage.implement": "實作",
    "actions.defaultStage.review": "審查",
    "actions.defaultStage.complete": "完成",
    "actions.error.http": "操作失敗（HTTP {status}，{label}）：{reason}",
    "actions.error.network": "操作失敗（請求沒有完成，{label}）：{reason}",
    "output.empty": "還沒選 pane。點 runtime 清單裡的任一列，或按 Factory Floor 列首的「{viewOutput}」。",
    "output.stale": "過期",
    "output.deselect": "取消選取",
    "output.truncated": "更早的輸出未顯示",
    "output.gone": "pane 已不存在",
    "output.networkError": "無法連線到伺服器，正在重試",
    "output.timeout": "請求逾時（超過 {seconds} 秒沒有回應），正在重試",
    "output.http": "HTTP {status}",
    // files.js 的介面文字（ui-language task 2.2）。錯誤文案依後端錯誤本體的 code 查（files.error.*；檔案分頁專用說法
    // files.fileError.*），鍵名是 code 的 camelCase。
    "files.tree.aria": "檔案樹",
    "files.tree.refresh": "重新整理",
    "files.tree.loadingRoot": "正在讀取根目錄…",
    "files.tree.loadingDir": "讀取中…",
    "files.tree.emptyDir": "（空資料夾）",
    "files.tree.more.one": "還有 {n} 項未顯示",
    "files.tree.more.other": "還有 {n} 項未顯示",
    "files.error.forbiddenSource": "請求來源不被接受，請從本機的 Cockpit 頁面開啟",
    "files.error.methodNotAllowed": "這個操作不被接受",
    "files.error.badRequest": "名稱含有無法處理的字元，無法讀取",
    "files.error.runtimeUnknown": "設定中沒有這個 runtime",
    "files.error.paneUnknown": "這個 pane 已不在目前的畫面中",
    "files.error.noRoot": "這個 pane 沒有可瀏覽的資料夾（沒有回報工作目錄，或該目錄不存在）",
    "files.error.rootUnavailable": "這個資料夾目前無法瀏覽（不在允許的根目錄中）",
    "files.error.pathOutsideRoot": "這個資料夾指向根目錄以外，不顯示內容",
    "files.error.notFound": "資料夾已不存在",
    "files.error.wrongKind": "這個項目已不是資料夾",
    "files.error.tooLarge": "內容太大，無法顯示",
    "files.error.notMarkdown": "不是 Markdown 檔案",
    "files.error.ioError": "讀取時發生錯誤",
    "files.error.network": "無法連線到 Cockpit 服務",
    "files.error.timeout": "讀取逾時（超過 {seconds} 秒沒有回應）",
    "files.error.unknown": "讀取失敗（無法辨識的回應）",
    "files.fileError.notFound": "檔案已不存在",
    "files.fileError.wrongKind": "這個項目已不是檔案",
    "files.fileError.pathOutsideRoot": "這個檔案指向根目錄以外，不顯示內容",
    "files.fileError.rootUnavailable": "這個根目錄目前沒有任何 pane，無法讀取",
    "files.tab.title": "{path}\n根目錄：{root}",
    "files.tab.close": "關閉",
    "files.tab.closeNamed": "關閉 {name}",
    "files.tab.split": "並排",
    "files.tab.splitNamed": "並排 {name}",
    "files.tab.splitDisabled": "先選另一個檔案分頁，才能與它並排",
    "files.tab.splitCol": "並排第 {n} 欄",
    "files.toolbar.readAt": "讀取於 {time}",
    "files.toolbar.notRead": "尚未讀取",
    "files.toolbar.stale": "過期",
    "files.toolbar.vscode": "在 VS Code 開啟",
    "files.status.loading": "正在讀取…",
    "files.viewer.notImplemented": "尚未實作「{kind}」檢視器",
    // viewers.js 的介面文字（ui-language task 2.2）。
    "viewers.note.tooLarge": "檔案太大，無法預覽",
    "viewers.note.unsupported": "不支援預覽",
    "viewers.note.sizeLabel": "檔案大小：",
    "viewers.size.unknown": "未知",
    "viewers.size.bytes.one": "{n} 位元組",
    "viewers.size.bytes.other": "{n} 位元組",
    "viewers.md.inertLink": "這個連結不會開啟（指向根目錄以外，或不是 http／https 網址）",
    "viewers.md.blockedImage": "這張圖片沒有載入（只載入根目錄內以相對路徑引用的圖片）",
    "viewers.md.anchorLabel": "連到標題「{heading}」",
    "viewers.md.imageAlt": "圖片",
    "viewers.html.frameTitle": "{path} 的內容（腳本已停用）",
    "viewers.pdf.toolbarLabel": "PDF 頁面與縮放",
    "viewers.pdf.prev": "上一頁",
    "viewers.pdf.next": "下一頁",
    "viewers.pdf.zoomOut": "縮小",
    "viewers.pdf.zoomIn": "放大",
    "viewers.pdf.fit": "符合寬度",
    "viewers.pdf.pageStatusTitle": "目前頁／總頁數",
    "viewers.pdf.zoomLevelTitle": "縮放比例",
    "viewers.pdf.pageLabel": "第 {n} 頁",
    "viewers.pdf.unparsable": "PDF 無法解析",
    // git.js 的介面文字（ui-language task 2.3）。錯誤文案依後端錯誤本體的 code 查：說法與 files.js 完全相同的直接用 files.error.*，
    // 根目錄、git 專屬的說法用 git.error.*。「過期」「變更過多」「重新整理」等同模組內多處共用的字串各只一個鍵。
    "git.error.rootUnavailable": "這個根目錄目前無法瀏覽（不在允許的根目錄中）",
    "git.error.notGit": "這個根目錄不是 git repo",
    "git.error.badRequest": "請求格式不正確",
    "git.error.gitUnavailable": "找不到可用的 git",
    "git.error.gitUntrusted": "git 拒絕讀取這個 repo（擁有者不符）",
    "git.error.gitTimeout": "執行逾時",
    "git.error.gitFailed": "執行 git 時發生錯誤",
    "git.error.revUnknown": "找不到這個版本",
    "git.error.refUnknown": "找不到這個分支",
    "git.error.notFoundInRev": "檔案在這個版本不存在",
    "git.error.noMergeBase": "兩者沒有共同祖先",
    "git.error.tooLarge": "差異過大，請在 VS Code 查看",
    "git.error.unmergedPath": "這個檔案還在合併衝突中，暫存區沒有單一版本可比較",
    "git.stale": "過期",
    "git.refresh": "重新整理",
    "git.truncated": "變更過多，只列出前面一部分",
    "git.status.loadingChanges": "正在讀取變更…",
    "git.tab.close": "關閉",
    "git.tab.closeNamed": "關閉 {label}",
    "git.side.worktree": "工作區",
    "git.side.index": "已暫存",
    "git.side.empty": "（空）",
    "git.rev.index": "暫存區",
    "git.rev.tabTitle": "{path}\n版本：{rev}\n根目錄：{root}",
    "git.rev.openCurrent": "開啟目前版本",
    "git.diff.binary": "二進位檔，不顯示差異",
    "git.diff.modeOnly": "只有權限改變",
    "git.diff.submodule": "子模組，不顯示差異",
    "git.diff.identical": "兩側內容相同",
    "git.diff.gap.one": "省略 {n} 行",
    "git.diff.gap.other": "省略 {n} 行",
    "git.diff.loading": "正在讀取差異…",
    "git.diff.tabTitle": "{path}\n{sides}\n根目錄：{root}",
    "git.diff.openFile": "開啟檔案",
    "git.diff.vscode": "在 VS Code 開啟",
    "git.diff.leftVersion": "看左側版本",
    "git.diff.rightVersion": "看右側版本",
    "git.changes.listAria": "變更清單",
    "git.changes.clean": "沒有未 commit 的變更",
    "git.group.conflict": "合併衝突",
    "git.group.staged": "已暫存",
    "git.group.unstaged": "變更",
    "git.group.untracked": "未追蹤",
    "git.group.heading": "{title}（{n}）",
    "git.branch.detached": "分離 HEAD {hash}",
    "git.graph.tabTitle": "{label}\nruntime：{runtime}",
    "git.graph.filter.branches": "本地分支",
    "git.graph.filter.remotes": "遠端分支",
    "git.graph.filter.toggle": "分支篩選",
    "git.graph.filter.all": "全選",
    "git.graph.filter.none": "清除",
    "git.graph.filter.cancel": "取消",
    "git.graph.filter.apply": "套用",
    "git.graph.search.placeholder": "搜尋 commit",
    "git.graph.search.prev": "上一筆",
    "git.graph.search.next": "下一筆",
    "git.graph.search.position.one": "第 {i}／共 {n} 筆",
    "git.graph.search.position.other": "第 {i}／共 {n} 筆",
    "git.graph.search.total.one": "共 {n} 筆",
    "git.graph.search.total.other": "共 {n} 筆",
    "git.graph.search.none": "沒有符合的結果",
    "git.graph.banner.changed": "分支已變更",
    "git.graph.banner.reload": "重新載入",
    "git.graph.listAria": "Commit 清單",
    "git.graph.loadMore": "載入更多",
    "git.graph.capNote": "已達上限 {max} 筆，可用分支篩選縮小範圍",
    "git.graph.empty": "沒有可顯示的 commit",
    "git.graph.loading": "正在讀取 commit…",
    "git.copy.copied": "已複製",
    "git.copy.failed": "無法複製",
    "git.detail.loading": "正在讀取 commit 詳情…",
    "git.detail.copy": "複製",
    "git.detail.copyAria": "複製{label}",
    "git.detail.fullHash": "完整 hash",
    "git.detail.refQuote": "「{name}」",
    "git.detail.author": "作者",
    "git.detail.time": "時間",
    "git.detail.refs": "指向它的 ref",
    "git.detail.parentNotLoaded": "{hash}（不在已載入範圍）",
    "git.detail.viewVersion": "看此版本",
    "git.detail.compareBase": "選為比較基準",
    "git.detail.compareBaseNote": "已選為比較基準，點選另一個 commit 開始比較",
    "git.detail.files.one": "變更檔案（{n}）",
    "git.detail.files.other": "變更檔案（{n}）",
    "git.detail.mergeNote": "與第一個父 commit 比較",
    "git.compare.header": "比較 {base} ↔ {target}",
    "git.compare.direct": "直接比較",
    "git.compare.fork": "自分岔點起",
    // notify.js（ui-language task 2.4）。四種事件的名稱（agent blocked／agent done／task failed／task completed）是產品詞彙，
    // 兩種語言都照原文，只有說明進字典。描述 HERDR done 的鍵（`notify.kind.done.desc`、`notify.title.done`）受 design D5 守門，
    // 值不得暗示 task 已結束（繁中不得含「完成」、英文不得含 complete／finished）。
    "notify.kind.blocked.desc": "agent 卡住，等你回應",
    "notify.kind.done.desc": "agent 停下，等你來看（不代表 task 結束）",
    "notify.kind.failed.desc": "task 被標記為 failed",
    "notify.kind.completed.desc": "task 被標記為 completed",
    "notify.title.blocked": "agent 卡住",
    "notify.title.done": "agent 停下等你看",
    "notify.panel.title": "桌面通知",
    "notify.panel.note": "Cockpit 視窗在前景時不跳出通知。",
    "notify.permission.granted": "通知權限：已允許。",
    "notify.permission.denied": "通知權限：已封鎖。要收到通知，請到瀏覽器的網站設定把這個網站的通知改為允許。",
    "notify.permission.unsupported": "這個瀏覽器不支援桌面通知。",
    "notify.permission.default": "通知權限：尚未決定。",
    "notify.permission.allowButton": "允許通知",
    "notify.body.pane": "{runtime} / {pane}",
    "notify.body.paneNames": "{runtime} / {pane}（{names}）",
    "notify.body.namesSeparator": "、",
    "notify.body.task": "{project}：{task}",
    "notify.summary.title.one": "Cockpit：{n} 件事需要注意",
    "notify.summary.title.other": "Cockpit：{n} 件事需要注意",
    "notify.summary.line": "{title} · {body}",
    // 後端訊息代碼（ui-language task 3.3；design D4）：鍵是 `msg.<code>`，參數值一律原樣代入、不翻譯。
    // 繁中範本與後端原文逐字相同（加佔位符；pane_not_bound、forbidden_source、method_not_allowed 因原文有多種，是概括句）；
    // 繁中介面實際顯示後端原文（見 tMsg 註解），這份範本是原文缺漏時的後備。
    // 錯誤本體（進度寫入、綁定覆蓋、Live Output）：
    "msg.invalid_op": "不是合法的操作：{op}",
    "msg.unknown_project": "project 不存在：{id}",
    "msg.unknown_task": "task 不存在：{id}",
    "msg.unknown_workstream": "workstream 不存在：{id}",
    "msg.already_last_stage": "已是最後一個 Stage",
    "msg.already_first_stage": "已是第一個 Stage",
    "msg.already_marked": "已有標記",
    "msg.task_not_in_workstream": "task 不屬於該 workstream",
    "msg.runtime_not_registered": "runtime 未登記",
    "msg.runtime_not_connected": "runtime 未連線",
    "msg.pane_not_found": "pane 不存在",
    "msg.pane_exited": "pane 已 exited",
    "msg.persist_failed": "寫入狀態檔失敗：{detail}",
    "msg.internal_error": "寫入任務異常結束",
    // pane_not_bound 有兩種原文（agent 端點帶 task、pane 參數；寫入鎖內重驗發現綁定已變時沒有參數），範本不依賴參數、涵蓋兩者。
    "msg.pane_not_bound": "pane 未綁定到該 task 所屬的 workstream",
    "msg.missing_pane_id": "缺少 X-Herdr-Pane-Id 標頭（值為 pane 內的 HERDR_PANE_ID）",
    // forbidden_source、method_not_allowed 各有多種固定原文（依哪個檢查失敗、哪個端點），代碼只有一個，範本是共通的概括。
    "msg.forbidden_source": "請求來源不被接受，請從本機的 Cockpit 頁面開啟",
    "msg.method_not_allowed": "這個操作不被接受",
    "msg.runtime_not_found": "runtime 不存在：{runtime}",
    "msg.pane_gone": "pane 不存在：{pane}",
    "msg.output_read_failed": "讀取 pane 輸出失敗：{detail}",
    "msg.read_timeout": "讀取逾時",
    // Repo Project 管理端點、固定 pane 工作線與免帶 id 推進（repo-projects task 4.4）。repo key 是主機路徑，後端不放進參數。
    "msg.repo_not_detected": "這個 repo 不在偵測到的清單中",
    "msg.repo_already_added": "這個 repo 已經加入",
    "msg.not_repo_project": "不是 Repo Project：{id}",
    "msg.invalid_body": "請求本體不合法：不是有效的 JSON 物件，或欄位不符合格式",
    "msg.invalid_name": "名稱不合規則：去除前後空白後須為 1～64 個字元，且不含控制字元或不可見的格式字元",
    "msg.invalid_stages": "stages 不合規則：須為 1～12 個、各 1～32 個字元、不含控制字元或不可見的格式字元且互不相同；from 須是現有的 stage 且不重複引用",
    "msg.not_overridable": "這條工作線固定綁定到 pane，不能改綁",
    "msg.no_task_for_pane": "這個 pane 沒有可推進的 task",
    "msg.ambiguous_task": "這個 pane 有多張可推進的 task，無法判斷要推進哪一張",
    // 投影的連線原因、protocol 警告、project 警告：
    "msg.wsl_distro_not_running": "WSL 發行版 {distro} 未啟動",
    "msg.wsl_probe_failed": "WSL 探測失敗：{detail}",
    "msg.snapshot_failed": "snapshot 失敗：{detail}",
    "msg.seed_snapshot_failed": "seed snapshot 失敗：{detail}",
    "msg.lifecycle_subscribe_failed": "L 訂閱建立失敗：{detail}",
    "msg.status_subscribe_failed": "S 訂閱建立失敗：{detail}",
    "msg.status_resubscribe_failed": "S 重開失敗：{detail}",
    "msg.protocol_untested": "HERDR protocol {protocol} 不在已測範圍 {tested}",
    "msg.event_stream_ended": "事件流結束",
    "msg.event_connection_error": "{label} 連線錯誤：{detail}",
    "msg.event_connection_ended": "{label} 連線結束",
    "msg.task_stage_reset": "task {task} 的 stage「{stage}」已不在 pipeline 的 stages 中，已退回起始 stage「{start}」",
    "msg.drift_workspace_not_found": "workspace {id} 不存在",
    "msg.drift_tab_not_found": "tab {id} 不存在",
    "msg.drift_pane_not_found": "pane {id} 不存在",
    "msg.drift_runtime_not_registered": "runtime {id} 未登記",
    "msg.event_payload_unparsable": "{event} payload 無法解析：{detail}",
    "msg.repo_project_id_conflict": "Repo Project「{id}」與設定檔中的 project id 相同，已隱藏；只能經 API（/api/repo-projects）改名或移除",
    "msg.raw": "{text}"
  };
  var en = {
    "index.leftTabs.aria": "Left pane",
    "index.tab.files": "Files",
    "index.tab.changes": "Changes",
    "index.files.empty": "Select a pane in the Factory Floor or the runtime list first",
    "index.reviewTabs.aria": "Tabs",
    "index.events.title": "Recent events",
    "index.channel.title": "Browser → cockpit service: {state}",
    "index.channel.label": "cockpit service",
    "render.lang.text": "中文",
    "render.lang.code": "zh-Hant",
    "render.lang.aria": "切換為繁體中文",
    "render.lang.needsStorage": "Switching language needs browser storage",
    "render.lamp.stale": "Last known",
    "render.lamp.title": "cockpit → HERDR runtime {runtime}: {state}",
    "render.lamp.titleStale": "cockpit → HERDR runtime {runtime}: {state} (last known)",
    "render.notify.label": "Notification settings",
    "render.pane.focus": "Active",
    "render.pane.focusTitle": "Pane currently focused in HERDR",
    "render.pane.bindHere": "Bind here",
    "render.warning.count.one": "{n} warning",
    "render.warning.count.other": "{n} warnings",
    "render.binding.rebound": "Rebound",
    "render.binding.unbound": "Unbound",
    "render.binding.ambiguous.one": "Ambiguous ({n})",
    "render.binding.ambiguous.other": "Ambiguous ({n})",
    "render.binding.runtimeDisconnected": "Runtime disconnected",
    "render.binding.none": "No binding",
    "render.task.back": "Back",
    "render.task.advance": "Advance",
    "render.task.clearMark": "Clear mark",
    "render.row.undeclared": "Working, no task declared",
    "render.row.viewOutput": "View output",
    "render.row.rebind": "Rebind",
    "render.row.undoRebind": "Undo rebind",
    "render.row.worktree": "Worktree: {name}",
    "render.rebind.banner": "Rebind mode: pick a pane for {project} / {workstream}, then click \"{bindHere}\" on its row",
    "render.rebind.cancel": "Cancel",
    "render.error.close": "Close",
    "render.projects.empty": "No Projects",
    // Detected repos section and empty state (repo-projects task 5.1).
    "render.floor.empty": "Add a repo under \"Detected repos\" in the left column to see its Factory Floor here.",
    "render.detected.title": "Detected repos",
    "render.detected.empty": "Open a pane inside a git repo and its repo will be listed here",
    "render.detected.paneCount.one": "{n} pane",
    "render.detected.paneCount.other": "{n} panes",
    "render.detected.add": "Add",
    "render.detected.addLabel": "Add {name}",
    "render.detected.adding": "Adding…",
    "render.detected.addingLabel": "Adding {name}",
    // Repo Project management menu and dialogs (repo-projects task 5.2).
    "render.projectMenu.label": "Manage {name}",
    "render.projectMenu.rename": "Rename",
    "render.projectMenu.editStages": "Edit stages",
    "render.projectMenu.remove": "Remove",
    "actions.dialog.cancel": "Cancel",
    "actions.dialog.save": "Save",
    "actions.dialog.rename.title": "Rename: {name}",
    "actions.dialog.rename.label": "Name",
    "actions.dialog.stages.title": "Edit stages: {name}",
    "actions.dialog.stages.hint": "The order is the Factory Floor columns from left to right. Tasks in a deleted stage move to the first stage.",
    "actions.dialog.stages.nameLabel": "Stage {n} name",
    "actions.dialog.stages.up": "Up",
    "actions.dialog.stages.upLabel": "Move stage {n} up",
    "actions.dialog.stages.down": "Down",
    "actions.dialog.stages.downLabel": "Move stage {n} down",
    "actions.dialog.stages.delete": "Delete",
    "actions.dialog.stages.deleteLabel": "Delete stage {n}",
    "actions.dialog.stages.add": "Add stage",
    "actions.dialog.stages.from": "was {from}",
    "actions.dialog.stages.added": "new stage",
    "actions.dialog.remove.title": "Remove Repo Project",
    "actions.dialog.remove.text": "Remove \"{name}\"? Its stages and all task progress will be deleted. The repo goes back to \"Detected repos\" and can be added again.",
    "actions.dialog.remove.confirm": "Remove",
    "actions.dialog.invalid.blankName": "The name cannot be blank",
    "actions.dialog.invalid.blankStage": "Stage {n} name cannot be blank",
    "actions.dialog.invalid.duplicateStage": "Duplicate stage name: {name}",
    "actions.dialog.invalid.noStages": "At least one stage is required",
    "actions.dialog.stale.stages": "The stages were changed elsewhere. Close this dialog and open it again.",
    "actions.dialog.stale.gone": "This Repo Project no longer exists (it may have been removed elsewhere). Close this dialog.",
    // Default stages sent by "Add" (follow the interface language).
    "actions.defaultStage.plan": "Plan",
    "actions.defaultStage.implement": "Implement",
    "actions.defaultStage.review": "Review",
    "actions.defaultStage.complete": "Complete",
    "actions.error.http": "Action failed (HTTP {status}, {label}): {reason}",
    "actions.error.network": "Action failed (request did not complete, {label}): {reason}",
    "output.empty": "No pane selected. Click any row in the runtime list, or press \"{viewOutput}\" on a Factory Floor row.",
    "output.stale": "Stale",
    "output.deselect": "Deselect",
    "output.truncated": "Earlier output not shown",
    "output.gone": "Pane no longer exists",
    "output.networkError": "Can't reach the server, retrying",
    "output.timeout": "Request timed out (no response in {seconds} s), retrying",
    "output.http": "HTTP {status}",
    "files.tree.aria": "File tree",
    "files.tree.refresh": "Refresh",
    "files.tree.loadingRoot": "Loading root folder…",
    "files.tree.loadingDir": "Loading…",
    "files.tree.emptyDir": "(empty folder)",
    "files.tree.more.one": "{n} more item not shown",
    "files.tree.more.other": "{n} more items not shown",
    "files.error.forbiddenSource": "Request origin not accepted. Open this from the local Cockpit page",
    "files.error.methodNotAllowed": "This operation is not allowed",
    "files.error.badRequest": "The name contains characters that can't be handled, so it can't be read",
    "files.error.runtimeUnknown": "This runtime is not in the configuration",
    "files.error.paneUnknown": "This pane is no longer on screen",
    "files.error.noRoot": "This pane has no browsable folder (no working directory reported, or the directory doesn't exist)",
    "files.error.rootUnavailable": "This folder can't be browsed right now (not in an allowed root)",
    "files.error.pathOutsideRoot": "This folder points outside the root, so its contents are not shown",
    "files.error.notFound": "Folder no longer exists",
    "files.error.wrongKind": "This item is no longer a folder",
    "files.error.tooLarge": "Content too large to display",
    "files.error.notMarkdown": "Not a Markdown file",
    "files.error.ioError": "Error while reading",
    "files.error.network": "Can't reach the Cockpit service",
    "files.error.timeout": "Read timed out (no response in {seconds} s)",
    "files.error.unknown": "Read failed (unrecognized response)",
    "files.fileError.notFound": "File no longer exists",
    "files.fileError.wrongKind": "This item is no longer a file",
    "files.fileError.pathOutsideRoot": "This file points outside the root, so its content is not shown",
    "files.fileError.rootUnavailable": "This root folder has no panes right now, so it can't be read",
    "files.tab.title": "{path}\nRoot folder: {root}",
    "files.tab.close": "Close",
    "files.tab.closeNamed": "Close {name}",
    "files.tab.split": "Split view",
    "files.tab.splitNamed": "Split view: {name}",
    "files.tab.splitDisabled": "Select another file tab first to split view with it",
    "files.tab.splitCol": "Split view column {n}",
    "files.toolbar.readAt": "Read at {time}",
    "files.toolbar.notRead": "Not read yet",
    "files.toolbar.stale": "Stale",
    "files.toolbar.vscode": "Open in VS Code",
    "files.status.loading": "Loading…",
    "files.viewer.notImplemented": "The \"{kind}\" viewer isn't implemented yet",
    "viewers.note.tooLarge": "File too large to preview",
    "viewers.note.unsupported": "Preview not supported",
    "viewers.note.sizeLabel": "File size: ",
    "viewers.size.unknown": "Unknown",
    "viewers.size.bytes.one": "{n} byte",
    "viewers.size.bytes.other": "{n} bytes",
    "viewers.md.inertLink": "This link won't open (it points outside the root folder, or isn't an http/https URL)",
    "viewers.md.blockedImage": "This image wasn't loaded (only images inside the root folder referenced by relative path are loaded)",
    "viewers.md.anchorLabel": "Link to heading \"{heading}\"",
    "viewers.md.imageAlt": "Image",
    "viewers.html.frameTitle": "Contents of {path} (scripts disabled)",
    "viewers.pdf.toolbarLabel": "PDF pages and zoom",
    "viewers.pdf.prev": "Previous page",
    "viewers.pdf.next": "Next page",
    "viewers.pdf.zoomOut": "Zoom out",
    "viewers.pdf.zoomIn": "Zoom in",
    "viewers.pdf.fit": "Fit width",
    "viewers.pdf.pageStatusTitle": "Current page / total pages",
    "viewers.pdf.zoomLevelTitle": "Zoom level",
    "viewers.pdf.pageLabel": "Page {n}",
    "viewers.pdf.unparsable": "Can't parse this PDF",
    "git.error.rootUnavailable": "This root folder can't be browsed right now (not in an allowed root)",
    "git.error.notGit": "This root folder is not a git repo",
    "git.error.badRequest": "Malformed request",
    "git.error.gitUnavailable": "No usable git found",
    "git.error.gitUntrusted": "git refused to read this repo (owner mismatch)",
    "git.error.gitTimeout": "Timed out",
    "git.error.gitFailed": "Error while running git",
    "git.error.revUnknown": "Version not found",
    "git.error.refUnknown": "Branch not found",
    "git.error.notFoundInRev": "The file does not exist in this version",
    "git.error.noMergeBase": "No common ancestor",
    "git.error.tooLarge": "Diff too large, view it in VS Code",
    "git.error.unmergedPath": "This file is still in a merge conflict, so the index has no single version to compare",
    "git.stale": "Stale",
    "git.refresh": "Refresh",
    "git.truncated": "Too many changes, showing only the first part",
    "git.status.loadingChanges": "Loading changes…",
    "git.tab.close": "Close",
    "git.tab.closeNamed": "Close {label}",
    "git.side.worktree": "Working tree",
    "git.side.index": "Staged",
    "git.side.empty": "(empty)",
    "git.rev.index": "Staging area",
    "git.rev.tabTitle": "{path}\nVersion: {rev}\nRoot folder: {root}",
    "git.rev.openCurrent": "Open current version",
    "git.diff.binary": "Binary file, diff not shown",
    "git.diff.modeOnly": "Only file permissions changed",
    "git.diff.submodule": "Submodule, diff not shown",
    "git.diff.identical": "Both sides are identical",
    "git.diff.gap.one": "{n} line omitted",
    "git.diff.gap.other": "{n} lines omitted",
    "git.diff.loading": "Loading diff…",
    "git.diff.tabTitle": "{path}\n{sides}\nRoot folder: {root}",
    "git.diff.openFile": "Open file",
    "git.diff.vscode": "Open in VS Code",
    "git.diff.leftVersion": "View left version",
    "git.diff.rightVersion": "View right version",
    "git.changes.listAria": "Change list",
    "git.changes.clean": "No uncommitted changes",
    "git.group.conflict": "Merge conflicts",
    "git.group.staged": "Staged",
    "git.group.unstaged": "Changes",
    "git.group.untracked": "Untracked",
    "git.group.heading": "{title} ({n})",
    "git.branch.detached": "Detached HEAD {hash}",
    "git.graph.tabTitle": "{label}\nruntime: {runtime}",
    "git.graph.filter.branches": "Local branches",
    "git.graph.filter.remotes": "Remote branches",
    "git.graph.filter.toggle": "Branch filter",
    "git.graph.filter.all": "Select all",
    "git.graph.filter.none": "Clear",
    "git.graph.filter.cancel": "Cancel",
    "git.graph.filter.apply": "Apply",
    "git.graph.search.placeholder": "Search commits",
    "git.graph.search.prev": "Previous",
    "git.graph.search.next": "Next",
    "git.graph.search.position.one": "{i} of {n} match",
    "git.graph.search.position.other": "{i} of {n} matches",
    "git.graph.search.total.one": "{n} match",
    "git.graph.search.total.other": "{n} matches",
    "git.graph.search.none": "No matches",
    "git.graph.banner.changed": "Branches changed",
    "git.graph.banner.reload": "Reload",
    "git.graph.listAria": "Commit list",
    "git.graph.loadMore": "Load more",
    "git.graph.capNote": "Reached the limit of {max} commits. Use the branch filter to narrow the range.",
    "git.graph.empty": "No commits to show",
    "git.graph.loading": "Loading commits…",
    "git.copy.copied": "Copied",
    "git.copy.failed": "Couldn't copy",
    "git.detail.loading": "Loading commit details…",
    "git.detail.copy": "Copy",
    "git.detail.copyAria": "Copy {label}",
    "git.detail.fullHash": "full hash",
    "git.detail.refQuote": "\"{name}\"",
    "git.detail.author": "Author",
    "git.detail.time": "Time",
    "git.detail.refs": "Refs pointing to it",
    "git.detail.parentNotLoaded": "{hash} (not loaded)",
    "git.detail.viewVersion": "View this version",
    "git.detail.compareBase": "Set as compare base",
    "git.detail.compareBaseNote": "Set as compare base. Click another commit to compare.",
    "git.detail.files.one": "Changed file ({n})",
    "git.detail.files.other": "Changed files ({n})",
    "git.detail.mergeNote": "Compared with the first parent commit",
    "git.compare.header": "Compare {base} ↔ {target}",
    "git.compare.direct": "Direct comparison",
    "git.compare.fork": "From fork point",
    // notify.js (ui-language task 2.4)
    "notify.kind.blocked.desc": "The agent is blocked and waiting for your response",
    "notify.kind.done.desc": "The agent has stopped and is waiting for you to take a look (this does not mean the task has ended)",
    "notify.kind.failed.desc": "The task was marked failed",
    "notify.kind.completed.desc": "The task was marked completed",
    "notify.title.blocked": "agent blocked",
    "notify.title.done": "agent stopped, take a look",
    "notify.panel.title": "Desktop notifications",
    "notify.panel.note": "Notifications are not shown while the Cockpit window is in the foreground.",
    "notify.permission.granted": "Notification permission: allowed.",
    "notify.permission.denied": "Notification permission: blocked. To receive notifications, open your browser's site settings and set notifications to Allow for this site.",
    "notify.permission.unsupported": "This browser does not support desktop notifications.",
    "notify.permission.default": "Notification permission: not decided yet.",
    "notify.permission.allowButton": "Allow notifications",
    "notify.body.pane": "{runtime} / {pane}",
    "notify.body.paneNames": "{runtime} / {pane} ({names})",
    "notify.body.namesSeparator": ", ",
    "notify.body.task": "{project}: {task}",
    "notify.summary.title.one": "Cockpit: {n} item needs attention",
    "notify.summary.title.other": "Cockpit: {n} items need attention",
    "notify.summary.line": "{title} · {body}",
    // Backend message codes (ui-language task 3.3; design D4). Parameter values are substituted verbatim, never translated.
    "msg.invalid_op": "Not a valid operation: {op}",
    "msg.unknown_project": "Unknown project: {id}",
    "msg.unknown_task": "Unknown task: {id}",
    "msg.unknown_workstream": "Unknown workstream: {id}",
    "msg.already_last_stage": "Already at the last stage",
    "msg.already_first_stage": "Already at the first stage",
    "msg.already_marked": "Already marked",
    "msg.task_not_in_workstream": "The task does not belong to this workstream",
    "msg.runtime_not_registered": "Runtime is not registered",
    "msg.runtime_not_connected": "Runtime is not connected",
    "msg.pane_not_found": "Pane does not exist",
    "msg.pane_exited": "Pane has exited",
    "msg.persist_failed": "Failed to write the state file: {detail}",
    "msg.internal_error": "The write task ended abnormally",
    "msg.pane_not_bound": "The pane is not bound to this task's workstream",
    "msg.missing_pane_id": "Missing X-Herdr-Pane-Id header (the value is HERDR_PANE_ID inside the pane)",
    "msg.forbidden_source": "Request origin not accepted. Open this from the local Cockpit page",
    "msg.method_not_allowed": "This operation is not allowed",
    "msg.runtime_not_found": "Unknown runtime: {runtime}",
    "msg.pane_gone": "Pane does not exist: {pane}",
    "msg.output_read_failed": "Failed to read pane output: {detail}",
    "msg.read_timeout": "Read timed out",
    // Repo Project management endpoints, pinned-pane workstreams and id-less advance (repo-projects task 4.4).
    "msg.repo_not_detected": "This repo is not in the detected list",
    "msg.repo_already_added": "This repo has already been added",
    "msg.not_repo_project": "Not a Repo Project: {id}",
    "msg.invalid_body": "Invalid request body: it is not a valid JSON object, or its fields do not match the expected format",
    "msg.invalid_name": "Invalid name: after trimming it must be 1 to 64 characters with no control or invisible formatting characters",
    "msg.invalid_stages": "Invalid stages: 1 to 12 stages, each 1 to 32 characters, no control or invisible formatting characters, all distinct; every from must be an existing stage and used at most once",
    "msg.not_overridable": "This workstream is pinned to its pane and cannot be rebound",
    "msg.no_task_for_pane": "This pane has no task to advance",
    "msg.ambiguous_task": "This pane has more than one task that could be advanced, so it is unclear which one to advance",
    "msg.wsl_distro_not_running": "WSL distro {distro} is not running",
    "msg.wsl_probe_failed": "WSL probe failed: {detail}",
    "msg.snapshot_failed": "Snapshot failed: {detail}",
    "msg.seed_snapshot_failed": "Seed snapshot failed: {detail}",
    "msg.lifecycle_subscribe_failed": "Failed to open the L subscription: {detail}",
    "msg.status_subscribe_failed": "Failed to open the S subscription: {detail}",
    "msg.status_resubscribe_failed": "Failed to reopen the S subscription: {detail}",
    "msg.protocol_untested": "HERDR protocol {protocol} is outside the tested range {tested}",
    "msg.event_stream_ended": "Event stream ended",
    "msg.event_connection_error": "{label} connection error: {detail}",
    "msg.event_connection_ended": "{label} connection ended",
    "msg.task_stage_reset": "Task {task}: stage \"{stage}\" is no longer in the pipeline stages; reset to the start stage \"{start}\"",
    "msg.drift_workspace_not_found": "Workspace {id} does not exist",
    "msg.drift_tab_not_found": "Tab {id} does not exist",
    "msg.drift_pane_not_found": "Pane {id} does not exist",
    "msg.drift_runtime_not_registered": "Runtime {id} is not registered",
    "msg.event_payload_unparsable": "Could not parse the {event} event payload: {detail}",
    "msg.repo_project_id_conflict": "Repo Project \"{id}\" has the same id as a project in the config file and is hidden; it can only be renamed or removed via the API (/api/repo-projects)",
    "msg.raw": "{text}"
  };

  // localStorage 能不能寫（載入時偵測一次）：隱私模式、封鎖網站資料時 setItem 會丟例外，切換按鈕據此停用
  // （spec「語言切換按鈕」；SDD 裁決）。用獨立的探測鍵，不動 cockpit.lang，也不會觸發其他視窗的語言同步。
  function detectCanPersist() {
    try {
      var probe = STORAGE_KEY + ".probe";
      window.localStorage.setItem(probe, "1");
      window.localStorage.removeItem(probe);
      return true;
    } catch (e) {
      return false;
    }
  }

  var lang = detectLang();
  var canPersist = detectCanPersist();
  var dict = lang === "zh" ? zh : en;
  var warned = {};

  var hasOwn = Object.prototype.hasOwnProperty;

  function t(key, params) {
    if (!hasOwn.call(dict, key)) {
      if (!hasOwn.call(warned, key)) {
        warned[key] = true;
        if (typeof console !== "undefined" && console.warn) console.warn("[i18n] 缺少鍵：" + key);
      }
      return key;
    }
    var p = params || {};
    return dict[key].replace(/\{(\w+)\}/g, function (whole, name) {
      return hasOwn.call(p, name) ? String(p[name]) : whole;
    });
  }

  // 依數量選單複數鍵（`<key>.one`／`<key>.other`）：n 為 1 用 .one，其餘（含 0）用 .other；`{n}` 自動代入 n，
  // params 的其他欄位照常代入。兩份字典都要有 .one 與 .other 兩個鍵（繁中兩者同值），鍵集合才會相等。
  function tn(key, n, params) {
    var p = {};
    if (params) for (var name in params) if (hasOwn.call(params, name)) p[name] = params[name];
    p.n = n;
    return t(key + (n === 1 ? ".one" : ".other"), p);
  }

  // 後端訊息（錯誤本體、投影的 *_msg 欄位；design D4）：`msgObj` 是 `{code, params}`（或整個錯誤本體，它也帶這兩欄），
  // `fallbackText` 是同一則訊息的原文欄位（後端繁中）。規則：
  //   - 繁中介面且有原文：顯示原文。原文就是後端的繁中文字，且帶有範本放不下的細節（狀態檔路徑與 I/O 原因、
  //     內部錯誤原因等），照原樣最不會失真；繁中範本只在原文缺漏時當後備。
  //   - 英文介面：字典有 `msg.<code>` 且範本的每個佔位符都有參數 -> 範本代入參數值（原樣，不翻譯）；
  //     否則顯示原文。字典沒有的代碼、沒有 msgObj、參數不齊都靜默退回原文，不 console.warn、不顯示代碼本身。
  //   - 連原文都沒有時回傳空字串。
  function tMsg(msgObj, fallbackText) {
    var hasText = typeof fallbackText === "string" && fallbackText !== "";
    if (lang === "zh" && hasText) return fallbackText;
    var code = msgObj && typeof msgObj.code === "string" ? msgObj.code : "";
    var key = "msg." + code;
    if (code !== "" && hasOwn.call(dict, key)) {
      var p = msgObj.params && typeof msgObj.params === "object" ? msgObj.params : {};
      var missing = false;
      var out = dict[key].replace(/\{(\w+)\}/g, function (whole, name) {
        if (hasOwn.call(p, name)) return String(p[name]);
        missing = true;
        return whole;
      });
      if (!missing) return out;
    }
    return typeof fallbackText === "string" ? fallbackText : "";
  }

  function setLang(next) {
    if (next !== "zh" && next !== "en") return;
    try {
      window.localStorage.setItem(STORAGE_KEY, next);
    } catch (e) {
      /* 本機儲存不可用：選擇存不下來，重新載入也不會變，所以不動作（切換按鈕此時已停用） */
      return;
    }
    window.location.reload();
  }

  function apply(root) {
    var scope = root || document;
    var nodes = scope.querySelectorAll("[data-i18n]");
    for (var i = 0; i < nodes.length; i++) {
      var node = nodes[i];
      var params = null;
      var rawParams = node.getAttribute("data-i18n-params");
      if (rawParams) {
        try {
          params = JSON.parse(rawParams);
        } catch (e) {
          params = null;
        }
      }
      var text = t(node.getAttribute("data-i18n"), params);
      var attr = node.getAttribute("data-i18n-attr");
      if (attr) node.setAttribute(attr, text);
      else node.textContent = text;
    }
  }

  window.cockpitI18n = {
    lang: lang,
    t: t,
    tn: tn,
    tMsg: tMsg,
    setLang: setLang,
    resolveLang: resolveLang,
    apply: apply,
    canPersist: canPersist,
    dictionaries: { zh: zh, en: en }
  };

  // 其他視窗切換語言（寫入 cockpit.lang）時，這個視窗也重新載入（spec「語言切換按鈕」；design D3）。
  // 新值是 zh／en 且與目前不同才重載；被清掉（null）或寫成非法值時重新判斷，結果與目前不同才重載。
  if (typeof window.addEventListener === "function") {
    window.addEventListener("storage", function (event) {
      if (event.key !== STORAGE_KEY) return;
      var next = event.newValue === "zh" || event.newValue === "en" ? event.newValue : detectLang();
      if (next !== lang) window.location.reload();
    });
  }

  // <html lang>：zh-Hant 或 en（spec 要求）。本檔在 <head> 同步載入，這一行在 <body> 解析之前就執行，
  // 第一次繪製前 <html lang> 已是最終值。
  var root = document.documentElement;
  root.setAttribute("lang", lang === "zh" ? "zh-Hant" : "en");

  // 靜態節點的翻譯在 DOMContentLoaded 才套用（本檔在 <head>，執行時 <body> 還沒解析）。英文介面在套用前把帶
  // data-i18n 的節點先藏起來（style.css 的 `html.i18n-pending [data-i18n]`），不會先閃一幀繁中後備文字
  // （spec「任何介面文字繪製之前決定語言」）；套用後移除。繁中的後備文字本來就是繁中，不藏、體驗不變。
  // 1.5 秒計時器是保險：文件遲遲解析不完時也不讓這些節點一直看不見（同宣傳頁 site/index.html 的做法）。
  var PENDING_CLASS = "i18n-pending";
  function reveal() {
    root.classList.remove(PENDING_CLASS);
  }
  if (document.readyState === "loading") {
    if (lang === "en") {
      root.classList.add(PENDING_CLASS);
      setTimeout(reveal, 1500);
    }
    document.addEventListener("DOMContentLoaded", function () {
      apply(document);
      reveal();
    });
  } else {
    apply(document);
  }
})();
