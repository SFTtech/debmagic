from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest
from debmagic.common.models.changelog import Changelog

asset_base = Path(__file__).parent / "assets"


@pytest.mark.parametrize(
    "debian_folder_path, name",
    [
        (asset_base / "pkg1/debian", "pkg1"),
    ],
)
def test_source_package_parsing(debian_folder_path: Path, name: str):
    changelog = Changelog.from_changelog_file(debian_folder_path / "changelog")

    assert changelog.entries[0].package == name
    assert changelog.entries[0].date == datetime(
        year=2025, month=9, day=27, hour=20, minute=55, second=33, tzinfo=timezone(timedelta(hours=2))
    )
    assert changelog.entries[0].distributions == ["trixie"]
