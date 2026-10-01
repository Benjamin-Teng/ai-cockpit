//! live-output-color task 3.1 驗收測試：HERDR `pane.read format=ansi` 字串 → 帶樣式片段
//! （spec `live-output`「輸出樣式轉換」；design D1、D4）。
//!
//! 一律經 `PaneOutput::from_segments(parse(x), false)` 斷言，檢查的是端點實際會拿到的 `text()`
//! 與合併後的 `segments()`，而不是 `parse` 的中間形狀。

use cockpit_core::{AnsiColor, OutputSegment, PaneOutput, SegmentStyle};
use cockpit_herdr::ansi::parse;

use AnsiColor::{
    Black, Blue, BrightBlack, BrightBlue, BrightCyan, BrightGreen, BrightMagenta, BrightRed,
    BrightWhite, BrightYellow, Cyan, Green, Magenta, Red, White, Yellow,
};

/// 前八色，依 `30`–`37` 的順序。
const BASE: [AnsiColor; 8] = [Black, Red, Green, Yellow, Blue, Magenta, Cyan, White];
/// `bright_` 八色，依 `90`–`97` 的順序。
const BRIGHT: [AnsiColor; 8] = [
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
];

fn convert(input: &str) -> PaneOutput {
    PaneOutput::from_segments(parse(input), false)
}

fn seg(text: &str, style: SegmentStyle) -> OutputSegment {
    OutputSegment {
        text: text.to_string(),
        style,
    }
}

fn plain() -> SegmentStyle {
    SegmentStyle::default()
}

fn fg(color: AnsiColor) -> SegmentStyle {
    SegmentStyle {
        fg: Some(color),
        ..SegmentStyle::default()
    }
}

fn bg(color: AnsiColor) -> SegmentStyle {
    SegmentStyle {
        bg: Some(color),
        ..SegmentStyle::default()
    }
}

fn bold() -> SegmentStyle {
    SegmentStyle {
        bold: true,
        ..SegmentStyle::default()
    }
}

fn underline() -> SegmentStyle {
    SegmentStyle {
        underline: true,
        ..SegmentStyle::default()
    }
}

/// 轉換 `ESC[{sgr}mX`，斷言結果恰為一段 `X`，回傳其樣式。
fn style_after(sgr: &str) -> SegmentStyle {
    let out = convert(&format!("\x1b[{sgr}mX"));
    assert_eq!(out.text(), "X", "SGR `{sgr}` 的 text");
    assert_eq!(out.segments().len(), 1, "SGR `{sgr}` 的片段數");
    out.segments()[0].style
}

/// 以真彩色前景 `38;2;R;G;B` 設色後的樣式。
fn truecolor_fg(r: u16, g: u16, b: u16) -> SegmentStyle {
    style_after(&format!("38;2;{r};{g};{b}"))
}

/// 以 256 色前景 `38;5;N` 設色後的樣式。
fn palette_fg(n: u16) -> SegmentStyle {
    style_after(&format!("38;5;{n}"))
}

// ---------------------------------------------------------------------------
// spec「輸出樣式轉換」的 scenario（逐條）
// ---------------------------------------------------------------------------

/// spec「基本色與樣式」：`ESC[1;31merror ESC[0mdone` → `text` 為 `error done`；兩段：`error `
/// 帶 `fg: red` 與 `bold`，`done` 不帶任何樣式欄位。
#[test]
fn scenario_basic_color_and_style() {
    let out = convert("\x1b[1;31merror \x1b[0mdone");
    assert_eq!(out.text(), "error done");
    let red_bold = SegmentStyle {
        fg: Some(Red),
        bold: true,
        ..SegmentStyle::default()
    };
    assert_eq!(
        out.segments(),
        &[seg("error ", red_bold), seg("done", plain())]
    );
}

/// spec「非 SGR 序列與 CR 被丟棄」：含 `ESC[2J`、`ESC[10;5H`、`ESC]0;title BEL` 與每行結尾的
/// `\r\n` → `text` 不含上述序列的任何字元、不含 `\r`，換行保留。
#[test]
fn scenario_non_sgr_sequences_and_cr_are_dropped() {
    let out = convert("\x1b[2J\x1b[10;5Hline1\r\n\x1b]0;title\x07line2\r\nline3");
    assert_eq!(out.text(), "line1\nline2\nline3");
    assert!(!out.text().contains('\r'));
    assert!(!out.text().contains("title"));
    assert_eq!(out.segments(), &[seg("line1\nline2\nline3", plain())]);
}

/// spec「256 色與真彩色歸色」：依序 `ESC[38;5;196m`、`ESC[38;5;244m`、`ESC[38;2;215;119;87m`、
/// `ESC[48:5:21m` → 前景 `red`、前景 `bright_black`、前景 `yellow`、背景 `blue`
/// （最後一段的前景沿用上一個設定）。
#[test]
fn scenario_palette_and_truecolor_quantization() {
    let out = convert("\x1b[38;5;196ma\x1b[38;5;244mb\x1b[38;2;215;119;87mc\x1b[48:5:21md");
    assert_eq!(out.text(), "abcd");
    let yellow_on_blue = SegmentStyle {
        fg: Some(Yellow),
        bg: Some(Blue),
        ..SegmentStyle::default()
    };
    assert_eq!(
        out.segments(),
        &[
            seg("a", fg(Red)),
            seg("b", fg(BrightBlack)),
            seg("c", fg(Yellow)),
            seg("d", yellow_on_blue),
        ]
    );
}

/// spec「不認得的參數不影響其他參數」：`ESC[5;32;99mok` → `ok` 只帶 `fg: green`。
#[test]
fn scenario_unknown_params_do_not_affect_others() {
    let out = convert("\x1b[5;32;99mok");
    assert_eq!(out.segments(), &[seg("ok", fg(Green))]);
}

/// spec「色彩參數吃掉的個數」：`ESC[38;9;1ma`、`ESC[0;38;5;256;4mb`、`ESC[0;38;5mc`、
/// `ESC[0;58;5;1;3md` → `a` 只帶 `bold`、`b` 只帶 `underline`、`c` 沒有任何樣式、`d` 只帶 `italic`
/// （`58;5;1` 整組被吃掉、不產生樣式）。
#[test]
fn scenario_color_param_consumption() {
    let out = convert("\x1b[38;9;1ma\x1b[0;38;5;256;4mb\x1b[0;38;5mc\x1b[0;58;5;1;3md");
    assert_eq!(out.text(), "abcd");
    let italic = SegmentStyle {
        italic: true,
        ..SegmentStyle::default()
    };
    assert_eq!(
        out.segments(),
        &[
            seg("a", bold()),
            seg("b", underline()),
            seg("c", plain()),
            seg("d", italic),
        ]
    );
}

/// spec「DEL 與 C1 控制字元被丟棄」：`a`、DEL、`b`、U+0085、`c` → `text` 為 `abc`。
#[test]
fn scenario_del_and_c1_are_dropped() {
    let out = convert("a\u{7f}b\u{85}c");
    assert_eq!(out.text(), "abc");
    assert_eq!(out.segments(), &[seg("abc", plain())]);
}

/// spec「結尾不完整的序列」：以 `abc ESC[3` 結尾 → `text` 以 `abc` 加一個空格結尾，不含殘字。
#[test]
fn scenario_trailing_incomplete_sequence() {
    let out = convert("x\nabc \x1b[3");
    assert_eq!(out.text(), "x\nabc ");
}

// ---------------------------------------------------------------------------
// SGR 樣式參數
// ---------------------------------------------------------------------------

/// `1` 粗體、`2` 變暗、`3` 斜體、`4` 底線、`7` 反白各自設定對應欄位。
#[test]
fn sgr_attribute_params_set_their_flag() {
    let base = SegmentStyle::default();
    assert_eq!(style_after("1"), SegmentStyle { bold: true, ..base });
    assert_eq!(style_after("2"), SegmentStyle { dim: true, ..base });
    assert_eq!(
        style_after("3"),
        SegmentStyle {
            italic: true,
            ..base
        }
    );
    assert_eq!(
        style_after("4"),
        SegmentStyle {
            underline: true,
            ..base
        }
    );
    assert_eq!(
        style_after("7"),
        SegmentStyle {
            reverse: true,
            ..base
        }
    );
}

/// `22` 取消粗體與變暗、`23` 取消斜體、`24` 取消底線、`27` 取消反白；不動其他欄位。
#[test]
fn sgr_cancel_params_clear_only_their_flags() {
    let all = SegmentStyle {
        fg: Some(Red),
        bg: Some(Green),
        bold: true,
        dim: true,
        italic: true,
        underline: true,
        reverse: true,
    };
    let set_all = "31;42;1;2;3;4;7";
    assert_eq!(style_after(set_all), all);
    assert_eq!(
        style_after(&format!("{set_all};22")),
        SegmentStyle {
            bold: false,
            dim: false,
            ..all
        }
    );
    assert_eq!(
        style_after(&format!("{set_all};23")),
        SegmentStyle {
            italic: false,
            ..all
        }
    );
    assert_eq!(
        style_after(&format!("{set_all};24")),
        SegmentStyle {
            underline: false,
            ..all
        }
    );
    assert_eq!(
        style_after(&format!("{set_all};27")),
        SegmentStyle {
            reverse: false,
            ..all
        }
    );
    // `22` 只帶變暗時也取消。
    assert_eq!(style_after("2;22"), plain());
}

/// `0` 與空參數都重設所有樣式：`ESC[0m`、`ESC[m`、開頭空參數 `ESC[;4m`、結尾空參數 `ESC[4;m`。
#[test]
fn sgr_zero_and_empty_param_reset() {
    let set_all = "\x1b[31;42;1;2;3;4;7m";
    let cases: [(&str, SegmentStyle); 4] = [
        ("\x1b[0m", plain()),
        ("\x1b[m", plain()),
        ("\x1b[;4m", underline()),
        ("\x1b[4;m", plain()),
    ];
    for (reset, expected) in cases {
        let out = convert(&format!("{set_all}A{reset}B"));
        assert_eq!(out.text(), "AB", "{reset:?}");
        let last = out.segments().last().expect("至少一段");
        assert_eq!(last.text, "B", "{reset:?}");
        assert_eq!(last.style, expected, "{reset:?}");
    }
}

/// `30`–`37`／`90`–`97` 前景、`40`–`47`／`100`–`107` 背景，全部 16 色。
#[test]
fn sgr_all_sixteen_foreground_and_background_codes() {
    for i in 0..8u16 {
        let idx = usize::from(i);
        assert_eq!(style_after(&(30 + i).to_string()), fg(BASE[idx]));
        assert_eq!(style_after(&(90 + i).to_string()), fg(BRIGHT[idx]));
        assert_eq!(style_after(&(40 + i).to_string()), bg(BASE[idx]));
        assert_eq!(style_after(&(100 + i).to_string()), bg(BRIGHT[idx]));
    }
}

/// `39` 前景回預設、`49` 背景回預設，各自不動另一邊與其他屬性。
#[test]
fn sgr_39_and_49_reset_only_their_color() {
    let red_on_green_bold = SegmentStyle {
        fg: Some(Red),
        bg: Some(Green),
        bold: true,
        ..SegmentStyle::default()
    };
    assert_eq!(
        style_after("1;31;42;39"),
        SegmentStyle {
            fg: None,
            ..red_on_green_bold
        }
    );
    assert_eq!(
        style_after("1;31;42;49"),
        SegmentStyle {
            bg: None,
            ..red_on_green_bold
        }
    );
}

/// 非色彩參數帶冒號子參數（`4:3` 波浪底線等）整個略過，不影響同一序列中的其他參數。
#[test]
fn sgr_colon_subparams_on_non_color_param_are_skipped() {
    assert_eq!(style_after("4:3"), plain());
    assert_eq!(style_after("4:3;1"), bold());
    assert_eq!(style_after("1;4:3"), bold());
    assert_eq!(style_after("0:1;32"), fg(Green));
}

/// 樣式跨換行延續（換行本身保留在同一段）。
#[test]
fn style_persists_across_newline() {
    let out = convert("\x1b[31ma\nb\x1b[0m");
    assert_eq!(out.segments(), &[seg("a\nb", fg(Red))]);
}

// ---------------------------------------------------------------------------
// 256 色與真彩色的寫法、吃掉的參數個數
// ---------------------------------------------------------------------------

/// `38;5;N` 與 `38:5:N`（以及 `48` 的兩種寫法）都設色；0–15 直接對應 16 色。
#[test]
fn palette_semicolon_and_colon_forms() {
    for n in 0..16u16 {
        let expected = if n < 8 {
            BASE[usize::from(n)]
        } else {
            BRIGHT[usize::from(n - 8)]
        };
        assert_eq!(style_after(&format!("38;5;{n}")), fg(expected), "38;5;{n}");
        assert_eq!(style_after(&format!("38:5:{n}")), fg(expected), "38:5:{n}");
        assert_eq!(style_after(&format!("48;5;{n}")), bg(expected), "48;5;{n}");
        assert_eq!(style_after(&format!("48:5:{n}")), bg(expected), "48:5:{n}");
    }
}

/// 真彩色 `38;2;R;G;B`、`38:2:R:G:B`、`38:2::R:G:B`（空的色彩空間 id）、`38:2:1:R:G:B`
/// （帶色彩空間 id），以及 `48` 的對應寫法。
#[test]
fn truecolor_semicolon_and_colon_forms() {
    for sgr in [
        "38;2;255;0;0",
        "38:2:255:0:0",
        "38:2::255:0:0",
        "38:2:1:255:0:0",
    ] {
        assert_eq!(style_after(sgr), fg(Red), "{sgr}");
    }
    for sgr in ["48;2;0;0;255", "48:2:0:0:255", "48:2::0:0:255"] {
        assert_eq!(style_after(sgr), bg(Blue), "{sgr}");
    }
}

/// 分號寫法吃掉的參數：`5` 吃其後 1 個、`2` 吃其後 3 個、其他子類型只吃自己；後續參數照常套用。
#[test]
fn semicolon_color_consumes_exact_param_count() {
    // `38;5;1` 之後的 `4` 不被吃掉。
    assert_eq!(
        style_after("38;5;1;4"),
        SegmentStyle {
            fg: Some(Red),
            underline: true,
            ..SegmentStyle::default()
        }
    );
    // `38;2;1;2;3` 吃掉 1、2、3（RGB(1,2,3)：最大減最小 2 < 64 → 無彩，(3+1)/2 = 2 < 48 → black），
    // 之後的 `4` 照常套用。
    assert_eq!(
        style_after("38;2;1;2;3;4"),
        SegmentStyle {
            fg: Some(Black),
            underline: true,
            ..SegmentStyle::default()
        }
    );
    // 其他子類型（`9`、`0`、空）只吃子類型本身。
    assert_eq!(style_after("38;9;1"), bold());
    assert_eq!(style_after("48;9;4"), underline());
    assert_eq!(style_after("38;0;1"), bold());
    assert_eq!(style_after("38;;1"), bold());
}

/// 參數不完整（`38;5`、`38;2;1;2`、只有 `38`）時色彩不生效，前面已套用的參數保留。
#[test]
fn incomplete_color_params_do_not_apply() {
    assert_eq!(style_after("38;5"), plain());
    assert_eq!(style_after("48;5"), plain());
    assert_eq!(style_after("38;2;1;2"), plain());
    assert_eq!(style_after("38"), plain());
    assert_eq!(style_after("1;38;2;1;2"), bold());
    // 冒號寫法不完整或多出元素同樣不生效。
    assert_eq!(style_after("38:5"), plain());
    assert_eq!(style_after("38:2:1:2"), plain());
    assert_eq!(style_after("48:2"), plain());
    // 已有前景色時，不生效的設定不改動原本的顏色。
    assert_eq!(style_after("32;38;5"), fg(Green));
}

/// 任一數值超出 0–255 時色彩不生效，該寫法吃掉的參數照吃、其後未被吃掉的參數照常套用。
#[test]
fn out_of_range_color_params_do_not_apply() {
    assert_eq!(style_after("38;5;256;4"), underline());
    assert_eq!(style_after("48;5;256;4"), underline());
    // `256` 是 R，仍吃掉 G、B，`1` 照常套用。
    assert_eq!(style_after("38;2;256;0;0;1"), bold());
    assert_eq!(style_after("38;2;0;300;0;1"), bold());
    assert_eq!(style_after("48;2;0;0;999;1"), bold());
    // 超出 u16 的數字（vte 飽和成 65535）同樣超出範圍。
    assert_eq!(style_after("38;5;70000;4"), underline());
    // 冒號寫法：超出範圍不生效。
    assert_eq!(style_after("38:5:256"), plain());
    assert_eq!(style_after("38:2:0:0:300"), plain());
    assert_eq!(style_after("38:2::0:256:0;1"), bold());
}

/// 冒號寫法自成一個參數，不吃後面以分號分隔的參數；`38:5;1` 的色彩不完整，`1` 照常當粗體。
#[test]
fn colon_color_does_not_consume_following_params() {
    assert_eq!(
        style_after("38:5:1;4"),
        SegmentStyle {
            fg: Some(Red),
            underline: true,
            ..SegmentStyle::default()
        }
    );
    assert_eq!(style_after("38:5;1"), bold());
}

// ---------------------------------------------------------------------------
// 歸色（design D4）
// ---------------------------------------------------------------------------

/// 色立方（各分量 0、95、135、175、215、255）與灰階（8＋10×(N−232)）的界線。
#[test]
fn palette_cube_and_grayscale_boundaries() {
    let cases: [(u16, AnsiColor); 15] = [
        // 16 → (0,0,0)：無彩，平均 0 → black。
        (16, Black),
        // 231 → (255,255,255)：平均 255 → white。
        (231, White),
        // 59 = 16＋36·1＋6·1＋1 → (95,95,95)：平均 95 → bright_black。
        (59, BrightBlack),
        // 102 = 16＋36·2＋6·2＋2 → (135,135,135)：平均 135 → bright_black。
        (102, BrightBlack),
        // 145 = 16＋36·3＋6·3＋3 → (175,175,175)：平均 175 ≥ 160 → white。
        (145, White),
        // 95 = 16＋36·2＋6·1＋1 → (135,95,95)：差 40 < 64 → 無彩，平均 115 → bright_black。
        (95, BrightBlack),
        // 131 = 16＋36·3＋6·1＋1 → (175,95,95)：差 80 ≥ 64 → 有彩，色相 0° → red。
        (131, Red),
        // 52 = 16＋36·1 → (95,0,0)：差 95 → 色相 0° → red。
        (52, Red),
        // 17 = 16＋1 → (0,0,95)：色相 240° → blue。
        (17, Blue),
        // 232 → 灰 8 → black；235 → 38 → black；236 → 48 → bright_black（平均 48 界線）。
        (232, Black),
        (235, Black),
        (236, BrightBlack),
        // 247 → 158 → bright_black；248 → 168 → white（平均 160 界線落在兩者之間）。
        (247, BrightBlack),
        (248, White),
        // 255 → 238 → white。
        (255, White),
    ];
    for (n, expected) in cases {
        assert_eq!(palette_fg(n), fg(expected), "38;5;{n}");
    }
}

/// 無彩門檻：最大分量減最小分量 63 為無彩、64 為有彩。
#[test]
fn chroma_threshold_63_and_64() {
    // (150,87,87)：差 63 → 無彩，(150+87)/2 = 118.5 → bright_black。
    assert_eq!(truecolor_fg(150, 87, 87), fg(BrightBlack));
    // (151,87,87)：差 64 → 有彩，色相 0° → red。
    assert_eq!(truecolor_fg(151, 87, 87), fg(Red));
    // (87,150,87)：差 63 → bright_black；(87,151,87)：差 64，色相 120° → green。
    assert_eq!(truecolor_fg(87, 150, 87), fg(BrightBlack));
    assert_eq!(truecolor_fg(87, 151, 87), fg(Green));
    // (63,0,0)：差 63 → 無彩，31.5 → black；(64,0,0)：差 64 → red。
    assert_eq!(truecolor_fg(63, 0, 0), fg(Black));
    assert_eq!(truecolor_fg(64, 0, 0), fg(Red));
}

/// 無彩的明度界線：(最大＋最小)÷2 小於 48 為 black、小於 160 為 bright_black、其餘 white。
#[test]
fn achromatic_lightness_boundaries() {
    // 灰 47 → 47 < 48 → black；灰 48 → bright_black。
    assert_eq!(truecolor_fg(47, 47, 47), fg(Black));
    assert_eq!(truecolor_fg(48, 48, 48), fg(BrightBlack));
    // 灰 159 → bright_black；灰 160 → white。
    assert_eq!(truecolor_fg(159, 159, 159), fg(BrightBlack));
    assert_eq!(truecolor_fg(160, 160, 160), fg(White));
    // 奇數和：(79,16,16) 差 63 → (79+16)/2 = 47.5 < 48 → black；
    // (80,17,17) 差 63 → 48.5 → bright_black。
    assert_eq!(truecolor_fg(79, 16, 16), fg(Black));
    assert_eq!(truecolor_fg(80, 17, 17), fg(BrightBlack));
    // (191,128,128) 差 63 → 159.5 < 160 → bright_black；(192,129,129) 差 63 → 160.5 → white。
    assert_eq!(truecolor_fg(191, 128, 128), fg(BrightBlack));
    assert_eq!(truecolor_fg(192, 129, 129), fg(White));
}

/// 每個色相界線（12°、75°、165°、200°、270°、330°）兩側；區間含下界不含上界。
///
/// 色相公式（HSV）：c = 最大 − 最小；最大為 R 時 60·((G−B)/c mod 6)；為 G 時 60·((B−R)/c＋2)；
/// 為 B 時 60·((R−G)/c＋4)。
#[test]
fn hue_boundaries() {
    let cases: [((u16, u16, u16), AnsiColor); 14] = [
        // 最大 R=100、最小 B=0、c=100：60·19/100 = 11.4° → red；60·20/100 = 12° → yellow。
        ((100, 19, 0), Red),
        ((100, 20, 0), Yellow),
        // 最大 G=100、最小 B=0、c=100：60·(−76/100＋2) = 74.4° → yellow；60·(−75/100＋2) = 75° → green。
        ((76, 100, 0), Yellow),
        ((75, 100, 0), Green),
        // 最大 G=100、最小 R=0、c=100：60·(74/100＋2) = 164.4° → green；60·(75/100＋2) = 165° → cyan。
        ((0, 100, 74), Green),
        ((0, 100, 75), Cyan),
        // 最大 B=90、最小 R=0、c=90：60·(−61/90＋4) = 199.33° → cyan；60·(−60/90＋4) = 200° → blue。
        ((0, 61, 90), Cyan),
        ((0, 60, 90), Blue),
        // 最大 B=100、最小 G=0、c=100：60·(49/100＋4) = 269.4° → blue；60·(50/100＋4) = 270° → magenta。
        ((49, 0, 100), Blue),
        ((50, 0, 100), Magenta),
        // 最大 R=100、最小 G=0、c=100：60·(−51/100＋6) = 329.4° → magenta；60·(−50/100＋6) = 330° → red。
        ((100, 0, 51), Magenta),
        ((100, 0, 50), Red),
        // 最大 R=100、最小 G=0：60·(−1/100＋6) = 359.4° → red（環繞回 0° 之前）。
        ((100, 0, 1), Red),
        // R=G=100 同為最大：60° → yellow（兩個公式結果相同）。
        ((100, 100, 0), Yellow),
    ];
    for ((r, g, b), expected) in cases {
        assert_eq!(truecolor_fg(r, g, b), fg(expected), "RGB({r},{g},{b})");
    }
}

/// 有彩的歸色只輸出非 `bright_` 名稱（design D4）；背景色也走同一套歸色。
#[test]
fn chromatic_quantization_never_outputs_bright_and_applies_to_background() {
    // xterm 的 bright_red (255,85,85)：差 170，色相 0° → red。
    assert_eq!(truecolor_fg(255, 85, 85), fg(Red));
    // 256 色 9（bright_red）屬 0–15，直接對應，不經歸色。
    assert_eq!(palette_fg(9), fg(BrightRed));
    assert_eq!(style_after("48;2;0;0;255"), bg(Blue));
    assert_eq!(style_after("48;5;244"), bg(BrightBlack));
}

// ---------------------------------------------------------------------------
// 非 SGR 序列與控制字元
// ---------------------------------------------------------------------------

/// OSC 以 BEL 或 ST（`ESC \`）結尾都整段丟棄。
#[test]
fn osc_terminated_by_bel_or_st_is_dropped() {
    assert_eq!(convert("a\x1b]0;title\x07b").text(), "ab");
    assert_eq!(convert("a\x1b]0;title\x1b\\b").text(), "ab");
    assert_eq!(
        convert("a\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\b").text(),
        "alinkb"
    );
}

/// DCS（`ESC P … ESC \`）與 APC（`ESC _ … ESC \`）整段丟棄。
#[test]
fn dcs_and_apc_are_dropped() {
    assert_eq!(convert("a\x1bPq#0;2;0;0;0\x1b\\b").text(), "ab");
    assert_eq!(convert("a\x1bP1$r0m\x1b\\b").text(), "ab");
    assert_eq!(convert("a\x1b_payload\x1b\\b").text(), "ab");
}

/// 帶私有前綴的 CSI 不是 SGR：`ESC[?25h` 丟棄、`ESC[>4;2m` 不設變暗；帶 intermediates 的 CSI 也不是。
#[test]
fn private_prefix_csi_is_not_sgr() {
    let out = convert("a\x1b[?25hb\x1b[>4;2mc\x1b[?1;31md\x1b[1 me");
    assert_eq!(out.text(), "abcde");
    assert_eq!(out.segments(), &[seg("abcde", plain())]);
}

/// 游標移動、清除等其他 CSI 與單字元 ESC 序列整段丟棄、不改樣式。
#[test]
fn other_csi_and_esc_sequences_are_dropped() {
    let out = convert("\x1b[31ma\x1b[Hb\x1b[2Kc\x1b[1;2rd\x1b7e\x1b8f\x1b(Bg\x1bch");
    assert_eq!(out.text(), "abcdefgh");
    assert_eq!(out.segments(), &[seg("abcdefgh", fg(Red))]);
}

/// 參數超過 32 個的 SGR 整段略過；恰好 32 個照常套用。
#[test]
fn sgr_with_more_than_32_params_is_skipped_whole() {
    // 31 個 `4` 加一個 `31`＝32 個參數 → 套用。
    let sgr32 = format!("{}31", "4;".repeat(31));
    assert_eq!(
        style_after(&sgr32),
        SegmentStyle {
            fg: Some(Red),
            underline: true,
            ..SegmentStyle::default()
        }
    );
    // 32 個 `4` 加一個 `31`＝33 個參數 → 整段略過（連前面的 `4` 也不套用）。
    let sgr33 = format!("{}31", "4;".repeat(32));
    assert_eq!(style_after(&sgr33), plain());
    // 被略過的序列不影響之前已生效的樣式。
    let out = convert(&format!("\x1b[1mA\x1b[{sgr33}mB"));
    assert_eq!(out.segments(), &[seg("AB", bold())]);
}

/// 32 個上限把冒號子參數也計入：分號參數不到 32 個、但加上子參數超過 32 個時整段略過。
#[test]
fn sgr_param_limit_counts_colon_subparams() {
    // `38:2::255:0:0` 佔 6 格（38、2、空 id、R、G、B）。
    // 26 個 `4`＋6 格＝32 → 套用（紅字＋底線）。
    let sgr32 = format!("{}38:2::255:0:0", "4;".repeat(26));
    assert_eq!(
        style_after(&sgr32),
        SegmentStyle {
            fg: Some(Red),
            underline: true,
            ..SegmentStyle::default()
        }
    );
    // 27 個 `4`＋6 格＝33（分號參數只有 28 個）→ 整段略過。
    let sgr33 = format!("{}38:2::255:0:0", "4;".repeat(27));
    assert_eq!(style_after(&sgr33), plain());
}

/// `58`（底線顏色）依 `38`／`48` 的規則吃參數，但不產生任何樣式。
#[test]
fn underline_color_58_consumes_params_without_style() {
    // 分號寫法：`5` 吃其後 1 個、`2` 吃其後 3 個；被吃掉的 `1`、`2`、`4` 不當成獨立參數。
    assert_eq!(style_after("58;5;1"), plain());
    assert_eq!(style_after("58;5;4;1"), bold());
    assert_eq!(style_after("58;2;1;2;4"), plain());
    assert_eq!(
        style_after("58;2;1;2;4;3"),
        SegmentStyle {
            italic: true,
            ..SegmentStyle::default()
        }
    );
    // 其他子類型只吃子類型本身。
    assert_eq!(style_after("58;9;1"), bold());
    // 不完整或超出範圍：照吃、不生效，其後參數照常。
    assert_eq!(style_after("58;5"), plain());
    assert_eq!(style_after("58;5;256;4"), underline());
    // 不改動既有的前景色、背景色與底線。
    assert_eq!(
        style_after("4;31;42;58;5;2"),
        SegmentStyle {
            fg: Some(Red),
            bg: Some(Green),
            underline: true,
            ..SegmentStyle::default()
        }
    );
}

/// `58` 的冒號寫法（`58:2::1:2:3`、`58:5:1`）整個元素略過，不產生樣式、不吃後面的參數。
#[test]
fn underline_color_58_colon_form_yields_no_style() {
    assert_eq!(style_after("58:2::1:2:3"), plain());
    assert_eq!(style_after("58:5:1"), plain());
    assert_eq!(style_after("58:2::1:2:3;1"), bold());
}

/// DEL 與 C1（U+0080–U+009F，含 U+009B 單字元 CSI）丟棄；其後的字元照常保留。
#[test]
fn del_and_c1_characters_are_dropped() {
    let out = convert("a\u{80}b\u{9b}c\u{9f}d\u{7f}e");
    assert_eq!(out.text(), "abcde");
}

/// `\r` 與其他 C0（NUL、BEL、BS、VT、FF）丟棄；`\n` 保留。
#[test]
fn cr_and_other_c0_are_dropped() {
    assert_eq!(convert("a\rb\r\n").text(), "ab\n");
    assert_eq!(convert("a\x00b\x07c\x08d\x0be\x0cf\n").text(), "abcdef\n");
}

/// `\t` 與中文保留，且依樣式分段。
#[test]
fn tab_and_cjk_are_kept() {
    let out = convert("\x1b[31m中文\x1b[0m\t字元\tok");
    assert_eq!(out.text(), "中文\t字元\tok");
    assert_eq!(
        out.segments(),
        &[seg("中文", fg(Red)), seg("\t字元\tok", plain())]
    );
}

/// 結尾停在各種序列中間（ESC、CSI、OSC、DCS）都整段丟棄，不留殘字。
#[test]
fn trailing_half_sequences_are_dropped() {
    for tail in [
        "\x1b",
        "\x1b[",
        "\x1b[38;5",
        "\x1b]0;ti",
        "\x1bP1",
        "\x1b_ap",
    ] {
        let out = convert(&format!("\x1b[31mabc{tail}"));
        assert_eq!(out.text(), "abc", "{tail:?}");
        assert_eq!(out.segments(), &[seg("abc", fg(Red))], "{tail:?}");
    }
}

/// 空輸入：`text` 為空字串、`segments` 為空陣列。
#[test]
fn empty_input_yields_no_segments() {
    let out = convert("");
    assert_eq!(out.text(), "");
    assert!(out.segments().is_empty());
    // 只有控制序列也一樣。
    let out = convert("\x1b[31m\x1b[0m\r\x1b[2J");
    assert_eq!(out.text(), "");
    assert!(out.segments().is_empty());
}

/// 所有片段依序串接等於 `text`；`parse` 自身的片段串接也等於同一段文字；無空片段、相鄰樣式不同。
#[test]
fn segments_concatenate_to_text() {
    let input = "\x1b[1;31merror\x1b[0m: \x1b[38;5;244mpath\x1b[0m\r\n\
                 \x1b]0;t\x07\t\x1b[4m中文\x1b[24m\x1b[4m續\x1b[0m end\x1b[38;2;215;119;87m!";
    let raw = parse(input);
    let raw_text: String = raw.iter().map(|s| s.text.as_str()).collect();
    let out = PaneOutput::from_segments(raw, false);
    let joined: String = out.segments().iter().map(|s| s.text.as_str()).collect();
    assert_eq!(joined, out.text());
    assert_eq!(raw_text, out.text());
    assert_eq!(out.text(), "error: path\n\t中文續 end!");
    assert!(out.segments().iter().all(|s| !s.text.is_empty()));
    assert!(
        out.segments()
            .windows(2)
            .all(|pair| pair[0].style != pair[1].style)
    );
}
