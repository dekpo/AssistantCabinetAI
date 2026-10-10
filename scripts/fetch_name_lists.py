#!/usr/bin/env python3
"""Download the public name lists used to calibrate the phonetic keys (KB lot 2 bis).

Standard library only, so it runs the same on Windows and macOS:

    python scripts/fetch_name_lists.py            # fetch what is missing
    python scripts/fetch_name_lists.py --force    # fetch again

Files land in data/names/<source>/ (the whole data/ folder is git-ignored). Each archive is
untrusted data: it is extracted into its own new folder, only plain file names are accepted, and
nothing is executed. The SHA-256 of every archive is printed so a later run can be compared.

Sources and licences: docs/DATA-SOURCES.md.
"""

from __future__ import annotations

import argparse
import hashlib
import shutil
import sys
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TARGET = REPO_ROOT / "data" / "names"

HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/124.0 Safari/537.36"
    ),
    "Accept": "application/zip,*/*;q=0.8",
    "Accept-Language": "en-US,en;q=0.9",
}

# folder -> (url, page that describes the file)
SOURCES = {
    "insee-noms": (
        "https://www.insee.fr/fr/statistiques/fichier/3536630/noms2008nat_txt.zip",
        "https://www.insee.fr/fr/statistiques/3536630",
    ),
    "insee-prenoms": (
        "https://www.insee.fr/fr/statistiques/fichier/8894961/prenoms-2024-nat_csv.zip",
        "https://www.insee.fr/fr/statistiques/8894961",
    ),
    "census-2010": (
        "https://www2.census.gov/topics/genealogy/2010surnames/names.zip",
        "https://www.census.gov/topics/population/genealogy/data/2010_surnames.html",
    ),
    "ssa-babynames": (
        "https://www.ssa.gov/oact/babynames/names.zip",
        "https://www.ssa.gov/oact/babynames/limits.html",
    ),
}

MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_EXTRACTED_BYTES = 256 * 1024 * 1024


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def download(url: str, destination: Path) -> None:
    request = urllib.request.Request(url, headers=HEADERS)
    with urllib.request.urlopen(request, timeout=60) as response, destination.open("wb") as out:
        size = 0
        while block := response.read(1 << 20):
            size += len(block)
            if size > MAX_ARCHIVE_BYTES:
                raise RuntimeError(f"{url} is larger than {MAX_ARCHIVE_BYTES} bytes")
            out.write(block)


def extract(archive: Path, folder: Path) -> list[str]:
    """Extract plain files only: no directory, no path separator, no absolute or parent path."""
    written: list[str] = []
    total = 0
    with zipfile.ZipFile(archive) as bundle:
        for info in bundle.infolist():
            name = info.filename
            if info.is_dir() or name.startswith("__MACOSX"):
                continue
            if "/" in name or "\\" in name or name.startswith(".") or ".." in name:
                print(f"  skipped (unexpected path): {name!r}")
                continue
            total += info.file_size
            if total > MAX_EXTRACTED_BYTES:
                raise RuntimeError(
                    f"{archive.name} expands to more than {MAX_EXTRACTED_BYTES} bytes"
                )
            with bundle.open(info) as source, (folder / name).open("wb") as out:
                shutil.copyfileobj(source, out)
            written.append(name)
    return written


def fetch(key: str, url: str, page: str, force: bool) -> bool:
    folder = TARGET / key
    archive = folder / "archive.zip"
    extracted = folder / "extracted"
    print(f"[{key}] {url}")
    if extracted.is_dir() and any(extracted.iterdir()) and not force:
        print(f"  already present: {sorted(p.name for p in extracted.iterdir())}")
        print(f"  sha256 {sha256_of(archive) if archive.is_file() else '(archive not kept)'}")
        return True

    folder.mkdir(parents=True, exist_ok=True)
    if not archive.is_file() or force:
        try:
            download(url, archive)
        except (urllib.error.URLError, OSError, RuntimeError) as error:
            archive.unlink(missing_ok=True)
            print(f"  FAILED: {error}")
            print(f"  Download the file by hand from {page}")
            print(f"  and save it as {archive}, then run this script again.")
            return False
    if extracted.is_dir():
        shutil.rmtree(extracted)
    extracted.mkdir()
    try:
        members = extract(archive, extracted)
    except (zipfile.BadZipFile, RuntimeError) as error:
        print(f"  FAILED: {error}")
        return False
    print(f"  sha256 {sha256_of(archive)}  ({archive.stat().st_size} bytes)")
    shown = members if len(members) <= 6 else [*members[:3], f"... {len(members) - 3} more files"]
    print(f"  extracted: {shown}")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--force", action="store_true", help="download and extract again")
    args = parser.parse_args()
    TARGET.mkdir(parents=True, exist_ok=True)
    results = [fetch(key, url, page, args.force) for key, (url, page) in SOURCES.items()]
    return 0 if all(results) else 1


if __name__ == "__main__":
    sys.exit(main())
