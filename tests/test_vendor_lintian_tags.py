"""Tests for scripts/vendor_lintian_tags.py: the Tag catalogue from tags/**/*.tag."""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))

import vendor_lintian_tags as vendor

REPO = Path(__file__).resolve().parent.parent
TAG_DATA = REPO / "packages" / "debmagic" / "src" / "lint" / "data" / "tags"


def write_tag(root: Path, relative: str, text: str) -> None:
    path = root / "tags" / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


@pytest.fixture
def source(tmp_path):
    write_tag(
        tmp_path,
        "c/cute-field.tag",
        "Tag: cute-field\nSeverity: warning\nCheck: fields\nShow-Always: yes\nExplanation: text\n",
    )
    # Field names are case-insensitive; the list continues on the next line.
    write_tag(
        tmp_path,
        "n/new-name.tag",
        "Tag: new-name\nSeverity: warning\nCheck: fields\nRenamed-from:\n old-one\n old-two\nExplanation: text\n",
    )
    write_tag(
        tmp_path,
        "t/team/pkg-perl/vcs/no-git.tag",
        "Tag: no-git\nSeverity: warning\nCheck: team/pkg-perl/vcs\nName-Spaced: yes\nExplanation: text\n",
    )
    return tmp_path


def test_known_tags_prefix_name_spaced_tags(source):
    known = vendor.compute_tables(source)[vendor.KNOWN_FILE]
    assert known == {"cute-field", "new-name", "team/pkg-perl/vcs/no-git"}


def test_renamed_from_is_a_case_insensitive_list(source):
    renamed = vendor.compute_tables(source)[vendor.RENAMED_FILE]
    assert renamed == {"old-one": "new-name", "old-two": "new-name"}


def test_show_always_holds_only_flagged_tags(source):
    durable = vendor.compute_tables(source)[vendor.SHOW_ALWAYS_FILE]
    assert durable == {"cute-field"}


def test_renamed_from_two_tags_is_an_error(source):
    write_tag(source, "o/other.tag", "Tag: other\nCheck: fields\nRenamed-from: old-one\n")
    with pytest.raises(SystemExit, match="old-one is renamed from two Tags"):
        vendor.compute_tables(source)


def test_rendered_format_matches_vendored_headers(source):
    tables = vendor.compute_tables(source)
    known = vendor.render(vendor.KNOWN_FILE, tables, "2.144.0").splitlines()
    assert known[:4] == [
        "# Every Tag name lintian 2.144.0 knows: the `Tag` field of each",
        "# tags/**/*.tag, prefixed with `<Check>/` when `Name-Spaced: yes`.",
        "# Generated from the lintian 2.144.0 source; sorted.",
        "",
    ]
    assert known[4:] == ["cute-field", "new-name", "team/pkg-perl/vcs/no-git"]

    renamed = vendor.render(vendor.RENAMED_FILE, tables, "2.144.0").splitlines()
    assert renamed[-2:] == ["old-one => new-name", "old-two => new-name"]

    durable = vendor.render(vendor.SHOW_ALWAYS_FILE, tables, "2.144.0")
    assert durable.splitlines()[-1] == "cute-field"
    assert durable.endswith("\n") and not durable.endswith("\n\n")


def test_pinned_version_is_read_from_the_pin_file(tmp_path):
    pin = tmp_path / "PIN"
    pin.write_text("lintian 2.144.0\ncommit abc\n", encoding="utf-8")
    assert vendor.pinned_version(pin) == "2.144.0"


def test_vendored_tables_are_the_pinned_clone_output():
    """The committed tables are what the script writes for the pinned clone."""
    version = vendor.pinned_version()
    clone = vendor.default_source(version)
    if not (clone / "tags").is_dir():
        pytest.skip(f"lintian {version} is not cloned at {clone}")
    tables = vendor.compute_tables(clone)
    for name in (vendor.KNOWN_FILE, vendor.RENAMED_FILE, vendor.SHOW_ALWAYS_FILE):
        assert (TAG_DATA / name).read_text(encoding="utf-8") == vendor.render(name, tables, version), (
            f"{name} is stale: run scripts/vendor_lintian_tags.py"
        )
