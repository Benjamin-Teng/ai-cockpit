//! Spike 5（tasks.md 1.5；design §2.9；ADR-0002 探測步驟）：
//! 探測 `wsl.exe --list --running --quiet` 是否喚醒虛擬機（(a) 本次不重驗，見下方
//! 說明），並實測 `wsl.exe` 各種 `--list` 變體的 stdout 位元組編碼，判斷
//! change 1b 的探測實作該用哪種解碼策略。
//!
//! 手動執行：`cargo run -p herdr-client --example spike5_wsl_probe`
//!
//! 本檔全程只執行唯讀的 `wsl.exe --list ...`，不啟動、不關閉任何發行版或 HERDR
//! server（不得執行 `wsl.exe --shutdown`／`--terminate`）。
//!
//! (a) WSL 虛擬機停止時是否喚醒：本次執行時虛擬機正在跑（`Ubuntu-24.04`，供 spike 2～4
//! 使用），無法重驗停止狀態；沿用 `docs/research/2026-09-13/local-checks.txt` 於
//! 2026-09-13 的實測（前後 `Get-Process` 皆無 `vmmem`／`vmmemWSL`）。本檔聚焦 (b)
//! 虛擬機在跑時的行為與輸出編碼。

use std::process::Output;

use tokio::process::Command;

fn print_usage() {
    eprintln!("用法: spike5_wsl_probe");
    eprintln!();
    eprintln!("  不接受任何參數。探測 wsl.exe --list 系列指令的輸出編碼，全程唯讀");
    eprintln!("  （只執行 `wsl.exe --list ...`，不啟動／關閉任何發行版或 HERDR server）。");
    eprintln!();
    eprintln!("  -h, --help  印出本說明並離開，不啟動任何子程序");
}

/// 在啟動任何子程序之前完整解析 argv：本檔不該有任何參數。`-h`／`--help` 印用法後
/// 直接離開（exit 0）；其他任何參數視為錯誤（exit 2）。
fn parse_args() {
    // 只需檢查第一個額外參數：本檔不接受任何參數，兩個分支都會 exit，第二個以後的
    // 參數不影響結果（用 for 迴圈寫會被 clippy::never_loop 抓到，因為每個分支都發散）。
    let Some(arg) = std::env::args().nth(1) else {
        return;
    };
    match arg.as_str() {
        "-h" | "--help" => {
            print_usage();
            std::process::exit(0);
        }
        other => {
            eprintln!("unknown argument: {other}");
            print_usage();
            std::process::exit(2);
        }
    }
}

/// 對一次 `wsl.exe` 呼叫的 stdout 做位元組層級分析並印出：長度、前 16 bytes
/// 十六進位、是否含 NUL byte、`String::from_utf8` 結果、以及以 UTF-16LE
/// （`from_utf16_lossy`）解碼後裁掉 `\r` 與空行的清單。
fn analyze(label: &str, output: &Output) {
    let bytes = &output.stdout;
    println!("--- {label} ---");
    println!("exit code: {:?}", output.status.code());
    println!("stdout bytes 長度: {}", bytes.len());

    let head: Vec<String> = bytes.iter().take(16).map(|b| format!("{b:02x}")).collect();
    println!("前 16 bytes 十六進位: {}", head.join(" "));

    let has_nul = bytes.contains(&0u8);
    println!("是否含 NUL byte: {has_nul}");

    match std::str::from_utf8(bytes) {
        Ok(s) => println!("String::from_utf8: Ok（{} chars）", s.chars().count()),
        Err(e) => println!("String::from_utf8: Err({e})"),
    }

    if !bytes.len().is_multiple_of(2) {
        println!(
            "警告：位元組長度為奇數（{}），UTF-16LE 解碼會捨棄最後一個 byte",
            bytes.len()
        );
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let decoded = String::from_utf16_lossy(&units);
    let lines: Vec<&str> = decoded
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !line.is_empty())
        .collect();
    println!("UTF-16LE 解碼後行清單: {lines:?}");
    println!();
}

/// 執行 `wsl.exe <args>`，`with_wsl_utf8` 為 true 時額外設定 `WSL_UTF8=1`
/// （WSL 文件曾提到此變數可讓輸出改為 UTF-8；本檔以實測結果為準，不引用記憶）。
async fn run_wsl(args: &[&str], with_wsl_utf8: bool) -> Output {
    let mut cmd = Command::new("wsl.exe");
    cmd.args(args);
    if with_wsl_utf8 {
        cmd.env("WSL_UTF8", "1");
    }
    cmd.output()
        .await
        .unwrap_or_else(|e| panic!("執行 wsl.exe {args:?} 失敗: {e}"))
}

/// 依實測結果印出建議的解碼策略：不引用「WSL 慣例上是 UTF-16LE」之類的記憶值，
/// 只看這次量到的 bytes。
fn suggest_strategy(default_out: &Output, utf8_env_out: &Output) {
    println!("=== 建議的解碼策略（依上方實測結果，非引用記憶）===");

    let default_is_utf8 = std::str::from_utf8(&default_out.stdout).is_ok();
    let default_has_nul = default_out.stdout.contains(&0u8);
    let utf8_env_is_utf8 = std::str::from_utf8(&utf8_env_out.stdout).is_ok();
    let bytes_changed = default_out.stdout != utf8_env_out.stdout;

    println!("WSL_UTF8=1 是否改變了 stdout 位元組內容: {bytes_changed}");

    // 注意順序：NUL byte 本身就是合法的單位元組 UTF-8 字元（U+0000），所以
    // UTF-16LE 交錯 ASCII 字元的輸出送進 String::from_utf8 也會回傳 Ok——這是偽陽性，
    // 必須先看有沒有 NUL byte，不能只憑 from_utf8().is_ok() 判斷已經是 UTF-8。
    if default_has_nul {
        println!(
            "預設環境輸出含 NUL byte（即使 String::from_utf8 回 Ok 也是偽陽性：0x00 本身\
             就是合法的單位元組 UTF-8 字元，但字串內容是 U\\0b\\0u\\0n\\0t\\0u… 交錯，\
             不是可用文字）→ 判定為 UTF-16LE，建議一律以 from_utf16_lossy 解碼後裁 \\r \
             與空行（不要用 from_utf8().is_ok() 當判斷依據，要看 has_nul）。"
        );
    } else if default_is_utf8 {
        println!(
            "預設環境（未設 WSL_UTF8）輸出不含 NUL byte 且是合法 UTF-8 → 建議直接 String::from_utf8。"
        );
    } else {
        println!("預設環境輸出非合法 UTF-8 也不含 NUL byte → 編碼不明，需人工檢視上方十六進位。");
    }

    if bytes_changed {
        if utf8_env_is_utf8 {
            println!(
                "設定 WSL_UTF8=1 後輸出變成合法 UTF-8：若能控制子程序環境變數可改走 \
                 WSL_UTF8=1 + UTF-8 解碼；但因不確定使用者環境是否一律可設，仍建議 \
                 change 1b 的探測實作維持不依賴此變數的 UTF-16LE 解碼，較穩健。"
            );
        } else {
            println!("設定 WSL_UTF8=1 後輸出改變但仍非合法 UTF-8，需人工檢視上方十六進位。");
        }
    } else {
        println!("設定 WSL_UTF8=1 對輸出位元組無影響（本次實測未觀察到效果）。");
    }
}

#[tokio::main]
async fn main() {
    parse_args();

    println!(
        "(a) 虛擬機停止時是否喚醒：本次執行時虛擬機正在跑，未重驗；沿用 \
         docs/research/2026-09-13/local-checks.txt 2026-09-13 的實測。以下為 (b) \
         虛擬機在跑時的行為與編碼實測。\n"
    );

    let running_default = run_wsl(&["--list", "--running", "--quiet"], false).await;
    analyze("wsl --list --running --quiet（預設環境）", &running_default);

    let running_utf8 = run_wsl(&["--list", "--running", "--quiet"], true).await;
    analyze("wsl --list --running --quiet（WSL_UTF8=1）", &running_utf8);

    let all_default = run_wsl(&["--list", "--quiet"], false).await;
    analyze(
        "wsl --list --quiet（不加 --running，預設環境）",
        &all_default,
    );

    let all_utf8 = run_wsl(&["--list", "--quiet"], true).await;
    analyze(
        "wsl --list --quiet（不加 --running，WSL_UTF8=1）",
        &all_utf8,
    );

    println!(
        "exit code 對照：--running --quiet(預設)={:?}；--running --quiet(WSL_UTF8=1)={:?}；\
         --quiet 不加 --running(預設)={:?}；--quiet 不加 --running(WSL_UTF8=1)={:?}",
        running_default.status.code(),
        running_utf8.status.code(),
        all_default.status.code(),
        all_utf8.status.code(),
    );

    suggest_strategy(&running_default, &running_utf8);
}
