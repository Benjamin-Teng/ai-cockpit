//! Git Graph 排版純函式（git-review task 2.5、design D8、spec「commit 清單端點」`graph` 欄位）。
//!
//! 輸入為依 `--date-order` 排好的 commit 清單（每筆 [`GraphCommit`]，只有 `oid` 與
//! `parents`——不依賴 [`crate::log::LogRow`]，呼叫端自行從 `LogOutput` 借出這兩個欄位，
//! 耦合較低）；「依 `--date-order` 排好」代表任何 commit 都排在它所有子 commit 之後（design
//! D4／spec「commit 清單端點」）。輸出為每一列的排版（[`GraphRow`]）：純函式，不做任何 I/O，
//! 不知道 HTTP 或 JSON。
//!
//! ## 線段語意（task 2.5 控制端裁決，spec 只定欄位，細節由這裡定義）
//!
//! 一列畫在固定高度的格子裡：[`GraphHalf::Top`] 是「從列頂邊到節點中心高度」，
//! [`GraphHalf::Bottom`] 是「從節點中心高度到列底邊」；[`GraphLine::from`]／[`GraphLine::to`]
//! 是這半段起訖的欄。穿過本列但與本列節點無關的車道＝同一欄的 `Top` 與 `Bottom` 各一段
//! （欄不變）。
//!
//! **連續性**：第 i 列某段 `Bottom` 的 `to` 欄與顏色，必須等於第 i+1 列某段 `Top` 的 `from`
//! 欄與顏色（同一條車道）——這個性質由演算法的建構方式保證（見下），測試以
//! `tests::assert_continuous` 逐列查核，隨機 DAG 的性質測試也套用同一個檢查函式。
//!
//! ## 演算法：進行中的車道
//!
//! 維護一組欄位（`lanes: Vec<Option<Lane>>`），每條車道記著它在等哪個 oid 與顏色。逐列處理：
//!
//! 1. 找出目前在等這個 commit 的所有欄（`found`，由小到大排序）。
//! 2. 其餘仍在等別的 oid 的欄，本列與節點無關，各自輸出一段 `Top` 與一段 `Bottom`（欄不變、
//!    顏色不變）——維持直線。
//! 3. `found` 為空：這個 commit 沒有任何已知車道在等（起點／已載入範圍內的新根），節點欄配置
//!    最左邊的空欄，顏色依全域計數器（0–5 循環）指派新色；不輸出 `Top`（列頂沒有線進來）。
//! 4. `found` 非空：節點欄＝`found` 中最小的欄，節點顏色＝該欄原本的顏色（沿第一父鏈延續）。
//!    `found` 中其餘的欄（有其他 commit 也指向同一個 parent，例如分支起點）在這列輸出
//!    `Top{from: 該欄, to: 節點欄, color: 該欄原本的顏色}`（合併進來的線段用來源車道的顏色）；
//!    節點欄本身輸出 `Top{from: 節點欄, to: 節點欄, color: 節點顏色}`。`found` 的欄全部釋放
//!    （設為空，可能立刻被本列稍後的配置重用）。
//! 5. 第一個 parent（若有）留在節點欄，車道改等它，輸出 `Bottom{節點欄, 節點欄, 節點顏色}`；
//!    沒有 parent（根 commit）則節點欄保持空。第一父鏈因此保持同一欄（spec 明文要求）。
//! 6. 其餘的 parent 依序處理：若已有車道在等它，輸出 `Bottom{from: 節點欄, to: 該車道欄,
//!    color: 該車道原本的顏色}`（連過去，車道不變）；否則配置最左邊的空欄、指派新顏色，輸出
//!    `Bottom{from: 節點欄, to: 新欄, color: 新顏色}`。
//!
//! **parent 不在已載入清單內**（被篩選掉或超出已載入範圍）：那個 parent 的 oid 永遠不會被
//! `found` 命中，於是它的車道在步驟 2 每一列都當成「與本列節點無關」持續輸出 `Top`／`Bottom`
//! 直到清單結束（最後一列仍輸出 `Bottom`）——不中斷、不報錯，也不需要特殊處理。
//!
//! **前綴穩定**：第 i 列的計算只讀取「處理前 i 列所累積的 `lanes` 狀態」，不看第 i 列之後的
//! commit，所以對前 N 筆單獨呼叫 [`layout`] 與對整份清單呼叫後取前 N 列，逐列完全相同（見
//! `tests::prefix_layout_matches_full_layout_prefix` 的隨機 DAG 性質測試）。
//!
//! **顏色**：6 色循環（0–5），由一個貫穿整份排版的計數器指派——包含步驟 3（新起點）與步驟 6
//! （新配置的額外 parent 車道）——沿同一條第一父鏈延續（步驟 5 不消耗計數器）。

/// 一段線落在列的上半或下半（模組文件「線段語意」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphHalf {
    /// 從列頂邊到節點中心高度。
    Top,
    /// 從節點中心高度到列底邊。
    Bottom,
}

/// 一列要畫的一段線。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphLine {
    pub from: usize,
    pub to: usize,
    pub half: GraphHalf,
    pub color: u8,
}

/// 一列的排版（spec「commit 清單端點」`graph` 欄位）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRow {
    pub col: usize,
    pub color: u8,
    pub lines: Vec<GraphLine>,
}

/// [`layout`] 的輸入：只需要 `oid` 與 `parents`（模組文件開頭「不依賴 `LogRow`」）。
#[derive(Debug, Clone, Copy)]
pub struct GraphCommit<'a> {
    pub oid: &'a str,
    pub parents: &'a [String],
}

/// 進行中的一條車道：正在等哪個 oid 抵達，以及這條車道的顏色。
struct Lane {
    awaiting: String,
    color: u8,
}

/// 配置最左邊的空欄（[`None`]），沒有就在尾端新增一欄；回傳配置到的欄。
fn alloc_column(lanes: &mut Vec<Option<Lane>>) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(col) => col,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

/// 依全域計數器指派下一個顏色（0–5 循環，模組文件「顏色」）。
fn alloc_color(next_color: &mut u8) -> u8 {
    let color = *next_color % 6;
    *next_color = next_color.wrapping_add(1);
    color
}

/// 對依 `--date-order` 排好的 commit 清單排版（模組文件「演算法」）。
pub fn layout(commits: &[GraphCommit<'_>]) -> Vec<GraphRow> {
    let mut lanes: Vec<Option<Lane>> = Vec::new();
    let mut next_color: u8 = 0;
    let mut rows = Vec::with_capacity(commits.len());

    for commit in commits {
        // 1. 找出目前在等這個 commit 的所有欄。
        let mut found: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter_map(|(col, lane)| {
                lane.as_ref()
                    .filter(|lane| lane.awaiting == commit.oid)
                    .map(|_| col)
            })
            .collect();
        found.sort_unstable();

        let mut lines = Vec::new();

        // 2. 與本列節點無關的車道：同一欄輸出 Top 與 Bottom 各一段，維持直線。
        for (col, lane) in lanes.iter().enumerate() {
            if let Some(lane) = lane
                && !found.contains(&col)
            {
                lines.push(GraphLine {
                    from: col,
                    to: col,
                    half: GraphHalf::Top,
                    color: lane.color,
                });
                lines.push(GraphLine {
                    from: col,
                    to: col,
                    half: GraphHalf::Bottom,
                    color: lane.color,
                });
            }
        }

        // 3／4. 決定節點欄與顏色。
        let (node_col, node_color) = if let Some(&primary) = found.first() {
            let color = lanes[primary]
                .as_ref()
                .expect("found 只收集 Some 的欄")
                .color;
            (primary, color)
        } else {
            let col = alloc_column(&mut lanes);
            let color = alloc_color(&mut next_color);
            (col, color)
        };

        for &col in &found {
            if col == node_col {
                lines.push(GraphLine {
                    from: col,
                    to: col,
                    half: GraphHalf::Top,
                    color: node_color,
                });
            } else {
                let source_color = lanes[col].as_ref().expect("found 只收集 Some 的欄").color;
                lines.push(GraphLine {
                    from: col,
                    to: node_col,
                    half: GraphHalf::Top,
                    color: source_color,
                });
            }
        }
        for &col in &found {
            lanes[col] = None;
        }

        // 5. 第一個 parent 留在節點欄，第一父鏈保持同一欄。
        if let Some(first_parent) = commit.parents.first() {
            lanes[node_col] = Some(Lane {
                awaiting: first_parent.clone(),
                color: node_color,
            });
            lines.push(GraphLine {
                from: node_col,
                to: node_col,
                half: GraphHalf::Bottom,
                color: node_color,
            });
        } else {
            lanes[node_col] = None;
        }

        // 6. 其餘的 parent：已有車道在等就連過去，否則配置新欄與新顏色。
        for parent in commit.parents.iter().skip(1) {
            let existing = lanes
                .iter()
                .position(|lane| matches!(lane, Some(lane) if &lane.awaiting == parent));
            match existing {
                Some(col) => {
                    let color = lanes[col]
                        .as_ref()
                        .expect("existing 只會是 Some 的欄")
                        .color;
                    lines.push(GraphLine {
                        from: node_col,
                        to: col,
                        half: GraphHalf::Bottom,
                        color,
                    });
                }
                None => {
                    let col = alloc_column(&mut lanes);
                    let color = alloc_color(&mut next_color);
                    lanes[col] = Some(Lane {
                        awaiting: parent.clone(),
                        color,
                    });
                    lines.push(GraphLine {
                        from: node_col,
                        to: col,
                        half: GraphHalf::Bottom,
                        color,
                    });
                }
            }
        }

        rows.push(GraphRow {
            col: node_col,
            color: node_color,
            lines,
        });
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 測試用的擁有型 commit（[`GraphCommit`] 只借用，測試需要先把資料存活在某處）。
    struct Owned {
        oid: String,
        parents: Vec<String>,
    }

    fn owned(specs: &[(&str, &[&str])]) -> Vec<Owned> {
        specs
            .iter()
            .map(|(oid, parents)| Owned {
                oid: (*oid).to_string(),
                parents: parents.iter().map(|p| (*p).to_string()).collect(),
            })
            .collect()
    }

    fn borrowed(owned: &[Owned]) -> Vec<GraphCommit<'_>> {
        owned
            .iter()
            .map(|o| GraphCommit {
                oid: &o.oid,
                parents: &o.parents,
            })
            .collect()
    }

    #[test]
    fn single_root_commit_has_no_lines() {
        let commits = owned(&[("a", &[])]);
        let rows = layout(&borrowed(&commits));

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].col, 0);
        assert_eq!(rows[0].color, 0);
        assert!(
            rows[0].lines.is_empty(),
            "根 commit 沒有 parent，不應有任何線段"
        );
    }

    /// 檢查模組文件「連續性」：第 i 列某段 `Bottom` 的 `to` 欄與顏色，必須等於第 i+1 列
    /// 某段 `Top` 的 `from` 欄與顏色。所有排版測試（含隨機 DAG 性質測試）都套用這個函式。
    fn assert_continuous(rows: &[GraphRow]) {
        for i in 0..rows.len().saturating_sub(1) {
            for bottom in rows[i]
                .lines
                .iter()
                .filter(|line| line.half == GraphHalf::Bottom)
            {
                let has_match = rows[i + 1].lines.iter().any(|top| {
                    top.half == GraphHalf::Top && top.from == bottom.to && top.color == bottom.color
                });
                assert!(
                    has_match,
                    "第 {i} 列的 bottom 段 {bottom:?} 在第 {} 列找不到對應的 top 段",
                    i + 1
                );
            }
        }
    }

    #[test]
    fn linear_history_stays_in_one_column_with_one_color() {
        // C 是 B 的子、B 是 A 的子：--date-order 之下 C、B、A 依序排列。
        let commits = owned(&[("c", &["b"]), ("b", &["a"]), ("a", &[])]);
        let rows = layout(&borrowed(&commits));

        assert_eq!(rows.len(), 3);
        for row in &rows {
            assert_eq!(row.col, 0, "沒有分岔的第一父鏈應保持同一欄");
            assert_eq!(row.color, 0);
        }
        assert_eq!(
            rows[0].lines,
            vec![GraphLine {
                from: 0,
                to: 0,
                half: GraphHalf::Bottom,
                color: 0
            }],
            "C 是起點，沒有東西在等它；只有一段 bottom 通往 B"
        );
        assert_eq!(
            rows[1].lines,
            vec![
                GraphLine {
                    from: 0,
                    to: 0,
                    half: GraphHalf::Top,
                    color: 0
                },
                GraphLine {
                    from: 0,
                    to: 0,
                    half: GraphHalf::Bottom,
                    color: 0
                },
            ],
            "B 有 top（接住 C）與 bottom（通往 A）"
        );
        assert_eq!(
            rows[2].lines,
            vec![GraphLine {
                from: 0,
                to: 0,
                half: GraphHalf::Top,
                color: 0
            }],
            "A 是根，只有 top（接住 B），沒有 bottom"
        );
        assert_continuous(&rows);
    }

    #[test]
    fn branch_and_merge_layout_matches_spec_scenario() {
        // spec「分支與合併的排版」：main 上 A→B，從 B 分出 feat 有 C，main 上有 D，
        // 再以 E 合併 feat 回 main（E 的 parents 為 D、C）。
        let commits = owned(&[
            ("e", &["d", "c"]),
            ("d", &["b"]),
            ("c", &["b"]),
            ("b", &["a"]),
            ("a", &[]),
        ]);
        let rows = layout(&borrowed(&commits));
        assert_eq!(rows.len(), 5);
        let (e, d, c, b, a) = (&rows[0], &rows[1], &rows[2], &rows[3], &rows[4]);

        assert_eq!(e.col, d.col, "E 與 D 應同欄");
        assert_eq!(d.col, b.col, "D 與 B 應同欄");
        assert_eq!(b.col, a.col, "B 與 A 應同欄");
        assert_ne!(c.col, e.col, "C 的欄應與 E、D、B、A 不同");

        // E 與 C 之間有線段相連：E 這列往 C 的欄配置了一條新車道（bottom）。
        assert!(
            e.lines
                .iter()
                .any(|l| l.half == GraphHalf::Bottom && l.to == c.col),
            "E 應有一段 bottom 通往 C 的欄"
        );
        // C 與 B 之間有線段相連：B 這列有一段 top 從 C 的欄合併進來。
        assert!(
            b.lines
                .iter()
                .any(|l| l.half == GraphHalf::Top && l.from == c.col && l.to == b.col),
            "B 應有一段 top 從 C 的欄合併進來"
        );

        assert_continuous(&rows);
    }

    #[test]
    fn octopus_merge_gives_each_parent_a_distinct_lane() {
        // M 是 octopus merge，三個 parent 互不相干（各自是根），驗收清單明確要求覆蓋。
        let commits = owned(&[
            ("m", &["p1", "p2", "p3"]),
            ("p1", &[]),
            ("p2", &[]),
            ("p3", &[]),
        ]);
        let rows = layout(&borrowed(&commits));
        assert_eq!(rows.len(), 4);
        let m = &rows[0];
        assert_eq!(m.lines.len(), 3, "M 應有三段 bottom，分別通往三個 parent");

        let mut targets: Vec<usize> = m
            .lines
            .iter()
            .filter(|l| l.half == GraphHalf::Bottom)
            .map(|l| l.to)
            .collect();
        targets.sort_unstable();
        targets.dedup();
        assert_eq!(targets.len(), 3, "三個 parent 應各自配置不同的欄");

        let mut colors: Vec<u8> = m
            .lines
            .iter()
            .filter(|l| l.half == GraphHalf::Bottom)
            .map(|l| l.color)
            .collect();
        colors.sort_unstable();
        colors.dedup();
        assert_eq!(colors.len(), 3, "三個 parent 的車道應各自有不同顏色");

        assert_continuous(&rows);
    }

    #[test]
    fn multiple_root_commits_are_handled_independently() {
        // 兩條互不相干的歷史（例如 Log 的 tips 包含兩個沒有共同祖先的分支）。
        let commits = owned(&[("x2", &["x1"]), ("x1", &[]), ("y2", &["y1"]), ("y1", &[])]);
        let rows = layout(&borrowed(&commits));
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].col, rows[1].col, "x2 與 x1 應同欄");
        assert_eq!(rows[2].col, rows[3].col, "y2 與 y1 應同欄");
        assert!(
            rows[1].lines.iter().all(|l| l.half == GraphHalf::Top),
            "x1 是根，不應有 bottom"
        );
        assert!(
            rows[3].lines.iter().all(|l| l.half == GraphHalf::Top),
            "y1 是根，不應有 bottom"
        );
        assert_continuous(&rows);
    }

    #[test]
    fn parent_outside_loaded_range_keeps_lane_open_to_last_row() {
        // C 的 parent「missing」不在清單內（被篩選掉或超出已載入範圍）；D、E 是另外兩個
        // 不相干的根，用來確認 missing 的車道持續延伸、不中斷、不報錯。
        let commits = owned(&[("c", &["missing"]), ("d", &[]), ("e", &[])]);
        let rows = layout(&borrowed(&commits));
        assert_eq!(rows.len(), 3);

        // 每一列都應該看得到 missing 車道的 bottom 段（第一列）或 top+bottom 直線
        // （之後每一列，因為它與本列節點無關），一路延伸到最後一列的底邊。
        let missing_col = rows[0]
            .lines
            .iter()
            .find(|l| l.half == GraphHalf::Bottom)
            .expect("C 應該有通往 missing 的 bottom")
            .to;
        for row in &rows {
            assert!(
                row.lines
                    .iter()
                    .any(|l| l.half == GraphHalf::Bottom && l.to == missing_col),
                "missing 車道應該在每一列都持續往下延伸"
            );
        }
        assert_continuous(&rows);
    }

    /// 極簡 xorshift32，固定種子重現隨機 DAG（brief：不得新增依賴，不用 proptest／rand）。
    struct Xorshift32(u32);

    impl Xorshift32 {
        fn new(seed: u32) -> Self {
            Self(if seed == 0 { 0x9e3779b9 } else { seed })
        }

        fn next_u32(&mut self) -> u32 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.0 = x;
            x
        }

        /// `[0, bound)` 的隨機整數；`bound == 0` 回傳 0。
        fn next_range(&mut self, bound: usize) -> usize {
            if bound == 0 {
                0
            } else {
                (self.next_u32() as usize) % bound
            }
        }
    }

    /// 產生一個「每個 commit 排在它所有子 commit 之後」的隨機 DAG（模擬 `--date-order`）：
    /// 索引 i 的 commit 只能以索引大於 i 的 commit 當 parent。涵蓋多根、octopus（最多 3
    /// parent）、長分支、交錯合併——用種子與筆數控制。
    fn random_dag(seed: u32, n: usize) -> Vec<Owned> {
        let mut rng = Xorshift32::new(seed);
        let mut commits = Vec::with_capacity(n);
        for i in 0..n {
            let oid = format!("c{i:05}");
            let remaining = n - i - 1;
            let max_parents = remaining.min(3);
            let num_parents = if max_parents == 0 {
                0
            } else {
                rng.next_range(max_parents + 1)
            };
            let mut candidates: Vec<usize> = ((i + 1)..n).collect();
            let mut parent_indices = Vec::with_capacity(num_parents);
            for _ in 0..num_parents {
                if candidates.is_empty() {
                    break;
                }
                let pick = rng.next_range(candidates.len());
                parent_indices.push(candidates.remove(pick));
            }
            let parents = parent_indices
                .into_iter()
                .map(|p| format!("c{p:05}"))
                .collect();
            commits.push(Owned { oid, parents });
        }
        commits
    }

    #[test]
    fn random_dags_satisfy_continuity() {
        for seed in [1u32, 42, 1_000_003, 0xdead_beef, 7] {
            for n in [1usize, 5, 30, 120] {
                let commits = random_dag(seed, n);
                let rows = layout(&borrowed(&commits));
                assert_eq!(rows.len(), n);
                assert_continuous(&rows);
            }
        }
    }

    /// 前綴穩定性質測試（brief）：對多個隨機 DAG，逐一比對「前 N 列單獨排版」與「整份排版
    /// 的前 N 列」完全相同，N 取多個值。
    #[test]
    fn prefix_layout_matches_full_layout_prefix() {
        for seed in [1u32, 2, 17, 99, 1_234_567, 0x00c0_ffee] {
            let n = 150;
            let commits = random_dag(seed, n);
            let full_rows = layout(&borrowed(&commits));
            assert_eq!(full_rows.len(), n);

            for &prefix_n in &[1usize, 2, 10, 50, 100, 149, 150] {
                let prefix_commits = &commits[..prefix_n];
                let prefix_rows = layout(&borrowed(prefix_commits));
                assert_eq!(
                    prefix_rows,
                    full_rows[..prefix_n],
                    "seed={seed} 的 DAG：前 {prefix_n} 列單獨排版應與整份排版的前 {prefix_n} 列完全相同"
                );
            }
        }
    }

    /// 粗略的效能檢查（brief：不做成易碎的計時斷言）：5000 筆 commit、20 條並行車道，
    /// debug build 下應遠低於 1 秒。這裡放寬到 5 秒，只當作「沒有明顯 O(n²) 或更糟」的
    /// 回歸守門，不是嚴謹的 benchmark。
    #[test]
    fn layout_5000_commits_with_20_lanes_is_fast_enough() {
        const LANES: usize = 20;
        const DEPTH: usize = 250;
        // 20 條互相獨立的長鏈交錯：先列出每條鏈的第 0 層（最新），再列出第 1 層……
        // 同一條鏈內先子後 parent，滿足「排在它所有子 commit 之後」。
        let mut commits = Vec::with_capacity(LANES * DEPTH);
        for depth in 0..DEPTH {
            for lane in 0..LANES {
                let oid = format!("lane{lane:02}-{depth:04}");
                let parents = if depth + 1 < DEPTH {
                    vec![format!("lane{lane:02}-{:04}", depth + 1)]
                } else {
                    Vec::new()
                };
                commits.push(Owned { oid, parents });
            }
        }
        assert_eq!(commits.len(), LANES * DEPTH);

        let graph_commits = borrowed(&commits);
        let start = std::time::Instant::now();
        let rows = layout(&graph_commits);
        let elapsed = start.elapsed();

        assert_eq!(rows.len(), LANES * DEPTH);
        assert!(
            elapsed.as_secs_f64() < 5.0,
            "5000 筆 commit 排版花了 {elapsed:?}，遠超預期（粗略守門，非精確 benchmark）"
        );
    }
}
