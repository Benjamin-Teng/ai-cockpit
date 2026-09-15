"""產生 PWA 圖示（192x192、512x512 PNG）。

用法：
    uv run --no-project python cockpit/assets/gen-icons.py

design D13：一次性工具，不進 Cargo build、不需要 uv 專案。只用 stdlib
（zlib／struct／pathlib）——不裝任何套件（含 Pillow）。固定內容、不含時間戳，
在同一個 Python／zlib 版本下重跑會產出逐位元相同的 bytes；輸出直接覆寫進 repo：
    cockpit/assets/icons/icon-192.png
    cockpit/assets/icons/icon-512.png

畫面：深色底（與 cockpit/assets/manifest.webmanifest 的 background_color 一致）＋
置中淺色圓形（儀表面）＋一條對角線（儀表指針），8-bit RGB、無 interlace。
"""

import struct
import zlib
from pathlib import Path

# 深色底：cockpit/assets/app/style.css 的 --bg，同 manifest.webmanifest 的
# background_color。
BACKGROUND = (0x0F, 0x11, 0x15)
# 圓形（儀表面）：style.css 的 --text（淺色）。
DIAL = (0xE6, 0xE6, 0xE6)
# 對角線（儀表指針）：style.css 的 --status-working（綠色）。
NEEDLE = (0x3F, 0xB9, 0x50)

ICONS_DIR = Path(__file__).resolve().parent / "icons"

PNG_SIGNATURE = bytes((0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))


def _pixel(x: int, y: int, size: int) -> tuple[int, int, int]:
    """算單一像素顏色：深色底 + 置中淺色圓形 + 一條對角線（指針，穿過圓形）。"""
    cx = size / 2
    cy = size / 2
    dx = x - cx
    dy = y - cy
    radius = size * 0.36
    half_width = max(1.0, size * 0.035)

    in_circle = dx * dx + dy * dy <= radius * radius
    # dx + dy 接近 0：右上到左下的 45 度對角線（像素座標 y 軸朝下）。
    on_needle = in_circle and abs(dx + dy) <= half_width

    if on_needle:
        return NEEDLE
    if in_circle:
        return DIAL
    return BACKGROUND


def _scanlines(size: int) -> bytes:
    """組出壓縮前的原始影像資料：每個 scanline 前綴 filter byte 0（None）。"""
    rows = bytearray()
    for y in range(size):
        rows.append(0)
        for x in range(size):
            rows.extend(_pixel(x, y, size))
    return bytes(rows)


def _chunk(chunk_type: bytes, data: bytes) -> bytes:
    """組一個 PNG chunk：length(4) + type(4) + data + CRC32（over type+data，4）。"""
    crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


def _png_bytes(size: int) -> bytes:
    """組一張完整 PNG：8-bit RGB（color type 2）、無 interlace。"""
    ihdr_data = struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)
    ihdr = _chunk(b"IHDR", ihdr_data)
    idat = _chunk(b"IDAT", zlib.compress(_scanlines(size), 9))
    iend = _chunk(b"IEND", b"")
    return PNG_SIGNATURE + ihdr + idat + iend


def main() -> None:
    ICONS_DIR.mkdir(parents=True, exist_ok=True)
    for size in (192, 512):
        out = ICONS_DIR / f"icon-{size}.png"
        out.write_bytes(_png_bytes(size))
        print(f"wrote {out} ({size}x{size})")


if __name__ == "__main__":
    main()
