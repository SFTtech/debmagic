import pytest
from debmagic.common.models.package_version import PackageVersion


@pytest.mark.parametrize(
    "version, expected",
    [
        (
            "1.2.3a.4-42.2-14ubuntu2~20.04.1",
            PackageVersion(epoch="0", upstream="1.2.3a.4-42.2", revision="14ubuntu2~20.04.1"),
        ),
        (
            "3:1.2.3a.4-42.2-14ubuntu2~20.04.1",
            PackageVersion(epoch="3", upstream="1.2.3a.4-42.2", revision="14ubuntu2~20.04.1"),
        ),
        ("3:1.2.3a.4ubuntu", PackageVersion(epoch="3", upstream="1.2.3a.4ubuntu", revision="")),
        ("3:1.2.3a-4ubuntu", PackageVersion(epoch="3", upstream="1.2.3a", revision="4ubuntu")),
        ("3:1.2.3a-4ubuntu1", PackageVersion(epoch="3", upstream="1.2.3a", revision="4ubuntu1")),
    ],
)
def test_version_parsing(version: str, expected: PackageVersion):
    parsed_version = PackageVersion.from_str(version)

    assert parsed_version == expected
