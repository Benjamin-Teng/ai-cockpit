# desktop-launch（delta）

## MODIFIED Requirements

### Requirement: 閒置自動結束

`cockpit` 必須接受命令列旗標 `--exit-when-idle`（可與 `--config` 併用、順序不拘；重複給視為錯誤）。帶此旗標時，後端記錄目前開著的 `/ws` 連線數，並在下列
任一情況走與 Ctrl-C 相同的正常關閉流程結束、結束碼 0：

- **從未連線**：自開始監聽起 60 秒內從未有任何 `/ws` 連線（期間其他 HTTP 請求不延長這 60 秒）。
- **閒置**：曾經有過連線，目前連線數為 0，且距離「最近一次連線數降為 0」與「之後最近一次 `GET /` 請求或通過來源檢查的 `GET /api/state` 請求」兩者中較晚者已滿 10 秒。
  計時中若有新的 `/ws` 連線即取消；之後再降為 0 時重新計時。

`GET /` 與通過來源檢查的 `GET /api/state` 在閒置計時中延長期限，是為了讓「啟動器偵測到 Cockpit 仍在執行、接著開啟視窗」這段時間內後端不會剛好結束。
被來源檢查拒絕的 `/ws` 與 `GET /api/state`（`cockpit-dashboard`「狀態端點只接受本機同源請求」）不算連線、不延長期限。
其他 HTTP 請求不影響連線數與計時。不帶此旗標時，行為與過去完全相同（不因連線數結束）。

後端結束後，`agent-reporting` 的回報端點與 `pipeline-progress` 的寫入端點都無法連線，在此期間送出的請求會連線失敗、不會被保留或重送；
這是「關視窗即結束」（使用者 2026-10-02 的決定）的直接後果，啟動器模式下 agent 的進度回報只在 Cockpit 視窗開著時有效。「結束」指開始正常關閉流程；關閉流程本身所需的時間依既有規則。

#### Scenario: 最後一個畫面關閉後結束

- **GIVEN** `cockpit --exit-when-idle` 執行中，有一個 `/ws` 連線
- **WHEN** 該連線關閉，之後 10 秒內沒有新連線、也沒有 `GET /` 或通過來源檢查的 `GET /api/state`
- **THEN** 後端在約 10 秒時開始正常關閉並結束，結束碼 0

#### Scenario: 重新整理不會被誤殺

- **GIVEN** `cockpit --exit-when-idle` 執行中，有一個 `/ws` 連線
- **WHEN** 該連線關閉，3 秒後又有新的 `/ws` 連線
- **THEN** 後端持續執行

#### Scenario: 啟動器偵測延長期限

- **GIVEN** `cockpit --exit-when-idle` 執行中，最後一個連線已關閉 8 秒
- **WHEN** 收到通過來源檢查的 `GET /api/state`（啟動器以 `Host: <監聽位址>` 偵測），之後 5 秒內有新的 `/ws` 連線
- **THEN** 後端持續執行

#### Scenario: 從未有人連上

- **GIVEN** `cockpit --exit-when-idle` 剛開始監聽
- **WHEN** 60 秒內沒有任何 `/ws` 連線（期間有 `GET /api/state` 也一樣）
- **THEN** 後端正常結束

#### Scenario: 被拒的 /ws 不讓後端保持執行

- **GIVEN** `cockpit --exit-when-idle` 執行中，唯一的 `/ws` 連線剛關閉
- **WHEN** 之後只有帶 `Origin: https://evil.example` 的 `/ws` 請求，以及 `Host: evil.example:<port>` 的 `GET /api/state` 請求（皆回 403）
- **THEN** 閒置計時不受影響，約 10 秒後後端照常結束

#### Scenario: 沒有旗標時不結束

- **GIVEN** `cockpit` 不帶 `--exit-when-idle` 執行
- **WHEN** 唯一的 `/ws` 連線關閉並經過 60 秒
- **THEN** 後端持續執行
