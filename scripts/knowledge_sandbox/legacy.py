"""Legacy binary Office files: XLS (BIFF8 in a compound file) and a PPT stand-in.

Standard library only. Kept apart from `writers.py` because it is the one place that writes raw binary
structures. Small tables only: the compound file has a single FAT sector (at most 64 KB).
"""

from __future__ import annotations

import struct
from pathlib import Path

_CFB_SIGNATURE = bytes.fromhex("D0CF11E0A1B11AE1")
_ENDOFCHAIN = 0xFFFFFFFE
_FREESECT = 0xFFFFFFFF
_FATSECT = 0xFFFFFFFD
_NOSTREAM = 0xFFFFFFFF
_NUL = b"\x00"


def _dir_entry(name: str, kind: int, start: int, size: int, right: int = _NOSTREAM, child: int = _NOSTREAM) -> bytes:
    raw = name.encode("utf-16-le")
    name_field = (raw + _NUL * 2).ljust(64, _NUL) if name else _NUL * 64
    return (
        name_field
        + struct.pack("<H", len(raw) + 2 if name else 0)
        + bytes([kind, 1 if kind else 0])
        + struct.pack("<III", _NOSTREAM, right, child)
        + _NUL * 16
        + struct.pack("<I", 0)
        + _NUL * 16
        + struct.pack("<I", start)
        + struct.pack("<Q", size)
    )


def _cfb(streams: list[tuple[str, bytes]]) -> bytes:
    """A compound file (OLE2) holding root-level streams, each at least 4096 bytes (so no mini stream),
    version 3 with 512-byte sectors."""
    sector = 512
    padded = []
    for name, data in sorted(streams, key=lambda item: (len(item[0]), item[0].upper())):
        if len(data) < 4096:
            data = data + _NUL * (4096 - len(data))
        if len(data) % sector:
            data = data + _NUL * (sector - len(data) % sector)
        padded.append((name, data))
    first_sector = {}
    next_sector = 2
    for name, data in padded:
        first_sector[name] = next_sector
        next_sector += len(data) // sector
    if next_sector > 128:
        raise ValueError("compound file too large for a single FAT sector")
    fat = [_FREESECT] * 128
    fat[0] = _FATSECT
    fat[1] = _ENDOFCHAIN
    for name, data in padded:
        start = first_sector[name]
        count = len(data) // sector
        for i in range(count):
            fat[start + i] = start + i + 1 if i < count - 1 else _ENDOFCHAIN
    fat_bytes = b"".join(struct.pack("<I", v) for v in fat)

    directory = [_dir_entry("Root Entry", 5, _ENDOFCHAIN, 0, child=1 if padded else _NOSTREAM)]
    for index, (name, data) in enumerate(padded, start=1):
        right = index + 1 if index < len(padded) else _NOSTREAM
        directory.append(_dir_entry(name, 2, first_sector[name], len(data), right=right))
    while len(directory) < 4:
        directory.append(_dir_entry("", 0, 0, 0))
    directory_bytes = b"".join(directory).ljust(sector, _NUL)

    header = bytearray(512)
    header[0:8] = _CFB_SIGNATURE
    struct.pack_into("<HHHHH", header, 24, 0x003E, 0x0003, 0xFFFE, 9, 6)
    struct.pack_into("<I", header, 44, 1)  # number of FAT sectors
    struct.pack_into("<I", header, 48, 1)  # first directory sector
    struct.pack_into("<I", header, 56, 4096)  # mini stream cutoff
    struct.pack_into("<I", header, 60, _ENDOFCHAIN)  # first mini FAT sector
    struct.pack_into("<I", header, 68, _ENDOFCHAIN)  # first DIFAT sector
    for i, value in enumerate([0] + [_FREESECT] * 108):
        struct.pack_into("<I", header, 76 + 4 * i, value)
    return bytes(header) + fat_bytes + directory_bytes + b"".join(data for _, data in padded)


def _biff(record: int, payload: bytes) -> bytes:
    return struct.pack("<HH", record, len(payload)) + payload


def _biff_string(text: str) -> bytes:
    if all(ord(c) < 256 for c in text):
        return struct.pack("<HB", len(text), 0) + text.encode("latin-1")
    return struct.pack("<HB", len(text), 1) + text.encode("utf-16-le")


def _bound_sheet(offset: int, name: str) -> bytes:
    encoded = name.encode("latin-1", "replace")
    return _biff(0x0085, struct.pack("<IBB", offset, 0, 0) + bytes([len(encoded), 0]) + encoded)


def write_xls(path: Path, sheets: list[tuple[str, list[list]]]) -> None:
    """A BIFF8 workbook: text and number cells only (a date is written as its serial number)."""
    head = _biff(0x0809, struct.pack("<HHHHII", 0x0600, 0x0005, 0x0DBB, 0x07CC, 0, 0x00000006))
    head += _biff(0x0042, struct.pack("<H", 1252))
    head += _biff(0x0022, struct.pack("<H", 0))
    font = struct.pack("<HHHHHBBBB", 200, 0, 0x7FFF, 400, 0, 0, 0, 0, 0) + bytes([5, 0]) + b"Arial"
    head += b"".join(_biff(0x0031, font) for _ in range(5))
    xf_style = struct.pack("<HHH", 0, 0, 0xFFF5) + _NUL * 14
    xf_cell = struct.pack("<HHH", 0, 0, 0x0001) + _NUL * 14
    head += b"".join(_biff(0x00E0, xf_style) for _ in range(15)) + _biff(0x00E0, xf_cell)
    sheet_streams = []
    for _, rows in sheets:
        body = _biff(0x0809, struct.pack("<HHHHII", 0x0600, 0x0010, 0x0DBB, 0x07CC, 0, 0x00000006))
        body += _biff(0x0200, struct.pack("<IIHHH", 0, len(rows), 0, max((len(r) for r in rows), default=0), 0))
        for r, row in enumerate(rows):
            for c, cell in enumerate(row):
                if cell is None or cell == "":
                    continue
                if isinstance(cell, tuple) and cell[0] == "date":
                    cell = cell[1]
                if isinstance(cell, (int, float)):
                    body += _biff(0x0203, struct.pack("<HHHd", r, c, 15, float(cell)))
                else:
                    body += _biff(0x0204, struct.pack("<HHH", r, c, 15) + _biff_string(str(cell)))
        body += _biff(0x000A, b"")
        sheet_streams.append(body)
    eof = _biff(0x000A, b"")
    bounds_length = sum(len(_bound_sheet(0, name)) for name, _ in sheets)
    offset = len(head) + bounds_length + len(eof)
    bounds = b""
    for (name, _), stream in zip(sheets, sheet_streams):
        bounds += _bound_sheet(offset, name)
        offset += len(stream)
    path.write_bytes(_cfb([("Workbook", head + bounds + eof + b"".join(sheet_streams))]))


def write_ppt_stand_in(path: Path, title: str, slides: list[tuple[str, list[str]]]) -> None:
    """NOT a real PowerPoint file. A compound file with a `PowerPoint Document` stream holding the slide
    texts as plain UTF-16, enough to carry the extension, the signature and the words. The binary
    PowerPoint format needs a writer the standard library does not offer; this marks the case and no more.
    The report says so wherever it counts a `.ppt`."""
    text = "\n".join([title] + [t for heading, bullets in slides for t in [heading] + bullets])
    stream = struct.pack("<HHHI", 0, 0x0FF5, 0x03E8, len(text) * 2) + text.encode("utf-16-le")
    path.write_bytes(_cfb([("PowerPoint Document", stream), ("Current User", _NUL * 64)]))
