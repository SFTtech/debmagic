"""Offline tests for the tools that orient a lintian-Tag port: the API index, the Rule scaffolder, the API map.

They guard against rot: `docs/develop/lintian-api-map.md` names debmagic APIs, and a rename would leave
agents following a stale map. No network and no lintian clone needed.
"""

import re
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))

import lint_api
import lintian_tag
import new_lint_rule

ROOT = Path(__file__).resolve().parent.parent
LINT_SRC = ROOT / "packages/debmagic/src/lint"
API_MAP = ROOT / "docs/develop/lintian-api-map.md"


def rust_sources() -> str:
    return "\n".join(path.read_text(encoding="utf-8") for path in LINT_SRC.rglob("*.rs"))


def test_api_index_lists_the_core_rule_facing_items(capsys: pytest.CaptureFixture[str]) -> None:
    assert lint_api.main([]) == 0
    index = capsys.readouterr().out
    for expected in (
        "fn data_files(&mut self)",
        "fn control_files(&mut self)",
        "fn debian_control(&self)",
        "fn is_executable_file(&self)",
        "fn satisfies(&self, condition: &str)",
        "struct BinaryPackageTester",
        "## rules/lintian helper modules",
    ):
        assert expected in index, expected


def test_api_index_skips_test_only_items(capsys: pytest.CaptureFixture[str]) -> None:
    lint_api.main(["elf"])
    index = capsys.readouterr().out
    assert "TestElf" not in index
    assert "fn from_bytes" in index


def map_rows() -> list[tuple[str, str]]:
    rows = []
    for line in API_MAP.read_text(encoding="utf-8").splitlines():
        cells = [cell.strip() for cell in re.split(r"(?<!\\)\|", line.strip())[1:-1]]
        if len(cells) == 3 and cells[0] not in {"lintian", "---"} and re.search(r"`[^`]+`", cells[0]):
            rows.append((cells[0], cells[1]))
    return rows


def test_every_row_of_the_api_map_has_a_lintian_name_and_a_debmagic_side() -> None:
    rows = map_rows()
    assert len(rows) > 60
    for lintian, debmagic in rows:
        assert lintian.count("`") == 2, f"one lintian name per row: {lintian}"
        assert debmagic, lintian


def test_api_map_names_only_methods_that_exist() -> None:
    """`ctx.method(`, `Type::function`, `entry.method(`... in the map must be defined in src/lint."""
    source = rust_sources()
    missing = []
    for lintian, debmagic in map_rows():
        for token in re.findall(r"`([^`]+)`", debmagic):
            names = re.findall(r"\b(?:ctx|entry|paragraph|tree|installable)\.(\w+)\(", token)
            names += [name for _type, name in re.findall(r"\b([A-Z]\w+)::(\w+)\b", token) if name[0].islower()]
            names += re.findall(r"\b(?:[a-z_]+)::(\w+)\(", token)
            for name in names:
                if name in {"from_utf8", "read", "len", "get", "kind", "path", "parent", "file_name", "contains"}:
                    continue  # std, or a method also reachable under another receiver
                if not re.search(rf"\bfn {re.escape(name)}\b", source):
                    missing.append(f"{lintian} -> {name}")
    assert not missing, missing


def test_api_map_hooks_match_lintian_check_subs() -> None:
    """The first table covers the hooks `lintian_tag.api_rows` looks for with `^sub NAME`."""
    hooks = {lintian for lintian, _debmagic in map_rows() if lintian.startswith("`sub ")}
    assert {"`sub source`", "`sub installable`"} <= hooks


def test_api_rows_pick_only_the_apis_a_check_uses() -> None:
    check = (
        "sub visit_installed_files {\n  my ($self, $item) = @_;\n  return unless $item->is_file;\n  $item->operm;\n}\n"
    )
    names = {name for name, _debmagic, _notes in lintian_tag.api_rows(check)}
    assert "visit_installed_files" in names
    assert "$item->is_file" in names
    assert "$item->operm" in names
    assert "$item->is_symlink" not in names


@pytest.mark.parametrize(
    ("tag", "expected"),
    [
        ("missing-build-dependency-for-dh_-command", "MissingBuildDependencyForDhCommand"),
        ("init.d-script-missing-start", "InitDScriptMissingStart"),
        ("package-contains-hardlink", "PackageContainsHardlink"),
    ],
)
def test_struct_names_follow_the_existing_convention(tag: str, expected: str) -> None:
    assert lintian_tag.camel(tag) == expected


def test_scaffolded_rule_uses_the_tags_default_selection_and_the_requested_kinds() -> None:
    tag = lintian_tag.report.LintianTag("a-tag", "warning", "files/example", False, [])
    text = new_lint_rule.render_rule(tag, "ATag", "LN9999", ["source", "binary"])
    assert 'code = "LN9999"' in text
    assert "default_selected = true" in text
    assert "impl SourceRule for ATag" in text
    assert "impl BinaryPackageRule for ATag" in text
    assert "Diagnostic::warn" in text
    assert "SourceTreeTester" in text
    assert "BinaryPackageTester" in text
