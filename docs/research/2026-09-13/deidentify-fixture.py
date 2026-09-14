#!/usr/bin/env python3
"""去識別化一行 HERDR success_response JSON、或一份 NDJSON 事件檔，輸出成可重用的合約測試
fixture。

用法：
    python deidentify-fixture.py <輸入檔> <輸出檔路徑> [--ndjson]

省略 `--ndjson`（預設）：輸入檔只取第一行，視為一個 JSON 物件（例如
`session.snapshot` 的 success_response），輸出以 2 空格縮排 pretty-print。

加 `--ndjson`（task 5.1 新增）：輸入檔逐行處理，每行各自視為一個獨立的事件 JSON 物件
（例如 `events.subscribe` 收到的一行事件），輸出保持 NDJSON 形狀（每行一個緊湊 JSON、
不 pretty-print），行數與原始檔完全相同——去識別化的編號映射（`_project_map` 等）在整份
檔案內共用同一個 `Deidentifier`，不是每行重開一份，維持「同一個原始值全文件內固定映射到
同一個編號」的規則不變。

替換規則（同一個原始值在整份文件內固定映射到同一個編號；規則之間各自獨立編號）：
    - "cwd"／"foreground_cwd" 欄位的字串值 → "D:\\PROJECT_<n>"（原值不以 "/" 開頭）或
      "/PROJECT_<n>"（原值以 "/" 開頭）。
    - "terminal_title"／"terminal_title_stripped"／"title" 欄位的字串值 → "TITLE_<n>"。
    - workspace／tab／pane 物件的 "label" 欄位字串值 → "LABEL_<n>"。
    - "agent_session" 物件的 "value" 子欄位 → 固定常數 "REDACTED_SESSION"（不編號）。
    - "tokens" 物件內每一個值 → 固定常數 "0"（不編號、不看原值是否相同）。
    - 其餘欄位（含 id、pane_id、tab_id、workspace_id、terminal_id、agent、agent_status、
      scroll、revision、layouts 等）原樣保留。

None／非字串值一律原樣保留（不佔用編號、不誤判為需替換的字串）。
輸出以 2 空格縮排 pretty-print，UTF-8，結尾補一個換行字元。

供 change herdr-client 的 spike 2／3 共用：spike 2 存 protocol 20 的 session_snapshot，
spike 3 之後也可能需要對事件行做同樣的去識別化（事件行的 "data" 內同樣可能出現
cwd／title／label／agent_session／tokens 欄位，本腳本的遞迴邏輯不假設頂層一定是
session_snapshot，一樣適用）。
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

_PROJECT_FIELDS = {"cwd", "foreground_cwd"}
_TITLE_FIELDS = {"terminal_title", "terminal_title_stripped", "title"}
_LABEL_FIELDS = {"label"}
_REDACTED_SESSION = "REDACTED_SESSION"
_REDACTED_TOKEN_VALUE = "0"


class Deidentifier:
    """走訪一份已解析的 JSON 值，就地把敏感欄位換成可重現的去識別化編號。"""

    def __init__(self) -> None:
        self._project_map: dict[str, str] = {}
        self._title_map: dict[str, str] = {}
        self._label_map: dict[str, str] = {}

    def _allocate(self, mapping: dict[str, str], original: str, template: str) -> str:
        if original not in mapping:
            mapping[original] = template.format(n=len(mapping) + 1)
        return mapping[original]

    def _project_value(self, original: str) -> str:
        template = "/PROJECT_{n}" if original.startswith("/") else "D:\\PROJECT_{n}"
        return self._allocate(self._project_map, original, template)

    def _title_value(self, original: str) -> str:
        return self._allocate(self._title_map, original, "TITLE_{n}")

    def _label_value(self, original: str) -> str:
        return self._allocate(self._label_map, original, "LABEL_{n}")

    def walk(self, node: Any) -> Any:
        if isinstance(node, dict):
            return self._walk_dict(node)
        if isinstance(node, list):
            return [self.walk(item) for item in node]
        return node

    def _walk_dict(self, node: dict[str, Any]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in node.items():
            if key in _PROJECT_FIELDS and isinstance(value, str):
                result[key] = self._project_value(value)
            elif key in _TITLE_FIELDS and isinstance(value, str):
                result[key] = self._title_value(value)
            elif key in _LABEL_FIELDS and isinstance(value, str):
                result[key] = self._label_value(value)
            elif key == "agent_session" and isinstance(value, dict):
                result[key] = self._walk_agent_session(value)
            elif key == "tokens" and isinstance(value, dict):
                result[key] = {token_key: _REDACTED_TOKEN_VALUE for token_key in value}
            else:
                result[key] = self.walk(value)
        return result

    def _walk_agent_session(self, session: dict[str, Any]) -> dict[str, Any]:
        result = self._walk_dict(session)
        if "value" in session:
            result["value"] = _REDACTED_SESSION
        return result


def deidentify_line(raw_line: str) -> Any:
    parsed = json.loads(raw_line)
    return Deidentifier().walk(parsed)


def deidentify_ndjson(raw_text: str) -> list[Any]:
    """逐行去識別化；所有行共用同一個 `Deidentifier`，讓編號映射跨行一致。"""
    deidentifier = Deidentifier()
    results = []
    for line in raw_text.splitlines():
        if not line.strip():
            continue
        results.append(deidentifier.walk(json.loads(line)))
    return results


def main(argv: list[str]) -> int:
    args = [a for a in argv[1:] if a != "--ndjson"]
    ndjson = "--ndjson" in argv[1:]
    if len(args) != 2:
        print(
            f"用法: {argv[0]} <輸入檔> <輸出檔路徑> [--ndjson]",
            file=sys.stderr,
        )
        return 2

    input_path, output_path = args
    raw_text = Path(input_path).read_text(encoding="utf-8")
    if not raw_text.strip():
        print(f"輸入檔 {input_path} 是空的", file=sys.stderr)
        return 1

    if ndjson:
        deidentified_lines = deidentify_ndjson(raw_text)
        with open(output_path, "w", encoding="utf-8", newline="\n") as f:
            for obj in deidentified_lines:
                f.write(json.dumps(obj, ensure_ascii=False))
                f.write("\n")
        return 0

    deidentified = deidentify_line(raw_text.splitlines()[0])
    with open(output_path, "w", encoding="utf-8", newline="\n") as f:
        json.dump(deidentified, f, indent=2, ensure_ascii=False)
        f.write("\n")

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
