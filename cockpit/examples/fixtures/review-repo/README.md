# review-repo

file-review task 3.4 的假 repo fixture，供 `ui_preview` 與瀏覽器驗收腳本測試檔案瀏覽（design D12）。

## 連結與圖片

- 相對連結＋錨點：[設計](docs/design.md#決策)
- 外部圖片（不應該被載入任何請求，改顯示替代文字）：![logo](https://example.com/logo.png)
- 根目錄內的圖片（經原始內容端點載入）：![圖](docs/pic.png)

## 表格

| 檔案 | 用途 |
| --- | --- |
| `README.md` | 本檔，測試連結、圖片、表格、任務清單等 GFM 元素 |
| `docs/design.md` | 決策記錄，測試錨點捲動 |
| `long.md` | 測試捲動位置與自動更新 |
| `page.html` | 測試 iframe sandbox |
| `report.pdf` | 測試中文 PDF |

## 任務清單

- [x] 完成項
- [ ] 未完成項

## 其他 GFM 元素

~~已經刪除的舊內容~~，改用 `行內程式碼` 表示目前狀態。

```rust
fn main() {
    println!("hello, review-repo");
}
```
