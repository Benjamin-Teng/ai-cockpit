# runtime-driver Specification

## Purpose

定義驅動器如何透過 `AgentRuntime` 抽象維持單一 runtime 的連線生命週期，讓狀態庫與 runtime 保持
一致（含 Drift 重拿、連線後沉降重拿、退避重試），與 runtime 種類無關。證據：設計文件 §4.2、§7.1、`docs/cockpit-spec.md` §24「Event loss」。

## Requirements

### Requirement: 訂閱先開、snapshot 整份替換、之前的事件丟棄

系統必須依序：把 runtime 設為 `Connecting` → 向 runtime 建立事件流 → 取得 authoritative snapshot
並整份替換狀態庫 → 設為 `Connected` → 開始逐筆套用事件；在 authoritative snapshot 完成前由事件流
送來的事件一律丟棄、不套用、不記入最近事件。

#### Scenario: 先訂閱後 snapshot

- **GIVEN** 假 runtime 記錄每次呼叫的順序
- **WHEN** 驅動器啟動並進入 `Connected`
- **THEN** 呼叫順序為「建立事件流、取得 snapshot」，且狀態庫等於該 snapshot

#### Scenario: snapshot 前的事件不套用

- **GIVEN** 假 runtime 在回應 snapshot 前先由事件流送出 3 筆指向不存在 pane 的事件
- **WHEN** snapshot 完成
- **THEN** snapshot 只被取得一次、狀態庫等於 snapshot 內容、最近事件為空

### Requirement: Drift 立即重拿

系統必須在套用結果為 Drift 時立即重新取得 snapshot 並整份替換；重拿進行中再遇到 Drift 不重複
觸發；重拿進行中到達的事件照常套用；若重拿進行中至少有一筆事件被成功套用，替換完成後必須再重拿一次
（這份 snapshot 可能比已套用的事件舊），同一次 Drift 觸發的追加重拿最多 2 次，之後即使仍有事件在
重拿期間被套用也不再追加（交給後續 Drift 或定期重拿收斂）；追加重拿期間遇到新的 Drift 不另外觸發；
重拿失敗視同斷線。

#### Scenario: Drift 觸發一次重拿

- **GIVEN** 驅動器已 `Connected`
- **WHEN** 事件流送出一筆會造成 Drift 的事件，且重拿期間沒有其他事件
- **THEN** 假 runtime 觀察到 snapshot 又被取得恰好一次，狀態庫等於新 snapshot

#### Scenario: 進行中的重拿合併

- **GIVEN** 假 runtime 讓 snapshot 回應延遲
- **WHEN** 延遲期間連續送出 3 筆造成 Drift 的事件
- **THEN** 只多取得一次 snapshot

#### Scenario: 重拿期間的事件照常套用

- **GIVEN** 假 runtime 讓 Drift 觸發的 snapshot 回應延遲
- **WHEN** 延遲期間送出一筆合法的 `AgentStatusChanged`
- **THEN** 替換前狀態庫已反映該事件；第一份替換完成後 snapshot 再被取得一次，狀態庫等於第二份 snapshot

#### Scenario: 追加重拿有上限

- **GIVEN** 假 runtime 讓每次 snapshot 回應延遲，且每次延遲期間都送出一筆合法事件
- **WHEN** 發生一次 Drift
- **THEN** 因該 Drift 取得的 snapshot 共 3 次（1 次重拿＋2 次追加），之後不再追加

### Requirement: 定期重拿

系統必須在 `Connected` 期間每隔設定的 resnapshot 間隔重拿一次 snapshot 並整份替換；任何一次成功的
snapshot（定期或 Drift 觸發）都重設計時器；斷線期間不定期重拿。

#### Scenario: 到期重拿

- **GIVEN** resnapshot 間隔 30 秒、驅動器 `Connected`、時間可控
- **WHEN** 經過 30 秒
- **THEN** snapshot 取得次數加一

#### Scenario: Drift 重拿重設計時器

- **GIVEN** 間隔 30 秒、在第 20 秒因 Drift 重拿一次
- **WHEN** 走到第 30 秒
- **THEN** 沒有再取得 snapshot；到第 50 秒才再取得一次

#### Scenario: 斷線期間不重拿

- **GIVEN** 驅動器為 `Disconnected`、退避等待 30 秒
- **WHEN** 經過 30 秒
- **THEN** 這段時間沒有取得 snapshot，只有重連時的那一次

### Requirement: 斷線、退避與固定間隔重試

系統必須在事件流結束或回傳錯誤、建立事件流失敗、取得 snapshot 失敗時：釋放事件流、把 runtime 設為
`Disconnected`（原因為錯誤的描述字串、附下次重試秒數）、等待後回到 `Connecting` 重來；等待時間依
退避序列 1、2、4、8、16、30、30…秒，成功進入 `Connected` 後歸零；若錯誤附「固定重試間隔」提示
（探測類錯誤），等待該間隔且不推進退避序列，提示的間隔小於 1 秒時以 1 秒計。

#### Scenario: 退避序列

- **GIVEN** 假 runtime 連續讓建立事件流失敗 6 次、時間可控
- **WHEN** 觀察每次 `Disconnected` 的重試秒數
- **THEN** 依序為 1、2、4、8、16、30

#### Scenario: 成功後歸零

- **GIVEN** 已失敗 3 次後成功 `Connected`，之後事件流結束
- **WHEN** 下一次 `Disconnected`
- **THEN** 重試秒數為 1

#### Scenario: 固定間隔不進退避

- **GIVEN** 假 runtime 連續回「附固定間隔 60 秒」的探測類錯誤 3 次，接著回一般錯誤
- **WHEN** 觀察四次 `Disconnected`
- **THEN** 前三次重試秒數都是 60，第四次是 1

#### Scenario: 事件流結束的原因

- **GIVEN** 驅動器 `Connected`
- **WHEN** 事件流回傳原因含 `L closed` 的錯誤項
- **THEN** `Disconnected` 的原因含 `L closed`

#### Scenario: 固定間隔下限

- **GIVEN** 假 runtime 回「附固定間隔 0 秒」的探測類錯誤、時間可控
- **WHEN** 觀察 `Disconnected` 與下一次 `Connecting`
- **THEN** 重試秒數為 1，且至少經過 1 秒才再次嘗試

#### Scenario: 取得 snapshot 時的固定間隔錯誤

- **GIVEN** 建立事件流成功，但取得 snapshot 回「附固定間隔 60 秒」的探測類錯誤
- **WHEN** 觀察 `Disconnected`
- **THEN** 重試秒數為 60，退避序列不推進

### Requirement: 可停止

系統必須在收到停止指令時結束驅動器並釋放事件流；釋放事件流即讓 runtime 關閉其內部連線。

#### Scenario: 停止後不再呼叫 runtime

- **GIVEN** 驅動器 `Connected`
- **WHEN** 發出停止
- **THEN** 假 runtime 觀察到事件流被釋放，之後不再有任何呼叫

### Requirement: 連線後沉降重拿

系統必須在每次進入 `Connected`（authoritative snapshot 整份替換完成）後，於事件流連續 1 秒沒有任何事件
到達時再取得一次 snapshot 並整份替換；若進入 `Connected` 後 5 秒內事件流始終沒有靜默滿 1 秒，必須在第
5 秒取得。每次連線只做一次沉降重拿；沉降重拿要開始時若已有重拿進行中（例如 Drift 觸發），併入該次、不
另外取得；沉降重拿失敗視同斷線。原因：部分 HERDR 版本在新訂閱時會重播整段事件歷史（舊值），且可能晚於
snapshot 到達，重播事件指向仍存在的 id、不會造成 Drift。

#### Scenario: 靜默後重拿一次

- **GIVEN** 假 runtime、時間可控，驅動器剛進入 `Connected`，之後事件流沒有事件
- **WHEN** 時間經過 1 秒後再經過 30 秒以內（不到定期重拿）
- **THEN** 進入 `Connected` 後 snapshot 恰好又被取得一次，且發生在第 1 秒之後

#### Scenario: 晚到的舊值被覆蓋

- **GIVEN** 假 runtime 在 snapshot 替換完成後 0.3 秒送出一筆 pane 的舊值（label 為空、agent 狀態 `unknown`），之後事件流靜默
- **WHEN** 時間經過 1.3 秒
- **THEN** 狀態庫中該 pane 的 label 與 agent 狀態等於第二份 snapshot

#### Scenario: 事件不停時的上限

- **GIVEN** 驅動器剛進入 `Connected`，假 runtime 每 0.5 秒送出一筆合法事件
- **WHEN** 時間經過 5 秒
- **THEN** snapshot 在第 5 秒被取得一次，之後不再因沉降而取得

#### Scenario: 重連後再做一次

- **GIVEN** 已完成沉降重拿的連線結束，驅動器重試後再次進入 `Connected`
- **WHEN** 事件流靜默 1 秒
- **THEN** snapshot 再被取得一次
