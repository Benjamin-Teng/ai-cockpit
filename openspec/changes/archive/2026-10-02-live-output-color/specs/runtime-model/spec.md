# runtime-model（delta）

## MODIFIED Requirements

### Requirement: AgentRuntime 抽象

系統必須提供 `AgentRuntime` 抽象：回報自己的 `RuntimeId`、取得一份 `RuntimeSnapshot`、建立一條
已合併的事件流（每個項目是 `RuntimeEvent` 或帶原因的錯誤），以及讀取指定 pane 的輸出（給定行數上限，
回傳格式標記、文字、帶樣式的片段、是否還有更早內容未回傳）；驅動器與狀態庫只透過這個抽象與 runtime 互動，看不到 runtime
內部有幾條連線；錯誤可附「固定重試間隔」提示，且必須能區分「pane 不存在」與其他失敗。輸出的型別不得含
任何 runtime 專屬的修訂號或版本欄位（HERDR 的 `revision` 實測恆為 0，見 `herdr-observer-types`）；輸出格式
目前只有純文字一種（表示文字不含控制序列）。輸出的片段依序串接必須等於文字、不含空片段、相鄰片段樣式不同，且這個不變式由輸出
型別本身保證（只能經由會維持它的建構方式產生），不依賴各 runtime 自律。讀取輸出不得改變狀態庫內容。

#### Scenario: 假 runtime 能替代真 runtime

- **GIVEN** 一個以腳本回應 snapshot 與事件、不含任何 HERDR 程式碼的假 runtime
- **WHEN** 交給驅動器（見 `runtime-driver`）
- **THEN** 狀態庫最終內容與腳本一致

#### Scenario: 假 runtime 回應輸出

- **GIVEN** 一個對 pane `p1` 回應三行文字、對其他 pane 回應「pane 不存在」的假 runtime
- **WHEN** 分別讀取 `p1` 與 `p9` 的輸出
- **THEN** 前者得到那三行、格式為純文字、單一段無樣式的片段；後者得到可與其他失敗區分的「pane 不存在」錯誤

#### Scenario: 讀取輸出不動狀態庫

- **GIVEN** 狀態庫已有某 runtime 的內容，投影 version 為 5
- **WHEN** 讀取該 runtime 任一 pane 的輸出十次
- **THEN** 投影 version 仍為 5

#### Scenario: 片段不變式由型別維持

- **GIVEN** 一串片段，含空字串片段與相鄰兩段樣式相同的片段
- **WHEN** 以它建立輸出
- **THEN** 得到的片段不含空片段、相鄰樣式皆不同，且串接等於輸出的文字
