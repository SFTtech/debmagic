"""Offline tests for scripts/import_lintian_parity.py: how Deb822 config files are read."""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))

import import_lintian_parity as importer


def read(tmp_path: Path, text: str) -> dict[str, str]:
    path = tmp_path / "fill-values"
    path.write_text(text, encoding="utf-8")
    return importer.read_deb822(path)


def test_read_deb822_keeps_continuation_lines(tmp_path):
    fields = read(
        tmp_path,
        "Testname: template-description\n"
        "Description: A short synopsis\n"
        " the first long line\n"
        "  an indented line\n"
        " .\n"
        " the last long line\n"
        "Author: Someone\n",
    )

    assert (
        fields["Description"] == "A short synopsis\n the first long line\n  an indented line\n .\n the last long line"
    )
    assert fields["Author"] == "Someone"


def test_read_deb822_trims_like_lintian(tmp_path):
    fields = read(
        tmp_path,
        "Empty:\nStarts-On-Next-Line:   \n value  \n more\t\nTrailing:   spaced out   \n",
    )

    assert fields["Empty"] == ""
    assert fields["Starts-On-Next-Line"] == "value\n more"
    assert fields["Trailing"] == "spaced out"


def test_read_deb822_skips_comment_and_blank_lines_but_not_indented_hashes(tmp_path):
    fields = read(
        tmp_path,
        "# a comment\nDescription: first\n# another comment\n # an indented line, kept\n\nNext: x\n",
    )

    assert fields["Description"] == "first\n # an indented line, kept"
    assert fields["Next"] == "x"


def test_read_deb822_rejects_a_continuation_without_a_field(tmp_path):
    with pytest.raises(ValueError, match="continuation without a field"):
        read(tmp_path, " orphan\n")


def test_wrapped_value_fills_a_template_with_its_newlines(tmp_path):
    fields = read(tmp_path, "Description: short\n long\n")

    filled = importer.fill_text("Description: [% $description %]\n", importer.to_hash(fields))

    assert filled == "Description: short\n long\n"
