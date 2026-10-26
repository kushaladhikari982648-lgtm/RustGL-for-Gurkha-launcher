#!/usr/bin/env python3
"""Check that rustgl/ is byte-identical to the renderer source it was copied from.

The renderer in this repository is synced from the Gurkha Launcher repository
(directory rustgl/ there). renderer-source.json records the exact Git tree hash
of that directory, so `python3 tools/check_sync.py` proves the local copy has
not been modified - without needing network access.

The hash is a Git tree object id computed the way Git computes it: recursively,
from the mode, name and blob id of every file (only text files are synced, all
of them mode 100644).
"""

import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE_DIR = ROOT / "rustgl"
RECORD = ROOT / "renderer-source.json"

SKIP = {".git", "target", ".cargo-ok"}


def blob_hash(data: bytes) -> bytes:
    return hashlib.sha1(b"blob " + str(len(data)).encode() + b"\x00" + data).digest()


def tree_hash(directory: Path) -> str:
    entries = []
    for child in directory.iterdir():
        if child.name in SKIP:
            continue
        if child.is_dir():
            entries.append((b"40000", child.name, tree_hash(child)))
        else:
            mode = b"100755" if child.stat().st_mode & 0o111 else b"100644"
            entries.append((mode, child.name, blob_hash(child.read_bytes()).hex()))
    # Git sorts tree entries by name, comparing directories as if they ended in "/".
    entries.sort(key=lambda item: (item[1] + "/" if item[0] == b"40000" else item[1]).encode())
    payload = b"".join(mode + b" " + name.encode() + b"\x00" + bytes.fromhex(digest)
                       for mode, name, digest in entries)
    header = b"tree " + str(len(payload)).encode() + b"\x00"
    return hashlib.sha1(header + payload).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record", action="store_true",
                        help="print the current tree hash instead of checking it")
    args = parser.parse_args()

    actual = tree_hash(SOURCE_DIR)
    if args.record:
        print(actual)
        return 0

    record = json.loads(RECORD.read_text(encoding="utf-8"))
    expected = record["treeHash"]
    print(f"rustgl/ tree:   {actual}")
    print(f"recorded tree:  {expected}  ({record['repository']} {record['commit']})")
    if actual != expected:
        print("ERROR: rustgl/ differs from the recorded source. Re-sync it from "
              f"{record['repository']} ({record['branch']}) and update renderer-source.json.")
        return 1
    print("OK: rustgl/ matches the recorded renderer source")
    return 0


if __name__ == "__main__":
    sys.exit(main())
