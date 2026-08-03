#!/usr/bin/env python3
"""Generate debian/changelog from package Keep-a-Changelog files during release bumps."""

from __future__ import annotations

import email.utils
import os
import re
import sys
from datetime import datetime
from pathlib import Path

PACKAGE_CHANGELOGS = (
    "packages/debmagic/CHANGELOG.md",
    "packages/debmagic-pkg/CHANGELOG.md",
)

SOURCE_PACKAGE = "debmagic"
DISTRIBUTION = "forky"
MAINTAINER = "Debmagic Maintainers <debmagic@sft.lol>"

PRE_RELEASE_PATTERN = re.compile(r"^(?P<base>\d+\.\d+\.\d+)-(?P<label>alpha|beta|rc)\.(?P<number>\d+)$")
VERSION_SECTION_PATTERN = re.compile(r"^## \[(?P<version>[^\]]+)\]")
LIST_ITEM_PATTERN = re.compile(r"^[\-*]\s+(?P<text>.+)$")


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def to_debian_version(version: str) -> str:
    """Convert canonical semver/PEP440 version to Debian upstream version."""
    match = PRE_RELEASE_PATTERN.match(version)
    if match:
        return f"{match.group('base')}~{match.group('label')}{match.group('number')}"
    if re.fullmatch(r"\d+\.\d+\.\d+", version):
        return version
    raise ValueError(f"unsupported version format: {version!r}")


def extract_bullets(changelog_text: str, version: str) -> list[str]:
    """Return list items from the Keep-a-Changelog section for *version*."""
    in_section = False
    bullets: list[str] = []

    for line in changelog_text.splitlines():
        section_match = VERSION_SECTION_PATTERN.match(line)
        if section_match:
            in_section = section_match.group("version") == version
            continue

        if not in_section:
            continue

        if line.startswith("## "):
            break

        item_match = LIST_ITEM_PATTERN.match(line)
        if item_match:
            bullets.append(item_match.group("text").strip())

    return bullets


def merge_bullets(changelog_paths: list[Path], version: str) -> list[str]:
    """Flat-merge bullets from package changelogs, deduping exact duplicates."""
    merged: list[str] = []
    seen: set[str] = set()

    for path in changelog_paths:
        text = path.read_text(encoding="utf-8")
        for bullet in extract_bullets(text, version):
            if bullet not in seen:
                seen.add(bullet)
                merged.append(bullet)

    return merged


def resolve_when(now_str: str | None) -> datetime:
    """Return a timezone-aware local datetime for the changelog trailer.

    bump-my-version sets BVHOOK_NOW via naive ``datetime.now().isoformat()``.
    Naive values are treated as local wall time; aware values are converted to local.
    """
    when = datetime.fromisoformat(now_str) if now_str else datetime.now()
    return when.astimezone()


def format_rfc2822_date(when: datetime) -> str:
    if when.tzinfo is None:
        raise ValueError("changelog date must be timezone-aware")
    return email.utils.format_datetime(when, usegmt=False)


def render_entry(
    *,
    debian_version: str,
    bullets: list[str],
    when: datetime,
) -> str:
    body_lines = [f"  * {bullet}" for bullet in bullets]
    body = "\n".join(body_lines)
    date = format_rfc2822_date(when)

    return f"{SOURCE_PACKAGE} ({debian_version}) {DISTRIBUTION}; urgency=medium\n\n{body}\n\n -- {MAINTAINER}  {date}\n"


def prepend_changelog(changelog_path: Path, entry: str) -> None:
    existing = changelog_path.read_text(encoding="utf-8") if changelog_path.exists() else ""
    # Debian policy: blank line between consecutive changelog entries.
    text = f"{entry}\n{existing}" if existing else entry
    changelog_path.write_text(text, encoding="utf-8")


def main() -> int:
    version = os.environ.get("BVHOOK_NEW_VERSION")
    if not version:
        print("BVHOOK_NEW_VERSION is required", file=sys.stderr)
        return 1

    root = repo_root()
    debian_changelog = root / "debian" / "changelog"
    changelog_paths = [root / path for path in PACKAGE_CHANGELOGS]

    try:
        debian_version = to_debian_version(version)
        bullets = merge_bullets(changelog_paths, version)
        if not bullets:
            raise RuntimeError(
                f"no changelog bullets found for version {version!r}; "
                "add notes under [Unreleased] in package changelogs before bumping"
            )

        when = resolve_when(os.environ.get("BVHOOK_NOW"))

        entry = render_entry(
            debian_version=debian_version,
            bullets=bullets,
            when=when,
        )
        prepend_changelog(debian_changelog, entry)
    except (RuntimeError, ValueError) as exc:
        print(str(exc), file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
