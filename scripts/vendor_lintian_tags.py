#!/usr/bin/env python3
"""Regenerate the lintian Tag catalogue under `packages/debmagic/src/lint/data/tags/`.

Reads `tags/**/*.tag` of a lintian source tree, the pinned one by default, and
writes three tables that back `lintian_tags.rs` and the override pass:

- `known-tags`: the Tag name of every tag file, prefixed with `<Check>/` when
  `Name-Spaced: yes` (lintian's `Lintian::Profile` known_tags_by_name).
- `renamed-tags`: `old => current` for every `Renamed-From` entry, sorted by old
  name (lintian's known_aliases). An old name that two Tags both claim is an
  error, since lintian would keep whichever file it loads first.
- `show-always`: the names of the Tags with `Show-Always: yes`, sorted
  (lintian's durable_tags_by_name; an override cannot suppress them).

Each table header records the lintian version. The default version is the one
in `packages/debmagic/tests/lintian-parity/PIN`, and the default source tree is
the clone that `scripts/lintian_tag.py` caches for that version, under
`$XDG_CACHE_HOME/debmagic/lintian/<version>` (or `~/.cache/...`).

Usage:
    python3 scripts/vendor_lintian_tags.py
    python3 scripts/vendor_lintian_tags.py --version 2.144.0 \
        --source /path/to/lintian --out-dir /path/to/output
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
PIN_FILE = REPO / "packages" / "debmagic" / "tests" / "lintian-parity" / "PIN"
DEFAULT_OUT_DIR = REPO / "packages" / "debmagic" / "src" / "lint" / "data" / "tags"

KNOWN_FILE = "known-tags"
RENAMED_FILE = "renamed-tags"
SHOW_ALWAYS_FILE = "show-always"


def pinned_version(pin_file: Path = PIN_FILE) -> str:
    """`lintian 2.144.0` on the first line of the PIN file -> `2.144.0`."""
    for line in pin_file.read_text(encoding="utf-8").splitlines():
        match = re.fullmatch(r"lintian\s+(\S+)", line.strip())
        if match:
            return match.group(1)
    raise SystemExit(f"no `lintian <version>` line in {pin_file}")


def default_source(version: str) -> Path:
    cache = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache"))
    return cache / "debmagic" / "lintian" / version


def parse_first_paragraph(path: Path) -> dict[str, str]:
    """The fields of the first Deb822 paragraph of a tag file, keyed lowercase.

    Field names are matched case-insensitively, as `Lintian::Deb822::Section`
    does: `Renamed-from:` is a `Renamed-From` field. Continuation lines
    (leading whitespace) join the value of the field before them.
    """
    fields: dict[str, str] = {}
    current: str | None = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        if not raw.strip():
            if fields:
                break
            continue
        if raw.startswith("#"):
            continue
        if raw[0] in " \t":
            if current is not None:
                fields[current] = f"{fields[current]} {raw.strip()}".strip()
            continue
        key, _, value = raw.partition(":")
        current = key.strip().lower()
        fields[current] = value.strip()
    return fields


def load_tags(source: Path) -> list[dict]:
    """One record per tag file, in path order: the name, aliases and durability."""
    tag_dir = source / "tags"
    records = []
    for path in sorted(tag_dir.rglob("*.tag")):
        fields = parse_first_paragraph(path)
        if "tag" not in fields:
            raise SystemExit(f"no Tag field in {path}")
        name = fields["tag"]
        if fields.get("name-spaced") == "yes":
            name = f"{fields['check']}/{name}"
        records.append(
            {
                "path": path,
                "name": name,
                "renamed_from": fields.get("renamed-from", "").split(),
                "show_always": fields.get("show-always") == "yes",
            }
        )
    return records


def compute_tables(source: Path) -> dict[str, object]:
    """The three tables as Python data, before formatting."""
    records = load_tags(source)
    known = {record["name"] for record in records}

    renamed: dict[str, str] = {}
    for record in records:
        for old in record["renamed_from"]:
            if renamed.get(old, record["name"]) != record["name"]:
                raise SystemExit(f"{old} is renamed from two Tags: {renamed[old]} and {record['name']}")
            renamed[old] = record["name"]

    durable = {record["name"] for record in records if record["show_always"]}
    return {KNOWN_FILE: known, RENAMED_FILE: renamed, SHOW_ALWAYS_FILE: durable}


def render(name: str, tables: dict, version: str) -> str:
    """The text of one table, header included."""
    if name == KNOWN_FILE:
        header = [
            f"# Every Tag name lintian {version} knows: the `Tag` field of each",
            "# tags/**/*.tag, prefixed with `<Check>/` when `Name-Spaced: yes`.",
            f"# Generated from the lintian {version} source; sorted.",
        ]
        body = sorted(tables[name])
    elif name == RENAMED_FILE:
        header = [
            f"# Old Tag names lintian {version} still accepts: each `Renamed-From` entry of",
            "# tags/**/*.tag, as `old-name => current-name`.",
            f"# Generated from the lintian {version} source; sorted by old name.",
        ]
        body = [f"{old} => {current}" for old, current in sorted(tables[name].items())]
    else:
        header = [
            f"# Tags that lintian {version} declares `Show-Always: yes` (`Tag:` of tags/**/*.tag).",
            "# An override cannot suppress them (Lintian::Profile durable_tags_by_name), so",
            "# the override pass ignores their overrides. Sorted.",
        ]
        body = sorted(tables[name])
    return "\n".join(header + ([""] if name != SHOW_ALWAYS_FILE else []) + body) + "\n"


def write_tables(source: Path, out_dir: Path, version: str) -> list[Path]:
    tables = compute_tables(source)
    out_dir.mkdir(parents=True, exist_ok=True)
    written = []
    for name in (KNOWN_FILE, RENAMED_FILE, SHOW_ALWAYS_FILE):
        path = out_dir / name
        path.write_text(render(name, tables, version), encoding="utf-8")
        written.append(path)
    return written


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument(
        "--version",
        help=f"lintian version recorded in the headers (default: the PIN file, {pinned_version()})",
    )
    parser.add_argument(
        "--source",
        type=Path,
        help="lintian source tree with tags/ (default: the cached clone of the version)",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=DEFAULT_OUT_DIR,
        help=f"output directory (default: {DEFAULT_OUT_DIR})",
    )
    args = parser.parse_args(argv)

    version = args.version or pinned_version()
    source = args.source or default_source(version)
    if not (source / "tags").is_dir():
        raise SystemExit(f"no tags/ directory in {source}; clone lintian {version} there first")
    for path in write_tables(source, args.out_dir, version):
        print(path)
    print(f"lintian {version} from {source}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
