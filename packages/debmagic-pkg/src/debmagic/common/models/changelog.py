from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Self

from debian.changelog import Changelog as DebianChangelog

from ..errors import DebmagicError
from ..type_utils import IterableDataSource


class ChangelogFormatError(DebmagicError):
    pass


def _parse_changelog_date(date: str) -> datetime:
    return datetime.strptime(date, "%a, %d %b %Y %H:%M:%S %z")


def _parse_author(author: str | None) -> tuple[str | None, str | None]:
    if author is None:
        return None, None
    if "<" not in author:
        raise ChangelogFormatError()
    name, email = author.split("<")
    return name, email


def _parse_distributions(distributions: str | None) -> list[str]:
    parsed: list[str] = [] if distributions is None else distributions.split(",")
    parsed = list(map(str.strip, parsed))
    return parsed


@dataclass
class ChangelogMetadata:
    urgency: str | None
    binary_only: bool = False


@dataclass
class ChangelogEntry:
    package: str | None
    version: str
    distributions: list[str]
    metadata: ChangelogMetadata
    changes: list[str]
    author_name: str | None
    author_email: str | None
    date: datetime | None


@dataclass
class Changelog:
    entries: list[ChangelogEntry]

    @classmethod
    def from_file(cls, file: IterableDataSource) -> Self:
        changelog = DebianChangelog()
        changelog.parse_changelog(file)
        entries = []
        for block in changelog:
            author_name, author_email = _parse_author(block.author)
            entries.append(
                ChangelogEntry(
                    package=block.package,
                    distributions=_parse_distributions(block.distributions),
                    version=block.version,
                    author_name=author_name,
                    author_email=author_email,
                    changes=block.changes(),
                    date=_parse_changelog_date(block.date) if block.date is not None else None,
                    metadata=ChangelogMetadata(
                        urgency=block.urgency,
                    ),
                )
            )

        return cls(entries=entries)

    @classmethod
    def from_changelog_file(cls, changelog_file_path: Path) -> Self:
        with changelog_file_path.open() as f:
            return cls.from_file(f)
