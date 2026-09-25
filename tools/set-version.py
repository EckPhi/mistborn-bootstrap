#!/usr/bin/env python3
"""Apply a release tag's version to the Cargo manifest and lockfile in CI."""

import re
import sys
from pathlib import Path


def replace_once(path: Path, pattern: str, replacement: str) -> str:
    content = path.read_text()
    updated, count = re.subn(pattern, replacement, content, count=1, flags=re.MULTILINE)
    if count != 1:
        raise SystemExit(f"expected exactly one version field in {path}")
    return updated


def main() -> None:
    if len(sys.argv) != 2 or not re.fullmatch(r"\d+\.\d+\.\d+", sys.argv[1]):
        raise SystemExit("usage: set-version.py MAJOR.MINOR.PATCH")

    version = sys.argv[1]
    manifest = Path("Cargo.toml")
    lockfile = Path("Cargo.lock")
    updated_manifest = replace_once(
        manifest,
        r'^version = "[^"]+"$',
        f'version = "{version}"',
    )
    updated_lockfile = replace_once(
        lockfile,
        r'(\[\[package\]\]\nname = "mistborn-bootstrap"\nversion = ")[^"]+',
        rf"\g<1>{version}",
    )
    manifest.write_text(updated_manifest)
    lockfile.write_text(updated_lockfile)


if __name__ == "__main__":
    main()
