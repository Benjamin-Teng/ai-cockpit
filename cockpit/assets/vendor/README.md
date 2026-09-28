# 第三方前端資源（vendor）

本目錄存放以 `include_dir!` 內嵌進 `cockpit` 執行檔的第三方前端資源（design.md D9）。
兩個來源皆為固定版本、附授權檔，升版時整份重做本檔記錄的步驟。

## pdfjs-dist 6.3.289

- **授權**：Apache-2.0（見 `pdfjs/LICENSE`；`pdfjs/wasm/LICENSE_*` 為套件內含的第三方元件
  〔JBIG2、OpenJPEG、QCMS〕子授權，隨套件一併保留，不是本 repo 的授權宣告）。
- **下載網址**（npm registry 的 `dist.tarball`，以
  `https://registry.npmjs.org/pdfjs-dist/6.3.289` 查得）：
  `https://registry.npmjs.org/pdfjs-dist/-/pdfjs-dist-6.3.289.tgz`
- **原始下載檔 SHA-256**：
  `06f25e887adc6489f04c9fcb14198c77e4e5623a59a0bba5c4cea5838a4f1241`
  （`pdfjs-dist-6.3.289.tgz`；npm registry 同時給的 SHA-1
  `9e46d89489782a479f58d674ae5ddde8481aaa17` 已核對一致）

### 取出方式（可重現指令）

明確指定 repo 根與 scratch 目錄（不要依賴目前所在目錄），並在複製目錄型資源前先
`rm -rf` 目的地——`cp -r src dst` 在 `dst` 已存在時會把 `src` 整層放進 `dst` 裡
（產生 `dst/src/`），不會覆蓋成 `src` 的內容；本 repo 首次 vendor 時就踩過這個坑
（升版或重新匯入時 `cmaps/`／`standard_fonts/`／`wasm/`／`iccs/`／`icons/` 這幾個
目的地已存在，必須先刪再複製，否則會新舊版混雜且產生巢狀目錄）。

整段指令包在 `set -euo pipefail` 的 subshell 裡執行：`curl -f` 讓下載遇到 HTTP
錯誤（4xx／5xx）就失敗而不是把錯誤頁存成假的壓縮檔；下載完再核對 SHA-256，
不符就中止；解壓後檢查必要檔案／目錄是否齊全，也不齊全就中止。**任何一步失敗都
會在碰到 `rm -rf` 之前結束整個 subshell**，不會刪掉既有的、還能用的 vendor 資源：

```bash
(
set -euo pipefail

REPO=/d/projects/ai-cockpit        # repo 根，換成實際路徑
SCRATCH=/tmp/vendor-dl             # 任一空的 scratch 目錄，換成實際路徑
mkdir -p "$SCRATCH"

# 期望的原始下載檔 SHA-256（見上方「原始下載檔 SHA-256」；升版時記得同步換成新版的值）
EXPECTED_SHA256=06f25e887adc6489f04c9fcb14198c77e4e5623a59a0bba5c4cea5838a4f1241

curl -fsS -L -o "$SCRATCH/pdfjs-dist-6.3.289.tgz" \
  https://registry.npmjs.org/pdfjs-dist/-/pdfjs-dist-6.3.289.tgz

ACTUAL_SHA256=$(sha256sum "$SCRATCH/pdfjs-dist-6.3.289.tgz" | cut -d' ' -f1)
if [ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]; then
  echo "下載檔 SHA-256 不符，中止（期望 $EXPECTED_SHA256，實際 $ACTUAL_SHA256）" >&2
  exit 1
fi

rm -rf "$SCRATCH/pdfjs-extract"
mkdir -p "$SCRATCH/pdfjs-extract"
tar -xzf "$SCRATCH/pdfjs-dist-6.3.289.tgz" -C "$SCRATCH/pdfjs-extract"   # 展開為 pdfjs-extract/package/

SRC="$SCRATCH/pdfjs-extract/package"
DST="$REPO/cockpit/assets/vendor/pdfjs"

# 解壓後的來源齊全才動 rm -rf；缺任何一個就中止，不動既有的 $DST
for f in build/pdf.min.mjs build/pdf.worker.min.mjs LICENSE; do
  [ -e "$SRC/$f" ] || { echo "來源缺少 $f，中止" >&2; exit 1; }
done
for d in cmaps standard_fonts wasm iccs; do
  [ -d "$SRC/$d" ] || { echo "來源缺少目錄 $d，中止" >&2; exit 1; }
done

mkdir -p "$DST"
cp "$SRC/build/pdf.min.mjs"        "$DST/pdf.min.mjs"
cp "$SRC/build/pdf.worker.min.mjs" "$DST/pdf.worker.min.mjs"
cp "$SRC/LICENSE"                  "$DST/LICENSE"

for d in cmaps standard_fonts wasm iccs; do
  rm -rf "$DST/$d"
  cp -r "$SRC/$d" "$DST/$d"
done
)
```

`pdf.min.mjs`／`pdf.worker.min.mjs` 取自 `package/build/`，落地時拿掉 `build/` 這層目錄，
直接放在 `pdfjs/` 下；`cmaps/`、`standard_fonts/`、`wasm/`、`iccs/` 整層目錄複製、不拿掉層，
但複製前一律先 `rm -rf` 目的地，確保是「完整替換」而非「疊加／巢狀」。
**不取**：`package/web/`（viewer，本專案只用函式庫模式，design.md D7）、
`package/legacy/`、`package/types/`、`package/image_decoders/`、`pdf.mjs`／`pdf.sandbox.*`
（未壓縮版與 sandbox 沙箱腳本，執行時不需要）。

### 執行時會讀取的資源目錄查證（getDocument 選項）

查證方法：`pdfjs-dist@6.3.289` 套件內 `build/pdf.mjs`（未壓縮版）與 GitHub tag
`v6.3.289`（commit `1c8020a7d4e43668ac287a3ecf9a8dbea17e4c56`）的
`src/display/api.js` 均可見以下 4 個 `getDocument()` 選項，在 `getDocumentParams()` 內以
`getFactoryUrlProp()` 讀取後併入 worker 端 `evaluatorOptions`：

| 選項 | 用途（原文件註解） | 套件內對應目錄 | 原始碼位置（tag v6.3.289） |
|---|---|---|---|
| `cMapUrl` | 「預先建置的 Adobe CMaps 所在的 URL，要含結尾斜線」 | `cmaps/` | 文件註解 `src/display/api.js#L138-139`；賦值 `#L258` — <https://github.com/mozilla/pdf.js/blob/1c8020a7d4e43668ac287a3ecf9a8dbea17e4c56/src/display/api.js#L258> |
| `iccUrl` | 「預先建置的 ICC 描述檔所在的 URL，要含結尾斜線」 | `iccs/` | 文件註解 `#L142-143`；賦值 `#L260` — <https://github.com/mozilla/pdf.js/blob/1c8020a7d4e43668ac287a3ecf9a8dbea17e4c56/src/display/api.js#L260> |
| `standardFontDataUrl` | 「標準字型檔所在的 URL，要含結尾斜線」 | `standard_fonts/` | 文件註解 `#L149-150`；賦值 `#L261` — <https://github.com/mozilla/pdf.js/blob/1c8020a7d4e43668ac287a3ecf9a8dbea17e4c56/src/display/api.js#L261> |
| `wasmUrl` | 「wasm 檔案所在的 URL，要含結尾斜線」 | `wasm/` | 文件註解 `#L151-152`；賦值 `#L262` — <https://github.com/mozilla/pdf.js/blob/1c8020a7d4e43668ac287a3ecf9a8dbea17e4c56/src/display/api.js#L262> |

四個選項套件內都有對應目錄，因此全部取出（`cmaps/`、`standard_fonts/`、`wasm/`、`iccs/`）。
`wasm/` 內含 `jbig2.wasm`／`openjpeg.wasm`／`qcms_bg.wasm`／`quickjs-eval.wasm` 與對應的
`*_nowasm_fallback.js`／`quickjs-eval.js`（無 WASM 支援時的退回腳本），以及 6 個
`LICENSE_*` 子授權檔；`iccs/` 內只有 1 個 `.icc` 描述檔與 1 個 `LICENSE`。**本 task 只負責
蒐集這些靜態資源；在前端程式碼實際把 `cMapUrl`／`iccUrl`／`standardFontDataUrl`／
`wasmUrl`（連同 `cMapPacked: true`）指到 `/vendor/pdfjs/...` 是後續 task 的工作
（design.md D7 目前只明確提到 `cMapUrl`／`standardFontDataUrl`；`iccUrl`／`wasmUrl`
留給實作 task 決定是否設定、或吃 pdf.js 4 個選項皆缺時的預設行為）。**`.wasm` 由
`cockpit` 以 `application/wasm` MIME 提供（design.md D9），瀏覽器才會走串流編譯。

### 關鍵檔案 SHA-256（落地到 repo 後、commit 前）

| 檔案 | SHA-256 |
|---|---|
| `pdfjs/pdf.min.mjs` | `f80490490320511e5df18c580b9edd6b5db8058dceebaf6f161992e0a964b9e2` |
| `pdfjs/pdf.worker.min.mjs` | `8ab0e5e30031b4a06ecfddd5ae9562f0227f830ee7ec9ed1a968b134243d2386` |

### 整個 `pdfjs/` 目錄的檔案清單雜湊

只驗 2 個關鍵檔案沒辦法抓到「目錄型資源被巢狀複製、混進舊版檔案」這種錯誤（新舊版本
的 `.mjs` 雜湊仍會正確更新，但 `cmaps/`／`wasm/` 等目錄裡的內容可能是巢狀複製後的舊版
殘留）。以下指令對 `pdfjs/` 底下所有檔案（含子目錄）逐檔算 SHA-256、排序後再整體算一次
SHA-256，任何檔案增減、內容改變、或多出巢狀目錄都會讓這個值改變：

```bash
cd "$REPO"
find cockpit/assets/vendor/pdfjs -type f | LC_ALL=C sort | xargs sha256sum | sha256sum
```

目前值（對應本檔記錄的版本）：

```text
e984ac2dad01c95ed7234b5273ff023ef8f7a672d4bb186338338ed4c85140af
```

## Material Icon Theme 5.38.1（VSIX）

- **授權**：MIT（見 `material-icons/LICENSE.txt`）。
- **下載網址**（open-vsx.org）：
  `https://open-vsx.org/api/PKief/material-icon-theme/5.38.1/file/PKief.material-icon-theme-5.38.1.vsix`
- **原始下載檔 SHA-256**：
  `fa7515831a2d68b1f78bd02de40f96260bfe74efb03a238c2bde70265e04b696`
  （`PKief.material-icon-theme-5.38.1.vsix`）

### 取出方式（可重現指令）

同樣明確指定 `$REPO`／`$SCRATCH`，複製 `icons/` 前先 `rm -rf` 目的地（理由同上：
`cp -r` 對已存在的目的地會巢狀複製、留下舊版檔案）。同樣包在 `set -euo pipefail`
的 subshell 裡、`curl -f`、下載後先驗 SHA-256、解壓後先確認必要檔案／目錄齊全，
任何一步失敗都會在碰到 `rm -rf` 之前中止，不動既有的 vendor 資源：

```bash
(
set -euo pipefail

REPO=/d/projects/ai-cockpit        # repo 根，換成實際路徑
SCRATCH=/tmp/vendor-dl             # 任一空的 scratch 目錄，換成實際路徑
mkdir -p "$SCRATCH"

# 期望的原始下載檔 SHA-256（見上方「原始下載檔 SHA-256」；升版時記得同步換成新版的值）
EXPECTED_SHA256=fa7515831a2d68b1f78bd02de40f96260bfe74efb03a238c2bde70265e04b696

curl -fsS -L -o "$SCRATCH/PKief.material-icon-theme-5.38.1.vsix" \
  https://open-vsx.org/api/PKief/material-icon-theme/5.38.1/file/PKief.material-icon-theme-5.38.1.vsix

ACTUAL_SHA256=$(sha256sum "$SCRATCH/PKief.material-icon-theme-5.38.1.vsix" | cut -d' ' -f1)
if [ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]; then
  echo "下載檔 SHA-256 不符，中止（期望 $EXPECTED_SHA256，實際 $ACTUAL_SHA256）" >&2
  exit 1
fi

rm -rf "$SCRATCH/vsix-extract"
mkdir -p "$SCRATCH/vsix-extract"
unzip -q "$SCRATCH/PKief.material-icon-theme-5.38.1.vsix" -d "$SCRATCH/vsix-extract"   # VSIX 本體是 zip

SRC="$SCRATCH/vsix-extract/extension"
DST="$REPO/cockpit/assets/vendor/material-icons"

# 解壓後的來源齊全才動 rm -rf；缺任何一個就中止，不動既有的 $DST
[ -d "$SRC/icons" ]                        || { echo "來源缺少 icons/，中止" >&2; exit 1; }
[ -e "$SRC/dist/material-icons.json" ]     || { echo "來源缺少 material-icons.json，中止" >&2; exit 1; }
[ -e "$SRC/LICENSE.txt" ]                  || { echo "來源缺少 LICENSE.txt，中止" >&2; exit 1; }

mkdir -p "$DST"
rm -rf "$DST/icons"
cp -r "$SRC/icons"                "$DST/icons"
cp    "$SRC/dist/material-icons.json" "$DST/material-icons.json"
cp    "$SRC/LICENSE.txt"          "$DST/LICENSE.txt"
)
```

只取 `extension/icons/*.svg`（1251 個）、`extension/dist/material-icons.json`、
`extension/LICENSE.txt`；不取 `changelog.md`、`readme.md`、`logo.png`、`package*.json`、
國際化字串檔（`package.nls.*.json`）等 viewer/編輯器相關內容。

### `iconPath` 的形狀與 SVG 目錄的對應方式

VSIX 內 `material-icons.json` 位在 `extension/dist/`，`icons/` 位在 `extension/`（`dist/` 的
上一層）；JSON 內 `iconDefinitions` 每個條目的 `iconPath` 都是相對於 JSON 檔自身位置的路徑，
形如：

```json
"git": { "iconPath": "./../icons/git.svg" }
```

即「往上一層再進 `icons/`」。落地到 repo 時把 `material-icons.json` 放在
`material-icons/` 下、SVG 放在 `material-icons/icons/` 下（而非兩者同層），
在檔案系統結構上保留與 VSIX 原始佈局相同的相對關係（`material-icons.json` 的
上一層即是 `icons/` 的父層）。design.md D9 訂的解析方式是
`IconTheme::from_json` 讀 `iconDefinitions` 的鍵、再用 `iconPath` 取出 **SVG 檔名**
（例如取 `git.svg` 這個 basename 後另外組出 `/vendor/material-icons/icons/git.svg`
這樣的服務網址），因此後端不會照字面把 `iconPath` 當相對路徑去解析檔案系統；
目錄佈局本身只是保留與上游一致、方便人工比對。

### 關鍵檔案 SHA-256（落地到 repo 後、commit 前）

| 檔案 | SHA-256 |
|---|---|
| `material-icons/material-icons.json` | `cf381fb253fe77abbe49367478ab659842abf74ee4eeb6af7a42a93ca83dfe33` |

SVG 數量驗收：`find cockpit/assets/vendor/material-icons -name '*.svg' | wc -l` 應為 `1251`。

### 整個 `material-icons/` 目錄的檔案清單雜湊

同 pdfjs：只驗 `material-icons.json` 一個檔案沒辦法抓到 `icons/` 被巢狀複製、混進舊版
SVG 的情況。指令與算法同上，換成 `material-icons/`：

```bash
cd "$REPO"
find cockpit/assets/vendor/material-icons -type f | LC_ALL=C sort | xargs sha256sum | sha256sum
```

目前值（對應本檔記錄的版本）：

```text
3656065dbd636a87c052e4b42d7f59f96a8176dc73dea090ef7074c278f2d494
```

## `.gitattributes`

`cockpit/assets/vendor/.gitattributes` 內容為 `* -text`：repo 根 `core.autocrlf=true`，
若不強制關閉本目錄的換行轉換，`.mjs`／`.svg`／`.json` 這類會被 git 判定為文字檔的內容在
checkout 時可能被轉成 CRLF，使工作目錄位元組與本檔記錄的 SHA-256、以及
`git show HEAD:<path> | sha256sum` 的結果不一致。`.bcmap`／`.pfb`／`.ttf`／`.wasm`／`.icc`
本來就會被 git 自動判定為二進位、不受影響，但整個 vendor 目錄一律套用 `-text`
以避免遺漏。

## 升版步驟

1. 確認新版本號（pdfjs-dist 看 npm、Material Icon Theme 看 open-vsx 的 release 頁）。
2. 把「取出方式」程式碼區塊裡的版本號（URL、檔名）換成新版本；`EXPECTED_SHA256`
   這一行也要換成新版的值，不能沿用舊版寫死的值（沿用舊值會被腳本自己的雜湊檢查
   判定「下載內容跟預期不符」而中止——這正是它的設計目的，避免把中斷或錯誤的下載
   當正常內容用）。新版雜湊可以先單獨跑 `curl -fsS -L <新版網址> -o <暫存檔> &&
   sha256sum <暫存檔>` 取得，記下來、更新腳本與本檔「原始下載檔 SHA-256」段落後，
   再整段執行更新後的腳本。
3. 若是 pdfjs-dist 升版：重新查證 `src/display/api.js`（或 `build/pdf.mjs`）裡
   `getDocument()` 會讀取的資源目錄選項是否有增減（曾出現過 `cMapUrl`／`iccUrl`／
   `standardFontDataUrl`／`wasmUrl` 4 個，未來版本可能改變），依查證結果調整要取出的
   目錄清單，不要照抄本次的 4 個名稱。
4. 用上面「取出方式」的指令覆蓋落地的檔案／目錄（指令本身已對目錄型資源先
   `rm -rf` 再 `cp -r`，不會巢狀複製或殘留舊版檔案），重新計算「關鍵檔案 SHA-256」、
   「整個目錄的檔案清單雜湊」與 SVG 數量，更新本檔對應段落。
5. 跑 `cargo build -p cockpit --example ui_preview`（embed 的是建置當下的磁碟內容）。
6. 跑「既有六支腳本＋`visual-check.js`」與本 task 的驗收指令，確認沒有回歸。
