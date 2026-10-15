"""Run a command and report the peak memory of the process (its resident set), for the baselines of
`docs/LONG-DOCUMENTS.md`. Measurement tooling, not product code. Works on Windows and on macOS / Linux.

    python scripts/peak_memory.py -- <command> [arguments]

Variables the child needs are read from the environment of this command. Prints one line:
`peak_bytes=<n> peak_mb=<n> exit_code=<n> seconds=<n>`. On Windows the figure is the process's own peak working set;
on macOS / Linux it is `ru_maxrss` of the finished children (so run one command per invocation).
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from dataclasses import dataclass


@dataclass
class Result:
    exit_code: int
    peak_bytes: int
    seconds: float


def _peak_windows(process: subprocess.Popen) -> int:
    import ctypes
    from ctypes import wintypes

    class Counters(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD), ("PeakWorkingSetSize", ctypes.c_size_t),
                    ("WorkingSetSize", ctypes.c_size_t), ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaPagedPoolUsage", ctypes.c_size_t), ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaNonPagedPoolUsage", ctypes.c_size_t), ("PagefileUsage", ctypes.c_size_t),
                    ("PeakPagefileUsage", ctypes.c_size_t)]

    counters = Counters()
    counters.cb = ctypes.sizeof(Counters)
    get = ctypes.windll.psapi.GetProcessMemoryInfo
    get.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
    if not get(wintypes.HANDLE(int(process._handle)), ctypes.byref(counters), counters.cb):  # type: ignore[attr-defined]
        raise OSError("GetProcessMemoryInfo failed")
    return int(counters.PeakWorkingSetSize)


def _peak_unix() -> int:
    import resource

    peak = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    return int(peak if sys.platform == "darwin" else peak * 1024)


def run(command: list, env: dict | None = None) -> Result:
    started = time.monotonic()
    process = subprocess.Popen(command, env={**os.environ, **(env or {})})
    exit_code = process.wait()
    seconds = time.monotonic() - started
    peak = _peak_windows(process) if os.name == "nt" else _peak_unix()
    return Result(exit_code, peak, seconds)


def main(argv: list | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] == "--":
        argv = argv[1:]
    if not argv:
        print(__doc__)
        return 2
    result = run(argv)
    print(f"peak_bytes={result.peak_bytes} peak_mb={result.peak_bytes / 1048576:.0f} exit_code={result.exit_code} seconds={result.seconds:.1f}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
