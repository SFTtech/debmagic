"""Offline tests for the ecosystem comparison: `O:` parsing, the override buckets, and pinned package lists.

No network, no UDD, no lintian: the oracle rows and debmagic output are synthetic. They pin the rules the
comparison is judged by, so a change to the override pass's output format shows up here first.
"""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))

import lint_ecosystem as eco

KEY = eco.SubjectKey("source", "hello", "2.10-5")


def oracle(tag: str, tag_type: str, information: str = "[debian/control:3]") -> eco.OracleRow:
    return eco.OracleRow("", "", "", tag_type, tag, information)


def diag(
    tag: str, severity: str = "warning", context: str = "[debian/control:3]", code: str = "LN0001"
) -> eco.Diagnostic:
    return eco.Diagnostic(severity, "", "source", tag, code, context)


def judge(
    rows: list[eco.OracleRow], diags: list[eco.Diagnostic], covered: dict[str, str] | None = None
) -> eco.Comparison:
    comparison = eco.Comparison()
    eco.compare_subject(
        comparison,
        KEY,
        rows,
        diags,
        covered if covered is not None else {"a-tag": "LN0001", "b-tag": "LN0002", "c-tag": "LN0003"},
        {},
        example_limit=3,
    )
    return comparison


def test_parse_diagnostics_reads_overridden_lines_and_skips_justification() -> None:
    output = (
        "E: hello (source): a-tag (LN0001) [debian/control:3]\n"
        "N: Justification: this is a known false positive\n"
        "O: hello (source): b-tag (LN0002) [debian/rules]\n"
        "I: hello (source): c-tag (LN0003)\n"
        "\n"
        "Finished in 0.10 seconds\n"
    )
    found = eco.parse_diagnostics(output)
    assert [(d.severity, d.tag) for d in found] == [
        ("error", "a-tag"),
        ("overridden", "b-tag"),
        ("information", "c-tag"),
    ]
    assert found[1].context == "[debian/rules]"


def test_overridden_on_both_sides_is_counted_not_judged() -> None:
    comparison = judge([oracle("a-tag", "overridden")], [diag("a-tag", "overridden", code="LN0001")])
    stats = comparison.stats["a-tag"]
    assert (stats.overridden, stats.match, stats.missed, stats.extra, stats.unapplied) == (1, 0, 0, 0, 0)
    assert not comparison.disagreements


def test_lintian_overrides_but_debmagic_reports_normally_is_unapplied() -> None:
    comparison = judge([oracle("a-tag", "masked")], [diag("a-tag")])
    stats = comparison.stats["a-tag"]
    assert (stats.extra, stats.unapplied, stats.overridden) == (1, 1, 0)
    [record] = comparison.disagreements
    assert (record.bucket, record.note) == ("extra", "override not applied: lintian suppresses this Tag here")


def test_debmagic_overrides_what_lintian_reports_is_missed() -> None:
    comparison = judge([oracle("a-tag", "warning")], [diag("a-tag", "overridden")])
    stats = comparison.stats["a-tag"]
    assert (stats.missed, stats.match, stats.extra) == (1, 0, 0)
    [record] = comparison.disagreements
    assert record.bucket == "missed"
    assert "suppressed" in record.note


def test_hidden_only_by_debmagic_without_lintian_hint_is_an_extra() -> None:
    comparison = judge([], [diag("a-tag", "overridden")])
    stats = comparison.stats["a-tag"]
    assert (stats.extra, stats.unapplied, stats.overridden) == (1, 0, 0)
    assert comparison.disagreements[0].note == "hidden by an override; lintian has no hint"


def test_both_visible_is_a_match_with_context_check() -> None:
    comparison = judge([oracle("a-tag", "warning", "[debian/control:3]")], [diag("a-tag", "warning", "[debian/rules]")])
    stats = comparison.stats["a-tag"]
    assert (stats.match, stats.missed, stats.extra) == (1, 0, 0)
    assert [r.bucket for r in comparison.disagreements] == ["context"]


def test_plain_extra_without_any_override_is_not_unapplied() -> None:
    comparison = judge([], [diag("a-tag")])
    stats = comparison.stats["a-tag"]
    assert (stats.extra, stats.unapplied) == (1, 0)
    assert comparison.disagreements[0].note == ""


def test_uncovered_suppressed_tag_stays_out_of_the_comparison() -> None:
    comparison = judge([oracle("z-tag", "overridden")], [], covered={"a-tag": "LN0001"})
    assert not comparison.stats
    assert not comparison.uncovered


def test_debmagic_overridden_uncovered_tag_is_still_judged() -> None:
    comparison = judge([], [diag("z-tag", "overridden", code="LN9999")], covered={"a-tag": "LN0001"})
    assert comparison.stats["z-tag"].extra == 1


def test_read_package_list_skips_comments_and_keeps_versions(tmp_path: Path) -> None:
    listing = tmp_path / "list.txt"
    listing.write_text("# header\n\nsamba 2:4.25.1+dfsg-1  # pinned\nhello\nhello\n", encoding="utf-8")
    assert eco.read_package_list(listing) == [("samba", "2:4.25.1+dfsg-1"), ("hello", "")]


@pytest.mark.parametrize("body", ["a b c\n", "bad;name 1.0\n", "# only a comment\n"])
def test_read_package_list_refuses_malformed_lines(tmp_path: Path, body: str) -> None:
    listing = tmp_path / "list.txt"
    listing.write_text(body, encoding="utf-8")
    with pytest.raises(SystemExit):
        eco.read_package_list(listing)


def test_pins_filter_mixes_names_and_pairs() -> None:
    sql = eco.pins_filter([("hello", ""), ("samba", "2:4.25.1+dfsg-1")])
    assert sql == "(s.source in ('hello') or (s.source, s.version) in (('samba', '2:4.25.1+dfsg-1')))"


def test_udd_lookups_pin_the_oracle_lintian_version(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    # Without the lintian_version predicate the (source, version) probes cannot use lintian_results_idx.
    sent: list[str] = []
    monkeypatch.setattr(eco, "udd_query", lambda sql: sent.append(sql) or [])
    eco.query_sources("sid", [eco.pins_filter([("samba", "2:4.25.1+dfsg-1")])])
    assert "l.lintian_version = (select max(lintian_version) from lintian_results) and l.source = s.source" in sent[0]
    assert "and ((s.source, s.version) in (('samba', '2:4.25.1+dfsg-1')))" in sent[0]

    entries = [eco.SourceEntry("samba", "2:4.25.1+dfsg-1", "main", [])]
    eco.load_oracle(entries, tmp_path, refresh=True)
    assert "from lintian_results where lintian_version = (select max(lintian_version) from lintian_results)" in sent[1]
    assert "(source, version) in (('samba', '2:4.25.1+dfsg-1'))" in sent[1]


def test_pin_matches_by_version_or_any_version() -> None:
    entry = eco.SourceEntry("samba", "2:4.25.1+dfsg-1", "main", [])
    assert eco.pin_matches(("samba", ""), entry)
    assert eco.pin_matches(("samba", "2:4.25.1+dfsg-1"), entry)
    assert not eco.pin_matches(("samba", "2:4.24.0-1"), entry)


def test_write_package_list_round_trips_with_header(tmp_path: Path) -> None:
    work = tmp_path / "work"
    (work / "oracle").mkdir(parents=True)
    (work / "oracle" / "hello_2.10-5.json").write_text('{"lintian": ["2.144.0"], "rows": []}', encoding="utf-8")
    entries = [eco.SourceEntry("samba", "2:4.25.1+dfsg-1", "main", []), eco.SourceEntry("hello", "2.10-5", "main", [])]
    target = tmp_path / "pinned.txt"
    args = type(
        "Args",
        (),
        {
            "work_dir": work,
            "write_package_list": target,
            "release": "sid",
            "max_source_mb": 30,
            "packages": None,
            "all": False,
            "package_list": None,
            "sample": 2,
            "seed": "20261123",
            "pool": "key",
        },
    )()
    eco.write_package_list(args, entries)
    text = target.read_text(encoding="utf-8")
    assert "oracle lintian 2.144.0" in text
    assert "--sample 2 --seed 20261123 --pool key" in text
    assert eco.read_package_list(target) == [("hello", "2.10-5"), ("samba", "2:4.25.1+dfsg-1")]
