//! git 執行目標與固定前綴（git-review design D2）。

use std::fmt;

/// git 要在哪個環境執行：Windows 本機的 git，或某個 WSL distro 內的 git。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitTarget {
    /// Windows 本機 git：`git -C <path> ...`。
    Native {
        /// 根目錄的主機路徑，原樣傳給 git 的 `-C`。
        path: String,
    },
    /// WSL 內的 git：`wsl.exe -d <distro> --exec env LC_ALL=C git -C <posix> ...`
    /// （design D2：一律 `--exec`，不經 shell；不用 `--cd`）。
    Wsl {
        /// `[A-Za-z0-9._-]` 組成的 distro 名稱。
        distro: String,
        /// repo 在該 distro 內的 POSIX 路徑（以 `/` 開頭）。
        posix: String,
    },
}

/// [`select_target`] 失敗的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectTargetError {
    /// 路徑以 `\\wsl.localhost\` 或 `\\wsl$\` 開頭，但 distro 名稱或某個 POSIX 路徑片段
    /// 不合法——fail-closed：不會被靜默當成 Native 路徑執行（那樣會讓 Windows git 對一個
    /// 語意不明的 UNC 路徑動作，行為難以預期）。
    InvalidWslPath,
}

impl fmt::Display for SelectTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WSL UNC 路徑的 distro 或路徑片段不合法")
    }
}

impl std::error::Error for SelectTargetError {}

const WSL_UNC_PREFIXES: [&str; 2] = [r"\\wsl.localhost\", r"\\wsl$\"];

/// 依主機路徑（design D2：**看路徑，不看 runtime**）選擇執行目標。
pub fn select_target(host_path: &str) -> Result<GitTarget, SelectTargetError> {
    match strip_wsl_prefix(host_path) {
        Some(rest) => parse_wsl_rest(rest),
        None => Ok(GitTarget::Native {
            path: host_path.to_string(),
        }),
    }
}

fn strip_wsl_prefix(path: &str) -> Option<&str> {
    WSL_UNC_PREFIXES.into_iter().find_map(|prefix| {
        let head = path.get(..prefix.len())?;
        head.eq_ignore_ascii_case(prefix)
            .then(|| &path[prefix.len()..])
    })
}

fn parse_wsl_rest(rest: &str) -> Result<GitTarget, SelectTargetError> {
    let (distro, tail) = rest.split_once('\\').unwrap_or((rest, ""));
    if !is_valid_distro(distro) {
        return Err(SelectTargetError::InvalidWslPath);
    }
    let mut posix = String::from("/");
    let mut first = true;
    for segment in tail.split('\\') {
        if segment.is_empty() {
            continue;
        }
        if !is_plain_wsl_segment(segment) {
            return Err(SelectTargetError::InvalidWslPath);
        }
        if !first {
            posix.push('/');
        }
        posix.push_str(segment);
        first = false;
    }
    Ok(GitTarget::Wsl {
        distro: distro.to_string(),
        posix,
    })
}

fn is_valid_distro(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// 與 `cockpit/src/files.rs` 的 `is_plain_segment` 相同規則（[`select_target`] 是
/// [`crate::target`]「反向」的動作：那裡是 POSIX → UNC，這裡是 UNC → POSIX；`cockpit-git`
/// 不依賴 `cockpit`，重複同一套規則是刻意的縱深防禦）。
fn is_plain_wsl_segment(segment: &str) -> bool {
    const FORBIDDEN: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && !segment.ends_with('.')
        && !segment.ends_with(' ')
        && !segment
            .chars()
            .any(|c| FORBIDDEN.contains(&c) || u32::from(c) < 0x20)
}

// ---------------------------------------------------------------------------
// argv 組裝（design D2 固定前綴）
// ---------------------------------------------------------------------------

/// 每個查詢都帶的固定前綴（design D2）。`diff.suppressBlankEmpty=false`：repo 若設為 true，git 會把
/// 空白 context 行輸出成 `""` 而非 `" "`，解析器會當殘渣跳過而漏列、後續行號錯位；命令列 `-c`
/// 優先於 repo 設定，強制還原成 `" "`。
pub(crate) const FIXED_PREFIX: &[&str] = &[
    "--no-pager",
    "--no-optional-locks",
    "--literal-pathspecs",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.quotepath=false",
    "-c",
    "color.ui=false",
    "-c",
    "log.showSignature=false",
    "-c",
    "gc.auto=0",
    "-c",
    "diff.suppressBlankEmpty=false",
];

/// diff 類查詢（`ChangedFiles`、`FileDiff`）在子命令名稱**之後**另加的旗標（design D2）。
///
/// 這三個旗標是 `diff`／`diff-tree` 子命令自己的選項，不是像 `--no-pager` 那樣的全域
/// 選項——task 1.2 實測（`git-review-probe.md` ①）證明它們必須接在子命令名稱之後
/// （`git ... diff --no-ext-diff --no-textconv --no-color --cached`），放在子命令名稱
/// 之前會被 git 拒絕，所以不能併入 [`GitTarget::base_argv`] 的全域固定前綴，改由呼叫端
/// （`query.rs`）在推入子命令名稱之後自行加上。
pub(crate) const DIFF_EXTRA: &[&str] = &["--no-ext-diff", "--no-textconv", "--no-color"];

impl GitTarget {
    /// `git`（或 `wsl.exe ... git`）加上 `-C <path>` 的 argv 前段，不含固定前綴。
    fn program_and_dir(&self) -> Vec<String> {
        match self {
            GitTarget::Native { path } => {
                vec!["git".to_string(), "-C".to_string(), path.clone()]
            }
            GitTarget::Wsl { distro, posix } => vec![
                "wsl.exe".to_string(),
                "-d".to_string(),
                distro.clone(),
                "--exec".to_string(),
                "env".to_string(),
                "LC_ALL=C".to_string(),
                "git".to_string(),
                "-C".to_string(),
                posix.clone(),
            ],
        }
    }

    /// 組出「程式名＋`-C`＋固定前綴」，子命令名稱與其餘引數（含 diff 類的
    /// [`DIFF_EXTRA`]）由呼叫端接續。
    pub(crate) fn base_argv(&self) -> Vec<String> {
        let mut argv = self.program_and_dir();
        argv.extend(FIXED_PREFIX.iter().map(|s| s.to_string()));
        argv
    }
}

#[cfg(test)]
mod tests {
    use super::{GitTarget, SelectTargetError, select_target};

    #[test]
    fn native_for_plain_windows_path() {
        let target = select_target(r"C:\repo").expect("應為 Native");
        assert_eq!(
            target,
            GitTarget::Native {
                path: r"C:\repo".to_string()
            }
        );
    }

    #[test]
    fn wsl_localhost_unc_selects_wsl_target() {
        let target = select_target(r"\\wsl.localhost\Ubuntu-24.04\home\me\repo").expect("應為 Wsl");
        assert_eq!(
            target,
            GitTarget::Wsl {
                distro: "Ubuntu-24.04".to_string(),
                posix: "/home/me/repo".to_string(),
            }
        );
    }

    #[test]
    fn wsl_dollar_unc_selects_wsl_target() {
        let target = select_target(r"\\wsl$\Ubuntu-24.04\home\me\repo").expect("應為 Wsl");
        assert_eq!(
            target,
            GitTarget::Wsl {
                distro: "Ubuntu-24.04".to_string(),
                posix: "/home/me/repo".to_string(),
            }
        );
    }

    #[test]
    fn prefix_matching_is_case_insensitive() {
        let target = select_target(r"\\WSL.LOCALHOST\Ubuntu-24.04\home\me\repo").expect("應為 Wsl");
        assert_eq!(
            target,
            GitTarget::Wsl {
                distro: "Ubuntu-24.04".to_string(),
                posix: "/home/me/repo".to_string(),
            }
        );
        let target = select_target(r"\\Wsl$\Ubuntu-24.04\repo").expect("應為 Wsl");
        assert_eq!(
            target,
            GitTarget::Wsl {
                distro: "Ubuntu-24.04".to_string(),
                posix: "/repo".to_string(),
            }
        );
    }

    #[test]
    fn root_of_distro_is_posix_root() {
        let target = select_target(r"\\wsl.localhost\Ubuntu-24.04").expect("應為 Wsl");
        assert_eq!(
            target,
            GitTarget::Wsl {
                distro: "Ubuntu-24.04".to_string(),
                posix: "/".to_string(),
            }
        );
        let target = select_target(r"\\wsl.localhost\Ubuntu-24.04\").expect("應為 Wsl");
        assert_eq!(
            target,
            GitTarget::Wsl {
                distro: "Ubuntu-24.04".to_string(),
                posix: "/".to_string(),
            }
        );
    }

    #[test]
    fn other_unc_forms_are_native() {
        let target = select_target(r"\\server\share\repo").expect("非 WSL UNC 應為 Native");
        assert_eq!(
            target,
            GitTarget::Native {
                path: r"\\server\share\repo".to_string()
            }
        );
    }

    #[test]
    fn invalid_distro_name_is_rejected() {
        for raw in [
            r"\\wsl.localhost\bad distro\repo",
            r"\\wsl.localhost\ba/d\repo",
            r"\\wsl.localhost\ba$d\repo",
            // distro 片段前多一個反斜線＝空字串 distro（`\\wsl.localhost\\repo`）。
            r"\\wsl.localhost\\repo",
        ] {
            assert_eq!(
                select_target(raw),
                Err(SelectTargetError::InvalidWslPath),
                "{raw:?} 應被拒絕"
            );
        }
    }

    #[test]
    fn invalid_posix_segment_is_rejected() {
        let raw = "\\\\wsl.localhost\\Ubuntu-24.04\\a\u{0}b";
        assert_eq!(select_target(raw), Err(SelectTargetError::InvalidWslPath));
    }

    #[test]
    fn native_base_argv_has_program_dir_and_fixed_prefix() {
        let target = GitTarget::Native {
            path: r"C:\repo".to_string(),
        };
        let argv = target.base_argv();
        assert_eq!(
            argv,
            vec![
                "git",
                "-C",
                r"C:\repo",
                "--no-pager",
                "--no-optional-locks",
                "--literal-pathspecs",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.quotepath=false",
                "-c",
                "color.ui=false",
                "-c",
                "log.showSignature=false",
                "-c",
                "gc.auto=0",
                "-c",
                "diff.suppressBlankEmpty=false",
            ]
        );
    }

    #[test]
    fn wsl_base_argv_uses_exec_env_git_dash_c() {
        let target = GitTarget::Wsl {
            distro: "Ubuntu-24.04".to_string(),
            posix: "/home/me/repo".to_string(),
        };
        let argv = target.base_argv();
        assert_eq!(
            &argv[..9],
            [
                "wsl.exe",
                "-d",
                "Ubuntu-24.04",
                "--exec",
                "env",
                "LC_ALL=C",
                "git",
                "-C",
                "/home/me/repo",
            ]
        );
        // 不含 `--cd`，也不含獨立的 `--`＋shell 形式（design D2、brief 驗收）。
        assert!(!argv.contains(&"--cd".to_string()));
        assert!(!argv.iter().any(|a| a == "--"));
    }
}
