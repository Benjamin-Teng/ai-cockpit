//! HERDR `pane.read format=ansi` 輸出 → 帶樣式片段（live-output-color task 3.1；design D1、D4）。
//!
//! 以 `vte` 的狀態機走過字串的位元組，只取可見字元、`\n`、`\t` 與 SGR（`ESC[…m`）；其餘控制字元與
//! 控制序列（游標移動、OSC、DCS、帶私有前綴的 CSI 等）整段丟棄，結尾停在序列中間的位元組也丟棄。
//! 規則的權威是 spec `live-output`「輸出樣式轉換」。256 色與真彩色依 design D4 以色相歸到 16 色之一。

use cockpit_core::{AnsiColor, OutputSegment, SegmentStyle};
use vte::{Params, ParamsIter, Parser, Perform};

/// 把 HERDR 回傳的 ansi 字串轉成帶樣式的片段。
///
/// 回傳的片段已合併相鄰同樣式、不含空片段；依序串接即為去掉控制序列後的純文字。呼叫端以
/// `PaneOutput::from_segments` 包成輸出。
pub fn parse(input: &str) -> Vec<OutputSegment> {
    let mut collector = Collector::default();
    let mut parser = Parser::new();
    parser.advance(&mut collector, input.as_bytes());
    // 結尾若停在序列中間，parser 停在非 Ground 狀態，那些位元組不會交給 `print`，自然丟棄。
    collector.segments
}

/// 依目前樣式收集可見字元的 `vte::Perform` 實作。
#[derive(Default)]
struct Collector {
    segments: Vec<OutputSegment>,
    style: SegmentStyle,
}

impl Collector {
    fn push(&mut self, c: char) {
        match self.segments.last_mut() {
            Some(last) if last.style == self.style => last.text.push(c),
            _ => self.segments.push(OutputSegment {
                text: c.to_string(),
                style: self.style,
            }),
        }
    }
}

impl Perform for Collector {
    fn print(&mut self, c: char) {
        // vte 在 Ground 狀態把 DEL 交給 `print` 而不是 `execute`（design D1）。
        if c != '\x7f' {
            self.push(c);
        }
    }

    fn execute(&mut self, byte: u8) {
        // 只保留換行與 tab；`\r`、其他 C0 與 vte 交來的 C1（U+0080–U+009F）一律丟棄。
        match byte {
            b'\n' => self.push('\n'),
            b'\t' => self.push('\t'),
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        // 帶 `?`、`>` 等私有前綴的序列 vte 放進 intermediates（例如 `ESC[>4;2m`），不是 SGR；
        // `ignore` 表示參數超過 vte 的 32 個上限（或 intermediates 過多），整段略過。
        if action == 'm' && intermediates.is_empty() && !ignore {
            apply_sgr(&mut self.style, params);
        }
    }
}

/// 依序套用一個 SGR 序列的參數；不認得的參數略過，不影響其他參數。
fn apply_sgr(style: &mut SegmentStyle, params: &Params) {
    let mut iter = params.iter();
    while let Some(param) = iter.next() {
        match *param {
            // 分號寫法的單一參數（vte 把空參數給成 0）。
            [code] => apply_code(style, code, &mut iter),
            // 冒號寫法的色彩：子類型與數值都在同一個元素內，不吃後面的參數。
            [38, ref rest @ ..] => {
                if let Some(color) = colon_color(rest) {
                    style.fg = Some(color);
                }
            }
            [48, ref rest @ ..] => {
                if let Some(color) = colon_color(rest) {
                    style.bg = Some(color);
                }
            }
            // `38`／`48` 以外帶冒號子參數的參數（例如 `4:3`，以及底線顏色 `58:2::R:G:B`）整個略過。
            _ => {}
        }
    }
}

/// 套用一個不帶子參數的 SGR 參數；`38`／`48`／`58` 會從 `iter` 往後吃參數。
fn apply_code(style: &mut SegmentStyle, code: u16, iter: &mut ParamsIter<'_>) {
    match code {
        0 => *style = SegmentStyle::default(),
        1 => style.bold = true,
        2 => style.dim = true,
        3 => style.italic = true,
        4 => style.underline = true,
        7 => style.reverse = true,
        22 => {
            style.bold = false;
            style.dim = false;
        }
        23 => style.italic = false,
        24 => style.underline = false,
        27 => style.reverse = false,
        30..=37 => style.fg = Some(BASE[usize::from(code - 30)]),
        39 => style.fg = None,
        40..=47 => style.bg = Some(BASE[usize::from(code - 40)]),
        49 => style.bg = None,
        90..=97 => style.fg = Some(BRIGHT[usize::from(code - 90)]),
        100..=107 => style.bg = Some(BRIGHT[usize::from(code - 100)]),
        38 => {
            if let Some(color) = semicolon_color(iter) {
                style.fg = Some(color);
            }
        }
        48 => {
            if let Some(color) = semicolon_color(iter) {
                style.bg = Some(color);
            }
        }
        // `58`（底線顏色）依 `38`／`48` 的規則吃參數，但不產生任何樣式（spec「輸出樣式轉換」）。
        58 => {
            let _ = semicolon_color(iter);
        }
        _ => {}
    }
}

/// 分號寫法 `38;…`／`48;…` 的色彩：子類型 `5` 吃其後至多 1 個、`2` 吃其後至多 3 個、其他子類型
/// 只吃子類型本身。吃掉的參數不再獨立解讀；參數不完整或數值超出 0–255 時回 `None`（色彩不生效）。
fn semicolon_color(iter: &mut ParamsIter<'_>) -> Option<AnsiColor> {
    match iter.next()? {
        [5] => {
            let index = iter.next();
            Some(palette(byte(index?)?))
        }
        [2] => {
            // 先吃滿至多 3 個，再檢查；不得因前面的數值不合法就少吃。
            let r = iter.next();
            let g = iter.next();
            let b = iter.next();
            Some(quantize(byte(r?)?, byte(g?)?, byte(b?)?))
        }
        _ => None,
    }
}

/// 冒號寫法 `38:5:N`、`38:2:R:G:B`、`38:2:<色彩空間 id>:R:G:B` 的色彩（`rest` 為 `38`／`48` 之後的子參數；
/// vte 把空子參數 `38:2::R:G:B` 給成 0）。其他形狀或數值超出 0–255 時回 `None`。
fn colon_color(rest: &[u16]) -> Option<AnsiColor> {
    match *rest {
        [5, index] => Some(palette(u8::try_from(index).ok()?)),
        [2, r, g, b] | [2, _, r, g, b] => Some(quantize(
            u8::try_from(r).ok()?,
            u8::try_from(g).ok()?,
            u8::try_from(b).ok()?,
        )),
        _ => None,
    }
}

/// 分號寫法中被吃掉的一個參數轉成 0–255；帶冒號子參數或超出範圍時回 `None`。
fn byte(param: &[u16]) -> Option<u8> {
    match *param {
        [value] => u8::try_from(value).ok(),
        _ => None,
    }
}

/// 前八色，依 `30`–`37` 的順序。
const BASE: [AnsiColor; 8] = [
    AnsiColor::Black,
    AnsiColor::Red,
    AnsiColor::Green,
    AnsiColor::Yellow,
    AnsiColor::Blue,
    AnsiColor::Magenta,
    AnsiColor::Cyan,
    AnsiColor::White,
];

/// `bright_` 八色，依 `90`–`97` 的順序。
const BRIGHT: [AnsiColor; 8] = [
    AnsiColor::BrightBlack,
    AnsiColor::BrightRed,
    AnsiColor::BrightGreen,
    AnsiColor::BrightYellow,
    AnsiColor::BrightBlue,
    AnsiColor::BrightMagenta,
    AnsiColor::BrightCyan,
    AnsiColor::BrightWhite,
];

/// 256 色索引 → 16 色：0–15 直接對應，16–255 先換成 RGB 再歸色（spec「輸出樣式轉換」）。
fn palette(index: u8) -> AnsiColor {
    /// 6×6×6 色立方每個分量的六個等級。
    const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        0..=7 => BASE[usize::from(index)],
        8..=15 => BRIGHT[usize::from(index - 8)],
        16..=231 => {
            let i = index - 16;
            quantize(
                CUBE_LEVELS[usize::from(i / 36)],
                CUBE_LEVELS[usize::from(i / 6 % 6)],
                CUBE_LEVELS[usize::from(i % 6)],
            )
        }
        // 232–255 為灰階 8＋10×(N−232)，最大 238，不會溢位。
        232..=255 => {
            let level = 8 + 10 * (index - 232);
            quantize(level, level, level)
        }
    }
}

/// RGB → 16 色（design D4）：最大與最小分量差小於 64 為無彩，依明度歸 `black`／`bright_black`／
/// `white`；其餘依色相歸到六個非 `bright_` 色之一。
///
/// 全程用整數比較，界線（例如色相恰為 12°）不受浮點誤差影響。
fn quantize(r: u8, g: u8, b: u8) -> AnsiColor {
    let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let chroma = max - min;
    if chroma < 64 {
        // (max＋min)÷2 < 48 ⇔ max＋min < 96；< 160 ⇔ max＋min < 320。
        let sum = max + min;
        return if sum < 96 {
            AnsiColor::Black
        } else if sum < 320 {
            AnsiColor::BrightBlack
        } else {
            AnsiColor::White
        };
    }
    // HSV 色相 = 60°·sector / chroma，sector 落在 [0, 6·chroma)。
    // 最大為 R 時 (G−B) mod 6·chroma；為 G 時 (B−R)＋2·chroma；為 B 時 (R−G)＋4·chroma。
    let sector = if max == r {
        (g - b).rem_euclid(6 * chroma)
    } else if max == g {
        b - r + 2 * chroma
    } else {
        r - g + 4 * chroma
    };
    // 色相 < deg ⇔ 60·sector < deg·chroma（chroma > 0）。
    let below = |deg: i32| 60 * sector < deg * chroma;
    if below(12) {
        AnsiColor::Red
    } else if below(75) {
        AnsiColor::Yellow
    } else if below(165) {
        AnsiColor::Green
    } else if below(200) {
        AnsiColor::Cyan
    } else if below(270) {
        AnsiColor::Blue
    } else if below(330) {
        AnsiColor::Magenta
    } else {
        AnsiColor::Red
    }
}
