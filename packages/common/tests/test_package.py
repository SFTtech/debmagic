from pathlib import Path

import pytest
from debmagic.common.package import SourcePackage

asset_base = Path(__file__).parent / "assets"


@pytest.mark.parametrize(
    "control_file_path, name",
    [
        (asset_base / "pkg1_minimal_control_file", "pkg1"),
    ],
)
def test_source_package_parsing(control_file_path: Path, name: str):
    package = SourcePackage.from_control_file(control_file_path)

    assert package.name == name
