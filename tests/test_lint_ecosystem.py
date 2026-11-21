"""Offline tests for scripts/lint_ecosystem.py: parsing and the comparison buckets (no network, no debmagic)."""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))

import lint_ecosystem as eco

SOURCE = eco.SubjectKey("source", "foo", "1.0-1")
BINARY = eco.SubjectKey("binary", "foo-bin", "1.0-1", "amd64")


def oracle(tag: str, info: str = "", tag_type: str = "warning", package: str = "") -> eco.OracleRow:
    return eco.OracleRow(package, "1.0-1" if package else "", "amd64" if package else "", tag_type, tag, info)


def diag(tag: str, context: str = "", severity: str = "warning", code: str = "LN0001") -> eco.Diagnostic:
    return eco.Diagnostic(severity, "foo", "source", tag, code, context)


def compare(oracle_rows, diagnostics, covered=("a-tag",), renames=None, key=SOURCE):
    comparison = eco.Comparison()
    eco.compare_subject(comparison, key, oracle_rows, diagnostics, dict.fromkeys(covered, "LN0001"), renames or {}, 3)
    return comparison


def test_parse_diagnostics_reads_lines_and_ignores_the_rest():
    output = (
        "W: dgit (binary): no-manual-page (LN0152) [usr/bin/x]\n"
        "E: dgit (source): some-tag (LN0009)\n"
        "Found 2 warnings.\n"
        "Finished in 2.1s with 499 rules using 32 threads.\n"
    )
    first, second = eco.parse_diagnostics(output)
    assert (first.severity, first.kind, first.tag, first.code, first.context) == (
        "warning",
        "binary",
        "no-manual-page",
        "LN0152",
        "[usr/bin/x]",
    )
    assert (second.severity, second.context) == ("error", "")


def test_run_summary_tells_failure_from_findings():
    assert eco.FINISHED_LINE.search("E: p (source): t (LN0001)\nFinished in 1s with 3 rules using 2 threads.")
    assert not eco.FINISHED_LINE.search("linting the package failed\n\nCaused by:\n    0: opening x failed")


def test_parse_checksums_and_pool_dir():
    files = eco.parse_checksums(
        "\n aa 10 foo_1.dsc\n bb 20 foo_1.tar.gz", "https://deb.debian.org/debian/pool/main/f/foo"
    )
    assert [(f.name, f.size, f.sha256) for f in files] == [("foo_1.dsc", 10, "aa"), ("foo_1.tar.gz", 20, "bb")]
    assert files[0].url.endswith("/pool/main/f/foo/foo_1.dsc")
    assert eco.pool_dir("main", "libfoo").endswith("/pool/main/libf/libfoo")
    assert eco.pool_dir("contrib", "hello").endswith("/pool/contrib/h/hello")


def test_sql_literal_rejects_anything_but_archive_identifiers():
    assert eco.sql_literal("1:2.0~rc1+dfsg-1") == "'1:2.0~rc1+dfsg-1'"
    with pytest.raises(ValueError):
        eco.sql_literal("x'; drop table sources; --")


def test_match_missed_and_extra():
    matched = compare([oracle("a-tag", "[f]")], [diag("a-tag", "[f]")])
    assert (matched.stats["a-tag"].match, matched.stats["a-tag"].instances_matched) == (1, 1)

    missed = compare([oracle("a-tag", "[f]")], [])
    assert (missed.stats["a-tag"].missed, missed.stats["a-tag"].match) == (1, 0)
    assert missed.stats["a-tag"].examples["missed"]

    extra = compare([], [diag("a-tag", "[f]")])
    assert extra.stats["a-tag"].extra == 1


def test_override_in_oracle_is_not_a_false_positive():
    for hidden in ("overridden", "masked"):
        result = compare([oracle("a-tag", tag_type=hidden)], [diag("a-tag")])
        assert (result.stats["a-tag"].overridden, result.stats["a-tag"].extra) == (1, 0)


def test_uncovered_tags_are_ranked_input_not_misses():
    result = compare([oracle("other", "[x]"), oracle("other", "[y]")], [], covered=("a-tag",))
    assert "other" not in result.stats
    assert (result.uncovered["other"].subjects, result.uncovered["other"].instances) == (1, 2)


def test_classification_free_oracle_and_native_rules_are_ignored():
    result = compare([], [diag("dm-tag", code="DM0001")])
    assert result.native_diagnostics == 1
    assert not result.stats


def test_context_multisets_and_whitespace_normalisation():
    result = compare(
        [oracle("a-tag", "[x]"), oracle("a-tag", "[y]  z")],
        [diag("a-tag", "[x]"), diag("a-tag", "[y z]"), diag("a-tag", "[w]")],
    )
    stats = result.stats["a-tag"]
    assert stats.match == 1
    assert (stats.instances_oracle, stats.instances_debmagic, stats.instances_matched) == (2, 3, 1)
    assert stats.examples["context"]


def test_severity_mismatch_is_recorded_but_experimental_is_not_judged():
    result = compare([oracle("a-tag", tag_type="information")], [diag("a-tag", severity="warning")])
    assert result.stats["a-tag"].severity_mismatch == {("information", "warning"): 1}
    experimental = compare([oracle("a-tag", tag_type="experimental")], [diag("a-tag", severity="warning")])
    assert not experimental.stats["a-tag"].severity_mismatch


def test_renamed_tags_are_folded_onto_the_current_name():
    result = compare([oracle("new-name")], [diag("old-name")], covered=("new-name",), renames={"old-name": "new-name"})
    assert result.stats["new-name"].match == 1


def test_binary_oracle_is_selected_by_package_version_and_arch():
    rows = [
        oracle("a-tag", package="foo-bin"),
        oracle("a-tag", package="other-bin"),
        oracle("src-tag"),
    ]
    assert [r.package for r in eco.subject_oracle(BINARY, rows)] == ["foo-bin"]
    assert [r.tag for r in eco.subject_oracle(SOURCE, rows)] == ["src-tag"]


def test_report_renders_and_counts():
    result = compare(
        [oracle("a-tag", "[f]"), oracle("other")],
        [diag("a-tag", "[f]"), diag("extra-tag")],
        covered=("a-tag", "extra-tag"),
    )
    result.subjects = 1
    text = eco.render(result, {"release": "sid"}, 5)
    assert "| a-tag | LN0001 | 1 |" in text
    assert "Recall 100%, precision 50%" in text
    assert "| other |" in text


# --- package selection ---------------------------------------------------------------------------


def test_default_selection_is_pinned_plus_a_key_sample():
    args = eco.parse_args([])
    assert (args.packages, args.all, args.pool, args.sample) == (None, False, "key", eco.DEFAULT_SAMPLE)
    assert "samba" in eco.DEFAULT_PINNED
    assert (args.perf, args.profile) == ("all", "release")


@pytest.mark.parametrize(
    "argv",
    [["--packages", "hello", "--all"], ["--packages", "hello", "--sample", "3"], ["--all", "--sample", "3"]],
)
def test_conflicting_selections_are_rejected(argv):
    with pytest.raises(SystemExit):
        eco.parse_args(argv)


def test_explicit_sample_and_all_are_respected():
    assert eco.parse_args(["--sample", "7"]).sample == 7
    assert eco.parse_args(["--all"]).all


def test_names_filter_quotes_and_negates():
    assert eco.names_filter(("samba", "hello")) == "s.source in ('samba', 'hello')"
    assert eco.names_filter(("samba",), negate=True) == "s.source not in ('samba')"
    with pytest.raises(ValueError):
        eco.names_filter(["x'--"])


def test_chunks_keep_every_item_in_order():
    assert eco.chunks(list(range(5)), 2) == [[0, 1], [2, 3], [4]]
    assert eco.chunks([], 3) == []


# --- measuring -----------------------------------------------------------------------------------


def test_timed_run_reports_exit_status_output_and_cost():
    run = eco.timed_run(["sh", "-c", "echo out; echo err >&2; exit 3"], timeout=10)
    assert run.returncode == 3
    assert "out" in run.output and "err" in run.output
    assert not run.timed_out
    assert run.metrics.wall > 0 and run.metrics.rss_mb > 0


def test_timed_run_charges_cpu_of_the_whole_process_tree():
    burn = "python3 -c 'sum(range(3_000_000))'"
    run = eco.timed_run(["sh", "-c", f"{burn}; {burn}"], timeout=60)
    assert run.metrics.cpu > 0.05


def test_timed_run_kills_the_process_group_on_timeout():
    run = eco.timed_run(["sh", "-c", "sleep 30 & sleep 30"], timeout=0.3)
    assert run.timed_out
    assert run.metrics.wall < 5


def test_lintian_command_is_restricted_by_tags_file_only_when_given(tmp_path):
    assert eco.lintian_command(Path("a.dsc"), None) == ["lintian", "--no-cfg", "--fail-on", "none", "a.dsc"]
    tags = tmp_path / "t.txt"
    expected = ["lintian", "--no-cfg", "--fail-on", "none", "--tags-from-file", str(tags), "a.dsc"]
    assert eco.lintian_command(Path("a.dsc"), tags) == expected


def test_tag_files_hold_only_tags_this_lintian_knows(tmp_path, monkeypatch):
    monkeypatch.setattr(eco.parity, "load_lintian_tags", lambda _: {"kept-tag": None, "other-tag": None})
    files = eco.lintian_tag_files(
        {"source": {"kept-tag": "LN0001", "removed-tag": "LN0002"}, "binary": {"other-tag": "LN0003"}},
        tmp_path,
        tmp_path,
    )
    assert files["source"].read_text() == "kept-tag\n"
    assert files["binary"].read_text() == "other-tag\n"


def test_median_metrics():
    m = eco.median_metrics([eco.Metrics(1, 2, 10), eco.Metrics(3, 4, 30), eco.Metrics(2, 3, 20)])
    assert (m.wall, m.cpu, m.rss_mb) == (2, 3, 20)


# --- perf report ---------------------------------------------------------------------------------


def perf_record(label, kind, debmagic_wall, tags_wall, full_wall=None):
    runs = {
        eco.TOOL_DEBMAGIC: eco.Metrics(debmagic_wall, debmagic_wall * 2, 50),
        eco.TOOL_LINTIAN_TAGS: eco.Metrics(tags_wall, tags_wall, 100),
    }
    if full_wall is not None:
        runs[eco.TOOL_LINTIAN_FULL] = eco.Metrics(full_wall, full_wall, 130)
    return eco.PerfRecord(label, kind, 1.0, runs)


def test_perf_summary_speedups_are_lintian_over_debmagic():
    summary = eco.perf_summary(
        [perf_record("source a 1", "source", 1.0, 10.0, 20.0), perf_record("source b 1", "source", 1.0, 30.0, 40.0)]
    )
    assert set(summary) == {"source", "all"}
    speedup = summary["source"]["speedup"][eco.TOOL_LINTIAN_TAGS]
    assert speedup["wall_total"] == 20.0  # (10 + 30) / (1 + 1)
    assert speedup["cpu_total"] == 10.0  # debmagic burns 2 CPU-seconds per wall-second here
    assert speedup["per_subject"] == {"median": 20.0, "min": 10.0, "max": 30.0}
    assert summary["source"]["speedup"][eco.TOOL_LINTIAN_FULL]["wall_total"] == 30.0


def test_perf_summary_without_records_is_empty():
    assert eco.perf_summary([]) == {}


def test_perf_section_lists_the_relatively_slowest_subjects_first():
    comparison = eco.Comparison(
        perf=[perf_record("binary fast 1", "binary", 1.0, 50.0), perf_record("binary slow 1", "binary", 2.0, 4.0)],
        perf_failures=[("binary broken 1", eco.TOOL_LINTIAN_FULL, "timed out after 5s")],
    )
    text = "\n".join(eco.render_perf(comparison, eco.perf_summary(comparison.perf), 5))
    assert "debmagic vs lintian (same tags): 18.0x less wall time in total" in text
    assert text.index("binary slow 1") < text.index("binary fast 1")
    assert "binary broken 1 [lintian (default)]: timed out after 5s" in text


def test_report_json_carries_perf():
    comparison = eco.Comparison(perf=[perf_record("source a 1", "source", 1.0, 10.0)])
    perf = eco.report_json(comparison)["perf"]
    assert perf["summary"]["all"]["subjects"] == 1
    assert perf["subjects"][0]["label"] == "source a 1"


def test_run_lintian_treats_any_nonzero_exit_as_failure(monkeypatch):
    monkeypatch.setattr(eco, "lintian_command", lambda subject, tags_file: ["sh", "-c", "echo boom; exit 2"])
    metrics, why = eco.run_lintian(Path("x.dsc"), None, 10)
    assert metrics is None and "boom" in why
    monkeypatch.setattr(eco, "lintian_command", lambda subject, tags_file: ["sh", "-c", "echo 'E: p source: t'"])
    metrics, why = eco.run_lintian(Path("x.dsc"), None, 10)
    assert metrics is not None and not why
