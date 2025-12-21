from pathlib import Path

import pytest
from debmagic.common.package import SourcePackage

asset_base = Path(__file__).parent / "assets"


@pytest.mark.parametrize(
    "debian_folder_path, name",
    [
        (asset_base / "pkg1/debian", "pkg1"),
    ],
)
def test_source_package_parsing(debian_folder_path: Path, name: str):
    package = SourcePackage.from_debian_directory(debian_folder_path)

    assert package.name == name
