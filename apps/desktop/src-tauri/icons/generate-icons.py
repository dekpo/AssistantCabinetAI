"""Generate the application icons.

Tauri forbids shipping its own icon, and the practice window carries no third-party mark, so the
product needs its own. This draws a plain document on the product colour: a placeholder a designer
can replace, not a logo pretending to be one. No third-party library, so it runs anywhere.

    python generate-icons.py
"""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent

BACKGROUND = (31, 93, 84, 255)  # the product colour, as in the interface stylesheet
PAPER = (255, 255, 255, 255)
SUPERSAMPLE = 4

PNG_SIZES = {
    "32x32.png": 32,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "icon.png": 512,
}
ICO_SIZES = (16, 32, 48, 64, 128, 256)


def rounded_rectangle(x: float, y: float, box: tuple[float, float, float, float], radius: float) -> bool:
    """Distance to the rectangle shrunk by the corner radius: inside means within that radius."""
    left, top, right, bottom = box
    radius = min(radius, (right - left) / 2, (bottom - top) / 2)
    nearest_x = min(max(x, left + radius), right - radius)
    nearest_y = min(max(y, top + radius), bottom - radius)
    return (x - nearest_x) ** 2 + (y - nearest_y) ** 2 <= radius**2


def colour_at(x: float, y: float) -> tuple[int, int, int, int]:
    """Coordinates are fractions of the icon, so the drawing is resolution independent."""
    if not rounded_rectangle(x, y, (0.02, 0.02, 0.98, 0.98), 0.22):
        return (0, 0, 0, 0)
    document = rounded_rectangle(x, y, (0.30, 0.22, 0.70, 0.78), 0.05)
    if not document:
        return BACKGROUND
    for line_top in (0.34, 0.46, 0.58):
        if rounded_rectangle(x, y, (0.37, line_top, 0.63, line_top + 0.05), 0.02):
            return BACKGROUND
    return PAPER


def render(size: int) -> bytes:
    """One RGBA image, supersampled so the rounded corners do not look cut with scissors."""
    rows: list[bytes] = []
    grid = size * SUPERSAMPLE
    for row in range(size):
        pixels = bytearray()
        for column in range(size):
            totals = [0, 0, 0, 0]
            for sub_row in range(SUPERSAMPLE):
                for sub_column in range(SUPERSAMPLE):
                    x = (column * SUPERSAMPLE + sub_column + 0.5) / grid
                    y = (row * SUPERSAMPLE + sub_row + 0.5) / grid
                    for channel, value in enumerate(colour_at(x, y)):
                        totals[channel] += value
            samples = SUPERSAMPLE * SUPERSAMPLE
            pixels.extend(total // samples for total in totals)
        rows.append(bytes(pixels))
    return encode_png(size, rows)


def encode_png(size: int, rows: list[bytes]) -> bytes:
    def chunk(kind: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + kind
            + payload
            + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
        )

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    raw = b"".join(b"\x00" + row for row in rows)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def encode_ico(images: dict[int, bytes]) -> bytes:
    count = len(images)
    directory = b""
    payload = b""
    offset = 6 + 16 * count
    for size, png in images.items():
        directory += struct.pack(
            "<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(png), offset + len(payload)
        )
        payload += png
    return struct.pack("<HHH", 0, 1, count) + directory + payload


def main() -> None:
    rendered: dict[int, bytes] = {}
    for size in sorted({*PNG_SIZES.values(), *ICO_SIZES}):
        rendered[size] = render(size)
        print(f"rendered {size}x{size}")

    for name, size in PNG_SIZES.items():
        (HERE / name).write_bytes(rendered[size])
    (HERE / "icon.ico").write_bytes(encode_ico({size: rendered[size] for size in ICO_SIZES}))
    print(f"wrote {len(PNG_SIZES) + 1} files into {HERE}")


if __name__ == "__main__":
    main()
