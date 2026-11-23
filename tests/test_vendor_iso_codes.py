"""Offline tests for scripts/vendor_iso_codes.py: the locale tables from iso-codes JSON."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))

import vendor_iso_codes as vendor


def write_json(json_dir: Path) -> None:
    json_dir.mkdir(parents=True, exist_ok=True)
    iso_639_3 = {
        "639-3": [
            {"alpha_2": "de", "alpha_3": "deu", "name": "German", "scope": "I", "type": "L"},
            {"alpha_2": "id", "alpha_3": "ind", "name": "Indonesian", "scope": "I", "type": "L"},
            {"alpha_3": "aaa", "name": "Ghotuo", "scope": "I", "type": "L"},
            # Special and reserved types are not locale codes.
            {"alpha_2": "qq", "alpha_3": "qqq", "name": "Special", "scope": "S", "type": "S"},
            {"alpha_3": "qqr", "name": "Reserved", "scope": "I", "type": ""},
        ]
    }
    iso_639_5 = {"639-5": [{"alpha_3": "aav", "name": "Austro-Asiatic languages"}]}
    (json_dir / "iso_639-3.json").write_text(json.dumps(iso_639_3), encoding="utf-8")
    (json_dir / "iso_639-5.json").write_text(json.dumps(iso_639_5), encoding="utf-8")


def read_lines(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8").splitlines()


def test_locale_codes_hold_alpha_2_and_alpha_3_sorted(tmp_path):
    json_dir = tmp_path / "json"
    write_json(json_dir)
    vendor.write_tables(json_dir, tmp_path / "out", "4.20.1")

    lines = read_lines(tmp_path / "out" / "locale-codes")
    codes = [line for line in lines if not line.startswith("#")]
    assert codes == ["aaa", "de", "deu", "id", "ind"]
    assert lines[0] == ("# ISO 639 codes accepted as a locale directory name, from iso-codes 4.20.1")


def test_group_codes_hold_iso_639_5_alpha_3(tmp_path):
    json_dir = tmp_path / "json"
    write_json(json_dir)
    vendor.write_tables(json_dir, tmp_path / "out", "4.20.1")

    lines = read_lines(tmp_path / "out" / "locale-group-codes")
    assert lines[0] == "# ISO 639-5 language group codes (iso_639-5.json alpha_3), from iso-codes 4.20.1"
    assert [line for line in lines if not line.startswith("#")] == ["aav"]


def test_preferred_codes_pair_alpha_3_with_alpha_2(tmp_path):
    json_dir = tmp_path / "json"
    write_json(json_dir)
    vendor.write_tables(json_dir, tmp_path / "out", "4.20.1")

    lines = read_lines(tmp_path / "out" / "locale-preferred-codes")
    assert "from iso-codes 4.20.1" in lines[0]
    assert [line for line in lines if not line.startswith("#")] == ["deu de", "ind id"]


def test_version_is_recorded_in_every_header(tmp_path):
    json_dir = tmp_path / "json"
    write_json(json_dir)
    vendor.write_tables(json_dir, tmp_path / "out", "9.9.9")

    for name in ("locale-codes", "locale-group-codes", "locale-preferred-codes"):
        header = [line for line in read_lines(tmp_path / "out" / name) if line.startswith("#")]
        assert any("9.9.9" in line for line in header), name


def test_upstream_version_drops_epoch_and_revision():
    assert vendor.upstream_version("4.20.1-1") == "4.20.1"
    assert vendor.upstream_version("1:4.20.1-1") == "4.20.1"
    assert vendor.upstream_version("4.20.1") == "4.20.1"


def test_main_writes_into_out_dir(tmp_path):
    json_dir = tmp_path / "json"
    write_json(json_dir)
    out_dir = tmp_path / "out"
    assert vendor.main(["--json-dir", str(json_dir), "--version", "4.20.1", "--out-dir", str(out_dir)]) == 0
    assert sorted(path.name for path in out_dir.iterdir()) == [
        "locale-codes",
        "locale-group-codes",
        "locale-preferred-codes",
    ]
