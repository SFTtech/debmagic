#!/usr/bin/env python3
"""Regenerate the iso-codes tables that back the `files/locales` Rules.

Reads `iso_639-3.json` and `iso_639-5.json` from an iso-codes install and
computes the same code sets as lintian's `Lintian::Check::Files::Locales`
(`LOCALE_CODES`, `LOCALE_GROUP_CODES`), then writes three tables under
`packages/debmagic/src/lint/data/files/`:

- `locale-codes`: every ISO 639 code (alpha_2 and alpha_3) of an ISO 639-3
  entry whose type is neither reserved (empty) nor special (`S`).
- `locale-group-codes`: the alpha_3 codes of the ISO 639-5 entries.
- `locale-preferred-codes`: `alpha_3 alpha_2` for every entry of the first
  table that has a two-letter code. lintian's hash value for such a key is
  the two-letter code, which `incorrect-locale-code` reports as the fix.

Each table header records the iso-codes version. The default version is the
upstream part of `dpkg-query -W iso-codes` (epoch and Debian revision
removed), which is 4.20.1 for the vendored tables.

Usage:
    python3 scripts/vendor_iso_codes.py
    python3 scripts/vendor_iso_codes.py --json-dir /path/to/iso-codes/json \
        --version 4.20.1 --out-dir /path/to/output
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_JSON_DIR = Path("/usr/share/iso-codes/json")
DEFAULT_OUT_DIR = REPO / "packages" / "debmagic" / "src" / "lint" / "data" / "files"

# lintian's `$RESERVED` and `$SPECIAL` types, skipped when computing LOCALE_CODES.
RESERVED = ""
SPECIAL = "S"

LOCALE_CODES_FILE = "locale-codes"
GROUP_CODES_FILE = "locale-group-codes"
PREFERRED_CODES_FILE = "locale-preferred-codes"


def host_version() -> str:
    """The upstream iso-codes version of the installed package."""
    out = subprocess.run(
        ["dpkg-query", "-W", "-f", "${Version}", "iso-codes"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    return upstream_version(out)


def upstream_version(debian_version: str) -> str:
    """`1:4.20.1-1` -> `4.20.1`."""
    version = re.sub(r"^\d+:", "", debian_version)
    return version.rsplit("-", 1)[0] if "-" in version else version


def load_entries(json_dir: Path, name: str, key: str) -> list[dict]:
    with (json_dir / name).open(encoding="utf-8") as handle:
        return json.load(handle)[key]


def locale_code_entries(json_dir: Path) -> list[dict]:
    """ISO 639-3 entries lintian's `LOCALE_CODES` is built from."""
    entries = load_entries(json_dir, "iso_639-3.json", "639-3")
    return [e for e in entries if e.get("type", RESERVED) not in (RESERVED, SPECIAL)]


def compute_tables(json_dir: Path) -> dict[str, set[str] | dict[str, str]]:
    """The three tables as Python data, before formatting."""
    codes: set[str] = set()
    preferred: dict[str, str] = {}
    for entry in locale_code_entries(json_dir):
        alpha_3 = entry["alpha_3"]
        codes.add(alpha_3)
        alpha_2 = entry.get("alpha_2", "")
        if alpha_2:
            codes.add(alpha_2)
            preferred[alpha_3] = alpha_2

    group_codes = {entry["alpha_3"] for entry in load_entries(json_dir, "iso_639-5.json", "639-5")}
    return {
        LOCALE_CODES_FILE: codes,
        GROUP_CODES_FILE: group_codes,
        PREFERRED_CODES_FILE: preferred,
    }


def render(name: str, tables: dict, version: str) -> str:
    """The text of one table, header included."""
    if name == LOCALE_CODES_FILE:
        header = [
            f"# ISO 639 codes accepted as a locale directory name, from iso-codes {version}",
            "# (iso_639-3.json: alpha_2 and alpha_3 of every entry except type S;",
            "# the lintian 2.144.0 Check reads these files at run time from /usr/share/iso-codes).",
            "# please alpha sort",
        ]
        body = sorted(tables[name])
    elif name == GROUP_CODES_FILE:
        header = [
            f"# ISO 639-5 language group codes (iso_639-5.json alpha_3), from iso-codes {version}",
            "# please alpha sort",
        ]
        body = sorted(tables[name])
    else:
        header = [
            f"# ISO 639-3 codes with a two-letter code (iso_639-3.json), from iso-codes {version}",
            "# alpha_3 and alpha_2 separated by a space; incorrect-locale-code reports",
            "# the alpha_2 code as the preferred locale directory name",
            "# please alpha sort",
        ]
        body = [f"{three} {two}" for three, two in sorted(tables[name].items())]
    return "\n".join(header + body) + "\n"


def write_tables(json_dir: Path, out_dir: Path, version: str) -> list[Path]:
    tables = compute_tables(json_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    written = []
    for name in tables:
        path = out_dir / name
        path.write_text(render(name, tables, version), encoding="utf-8")
        written.append(path)
    return written


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument(
        "--json-dir",
        type=Path,
        default=DEFAULT_JSON_DIR,
        help=f"iso-codes JSON directory (default: {DEFAULT_JSON_DIR})",
    )
    parser.add_argument(
        "--version",
        help="iso-codes version recorded in the headers (default: the installed package)",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=DEFAULT_OUT_DIR,
        help=f"output directory (default: {DEFAULT_OUT_DIR})",
    )
    args = parser.parse_args(argv)

    version = args.version or host_version()
    for path in write_tables(args.json_dir, args.out_dir, version):
        print(path)
    print(f"iso-codes {version}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
