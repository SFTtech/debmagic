#!/usr/bin/env python3
"""Report how far `debmagic lint` is from lintian Parity.

Reads three things, all offline:

- the lintian tag catalogue (`<lintian-data>/tags/**/*.tag`, default `/usr/share/lintian`):
  the denominator of "which Rules could exist";
- the Rust Rule catalogue (`packages/debmagic/src/lint/rules/`): which Tags are LN Rules,
  with which Applicability (`SourceRule` / `BinaryPackageRule`) and default Severity;
- the vendored Parity oracle (`packages/debmagic/tests/lintian-parity/**/eval/hints` plus the
  generated `tests/lintian_parity.rs`): which expected hint lines exist and which recipes are
  wired into tests.

Optionally (`--run`, or `--test-log FILE`) it includes the pass/fail of the generated Parity tests.

Parity is Tag-set parity on the vendored recipes (ADR 0007, 0012): this report measures that, not
output format, not message text, and not live lintian.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import import_lintian_parity as importer

DEFAULT_LINTIAN_DATA = Path("/usr/share/lintian")
RULE_DIRS = {
    "LN": Path("packages/debmagic/src/lint/rules/lintian"),
    "DM": Path("packages/debmagic/src/lint/rules/native"),
}
PIN_FILE = Path("packages/debmagic/tests/lintian-parity/PIN")
DEFAULT_SELECTED_SEVERITIES = frozenset({"error", "warning"})
SEVERITY_ORDER = ["error", "warning", "info", "pedantic"]

DECLARE_RULE = re.compile(
    r"declare_rule!\s*\{.*?"
    r"(?P<name>\w+),\s*"
    r'code\s*=\s*"(?P<code>[A-Z]{2}\d{4})",\s*'
    r'tag\s*=\s*"(?P<tag>[^"]+)",\s*'
    r"default_selected\s*=\s*(?P<default_selected>true|false),\s*"
    r"experimental\s*=\s*(?P<experimental>true|false),",
    re.DOTALL,
)
IMPL_TRAIT = re.compile(r"impl\s+(SourceRule|BinaryPackageRule)\s+for\s+(\w+)")
SEVERITY_CONSTRUCTOR = re.compile(r"Diagnostic::(error|warn)\(|Severity::(Error|Warning|Info|Pedantic)")
SEVERITY_NAMES = {"error": "error", "warn": "warning", "Error": "error", "Warning": "warning"}
TEST_CASE = re.compile(r'#\[test_case\("(?P<rel>[^"]+)";\s*"(?P<name>[^"]+)"\)\]')
TEST_FN = re.compile(r"fn\s+(?P<fn>\w+)\(")
TEST_RESULT = re.compile(r"^test (?P<module>\w+)::(?P<name>\w+) \.\.\. (?P<status>ok|FAILED|ignored)")

# Processable type on a hint line -> the Rule trait that would have to produce it.
PTYPE_TO_KIND = {"source": "source", "binary": "binary", "udeb": "binary"}
KIND_TO_TEST = {
    "source_tree": "lintian_parity_source_tree",
    "binary_package": "lintian_parity_binary_package",
    "source_package": "lintian_parity_source_package",
}


@dataclass
class LintianTag:
    tag: str
    severity: str
    check: str
    experimental: bool
    renamed_from: list[str]

    @property
    def rule_eligible(self) -> bool:
        """Classification tags are not Rules (ADR 0019)."""
        return self.severity != "classification"

    @property
    def default_selected(self) -> bool:
        return self.severity in DEFAULT_SELECTED_SEVERITIES and not self.experimental


@dataclass
class RustRule:
    code: str
    tag: str
    struct: str
    path: str
    default_selected: bool
    experimental: bool
    kinds: set[str] = field(default_factory=set)
    severities: set[str] = field(default_factory=set)


@dataclass
class Hint:
    recipe: str
    ptype: str
    tag: str
    extra: str


@dataclass
class Recipe:
    rel: str
    form: str
    hints: list[Hint]


# --- lintian tag catalogue ----------------------------------------------------------------------


def parse_tag_file(path: Path) -> LintianTag | None:
    fields: dict[str, list[str]] = defaultdict(list)
    current: str | None = None
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if line[:1] in {" ", "\t"}:
            if current is not None:
                fields[current].append(line.strip())
            continue
        name, sep, value = line.partition(":")
        if not sep:
            continue
        current = name
        if value.strip():
            fields[current].append(value.strip())
    if "Tag" not in fields or "Severity" not in fields:
        return None
    return LintianTag(
        tag=fields["Tag"][0],
        severity=fields["Severity"][0],
        check=(fields.get("Check") or [""])[0],
        experimental=(fields.get("Experimental") or [""])[0] == "yes",
        renamed_from=fields.get("Renamed-From", []),
    )


def load_lintian_tags(data_dir: Path) -> dict[str, LintianTag]:
    tags_dir = data_dir / "tags"
    if not tags_dir.is_dir():
        raise SystemExit(f"no lintian tag catalogue at {tags_dir}; pass --lintian-data")
    tags: dict[str, LintianTag] = {}
    for path in sorted(tags_dir.rglob("*.tag")):
        parsed = parse_tag_file(path)
        if parsed is not None:
            tags[parsed.tag] = parsed
    return tags


def installed_lintian_version() -> str:
    result = subprocess.run(
        ["dpkg-query", "-W", "-f", "${Version}", "lintian"], check=False, capture_output=True, text=True
    )
    return result.stdout.strip() if result.returncode == 0 and result.stdout.strip() else "unknown"


def read_pin(root: Path) -> tuple[str, str]:
    path = root / PIN_FILE
    version, commit = "unknown", "unknown"
    if path.is_file():
        for line in path.read_text(encoding="utf-8").splitlines():
            key, _, value = line.partition(" ")
            if key == "lintian":
                version = value.strip()
            elif key == "commit":
                commit = value.strip()
    return version, commit


# --- Rust Rule catalogue ------------------------------------------------------------------------


def parse_rust_rules(root: Path, prefix: str) -> list[RustRule]:
    rules: list[RustRule] = []
    directory = root / RULE_DIRS[prefix]
    for path in sorted(directory.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        declared = DECLARE_RULE.search(text)
        if declared is None:
            continue
        rule = RustRule(
            code=declared["code"],
            tag=declared["tag"],
            struct=declared["name"],
            path=path.relative_to(root).as_posix(),
            default_selected=declared["default_selected"] == "true",
            experimental=declared["experimental"] == "true",
        )
        for trait, struct in IMPL_TRAIT.findall(text):
            if struct == rule.struct:
                rule.kinds.add("source" if trait == "SourceRule" else "binary")
        # Severity lives on the Diagnostic constructor, not in `declare_rule!`; scan non-test code.
        production = text.split("#[cfg(test)]", maxsplit=1)[0]
        for constructor, severity in SEVERITY_CONSTRUCTOR.findall(production):
            name = constructor or severity
            rule.severities.add(SEVERITY_NAMES.get(name, name.lower()))
        rules.append(rule)
    return rules


# --- Parity oracle ------------------------------------------------------------------------------


def classify_recipe(recipe_dir: Path) -> str:
    if (recipe_dir / "debian").is_dir():
        return "source"
    if importer.recipe_is_deb_skeleton(recipe_dir) or (recipe_dir / "DEBIAN").is_dir():
        return "deb"
    return "other"


def load_recipes(suite: Path) -> list[Recipe]:
    recipes: list[Recipe] = []
    for recipe_dir in importer.discover_vendored_recipe_dirs(suite):
        rel = recipe_dir.relative_to(suite).as_posix()
        hints: list[Hint] = []
        hints_file = recipe_dir / "eval" / "hints"
        if hints_file.is_file():
            text, _encoding = importer.read_text(hints_file)
            for line in text.splitlines():
                parsed = importer.parse_universal_hint_line(line)
                if parsed is not None:
                    ptype, tag, extra = parsed
                    hints.append(Hint(rel, ptype, tag, extra))
        recipes.append(Recipe(rel, classify_recipe(recipe_dir), hints))
    return recipes


def origin_commits(suite: Path) -> dict[str, int]:
    """Lintian commit named by each vendored recipe's ORIGIN, with recipe counts."""
    commits: dict[str, int] = defaultdict(int)
    for recipe_dir in importer.discover_vendored_recipe_dirs(suite):
        first = (recipe_dir / "ORIGIN").read_text(encoding="utf-8", errors="replace").splitlines()[:1]
        match = re.search(r"\(([0-9a-f]{7,40})\)", first[0]) if first else None
        commits[match[1] if match else "unknown"] += 1
    return dict(commits)


def parse_listed_tests(tests_rs: Path) -> dict[str, dict[str, str]]:
    """Generated test kind -> {recipe rel -> test case name}."""
    listed: dict[str, dict[str, str]] = {kind: {} for kind in KIND_TO_TEST}
    if not tests_rs.is_file():
        return listed
    by_fn = {fn: kind for kind, fn in KIND_TO_TEST.items()}
    pending: list[tuple[str, str]] = []
    for line in tests_rs.read_text(encoding="utf-8").splitlines():
        case = TEST_CASE.search(line)
        if case:
            pending.append((case["rel"], case["name"]))
            continue
        function = TEST_FN.search(line)
        if function:
            kind = by_fn.get(function["fn"])
            if kind is not None:
                listed[kind].update(pending)
            pending = []
    return listed


def expected_listed_tests(suite: Path, rules_dir: Path) -> dict[str, list[str]]:
    """What `import_lintian_parity.py --write-tests` would list right now."""
    tags = importer.catalogued_ln_tags(rules_dir)
    return {
        "source_tree": importer.catalogued_source_tree_recipe_rels(suite, tags),
        "binary_package": importer.catalogued_binary_package_recipe_rels(
            suite, importer.catalogued_binary_package_tags(rules_dir)
        ),
        "source_package": importer.catalogued_source_package_recipe_rels(
            suite, importer.catalogued_source_package_tags(rules_dir)
        ),
    }


def run_parity_tests(root: Path) -> str:
    command = ["cargo", "test", "-p", "debmagic", "--test", "lintian_parity"]
    result = subprocess.run(command, cwd=root, check=False, capture_output=True, text=True)
    return result.stdout + result.stderr


def parse_test_log(text: str) -> dict[tuple[str, str], str]:
    """(generated test fn, case name) -> ok | FAILED | ignored."""
    results: dict[tuple[str, str], str] = {}
    for line in text.splitlines():
        match = TEST_RESULT.match(line.strip())
        if match:
            results[(match["module"], match["name"])] = match["status"]
    return results


# --- analysis -----------------------------------------------------------------------------------


@dataclass
class RuleRow:
    rule: RustRule
    lintian: LintianTag | None
    hints_by_kind: dict[str, int]
    hints_unsupported: dict[str, int]
    primary_recipes: int
    all_recipes: int
    listed: dict[str, int]
    status: str
    flags: list[str]


def parity_status(
    rule: RustRule,
    recipe_tests: dict[str, list[tuple[str, str]]],
    results: dict[tuple[str, str], str] | None,
) -> str:
    """Aggregate the generated Parity cases that touch this Rule's Tag."""
    cases = recipe_tests.get(rule.tag, [])
    if not cases:
        return "no-parity-case"
    if results is None:
        return f"{len(cases)} case(s)"
    outcomes = {results.get(case, "missing") for case in cases}
    if "FAILED" in outcomes:
        return "FAIL"
    if outcomes == {"ok"}:
        return "pass"
    return "/".join(sorted(outcomes))


def rule_flags(
    rule: RustRule, lintian: LintianTag | None, unsupported: dict[str, int], tags: dict[str, LintianTag]
) -> list[str]:
    flags: list[str] = []
    if lintian is None:
        renamed_to = next((t.tag for t in tags.values() if rule.tag in t.renamed_from), None)
        flags.append(f"tag-renamed-to:{renamed_to}" if renamed_to else "tag-not-in-lintian")
        return flags
    if lintian.severity == "classification":
        flags.append("tag-is-classification")
    expected_severity = lintian.severity
    if rule.severities and expected_severity not in rule.severities:
        flags.append(f"severity:{'/'.join(sorted(rule.severities))}!={expected_severity}")
    if rule.default_selected != lintian.default_selected:
        flags.append(f"default-selected:{str(rule.default_selected).lower()}!={str(lintian.default_selected).lower()}")
    if rule.experimental != lintian.experimental:
        flags.append("experimental-mismatch")
    for ptype, count in sorted(unsupported.items()):
        flags.append(f"applicability-gap:{ptype}x{count}")
    return flags


def recipe_parent(rel: str) -> str:
    return rel.rsplit("/", maxsplit=1)[0] if "/" in rel else ""


def owner_dirs(hints: list[Hint], lintian: LintianTag | None) -> set[str]:
    """Recipe directories that are this tag's own test home, not incidental appearances.

    The tag's Check (`checks/<check>/`) names it in the catalogue, but Check names drift between lintian
    releases, so the directory where the tag appears most often counts as well.
    """
    counts: dict[str, int] = defaultdict(int)
    for hint in hints:
        counts[recipe_parent(hint.recipe)] += 1
    owners: set[str] = set()
    if counts:
        owners.add(max(sorted(counts), key=lambda directory: counts[directory]))
    if lintian is not None and lintian.check:
        owners.add(f"checks/{lintian.check}")
    return owners


def build_tag_index(recipes: list[Recipe]) -> dict[str, list[Hint]]:
    index: dict[str, list[Hint]] = defaultdict(list)
    for recipe in recipes:
        for hint in recipe.hints:
            index[hint.tag].append(hint)
    return index


def case_key(test_fn: str, name: str) -> tuple[str, str]:
    return (test_fn, name)


def tag_cases(
    listed: dict[str, dict[str, str]],
    recipes: list[Recipe],
    ln_rules: dict[str, RustRule],
) -> dict[str, list[tuple[str, str]]]:
    """Tag -> generated Parity cases whose recipe expects that Tag for that Subject kind."""
    by_rel = {recipe.rel: recipe for recipe in recipes}
    cases: dict[str, list[tuple[str, str]]] = defaultdict(list)
    for kind, recipe_names in listed.items():
        for rel, name in recipe_names.items():
            recipe = by_rel.get(rel)
            if recipe is None:
                continue
            for tag in {hint.tag for hint in recipe.hints if hint.tag in ln_rules and _hint_in_kind(hint, kind)}:
                cases[tag].append(case_key(KIND_TO_TEST[kind], name))
    return cases


def _hint_in_kind(hint: Hint, kind: str) -> bool:
    if kind == "binary_package":
        return hint.ptype in {"binary", "udeb"}
    if kind == "source_tree":
        return importer.applies_to_source_tree(hint.ptype, hint.extra)
    return hint.ptype == "source"


def analyse_rules(
    ln_rules: list[RustRule],
    tags: dict[str, LintianTag],
    recipes: list[Recipe],
    listed: dict[str, dict[str, str]],
    results: dict[tuple[str, str], str] | None,
) -> list[RuleRow]:
    index = build_tag_index(recipes)
    by_tag = {rule.tag: rule for rule in ln_rules}
    cases = tag_cases(listed, recipes, by_tag)
    rows: list[RuleRow] = []
    for rule in ln_rules:
        lintian = tags.get(rule.tag)
        hints = index.get(rule.tag, [])
        by_kind: dict[str, int] = defaultdict(int)
        unsupported: dict[str, int] = defaultdict(int)
        for hint in hints:
            kind = PTYPE_TO_KIND.get(hint.ptype)
            if kind in rule.kinds:
                by_kind[kind] += 1
            else:
                unsupported[hint.ptype] += 1
        owners = owner_dirs(hints, lintian)
        primary = {h.recipe for h in hints if recipe_parent(h.recipe) in owners}
        listed_counts = {
            kind: sum(1 for rel in names if any(h.recipe == rel and h.tag == rule.tag for h in hints))
            for kind, names in listed.items()
        }
        rows.append(
            RuleRow(
                rule=rule,
                lintian=lintian,
                hints_by_kind=dict(by_kind),
                hints_unsupported=dict(unsupported),
                primary_recipes=len(primary),
                all_recipes=len({h.recipe for h in hints}),
                listed=listed_counts,
                status=parity_status(rule, cases, results),
                flags=rule_flags(rule, lintian, dict(unsupported), tags),
            )
        )
    return rows


@dataclass
class Candidate:
    tag: LintianTag
    primary_recipes: int
    all_recipes: int
    hint_lines: int
    ptypes: dict[str, int]


def find_candidates(
    tags: dict[str, LintianTag],
    implemented: set[str],
    recipes: list[Recipe],
    selected_only: bool,
) -> list[Candidate]:
    by_tag: dict[str, list[tuple[Recipe, Hint]]] = defaultdict(list)
    for recipe in recipes:
        for hint in recipe.hints:
            by_tag[hint.tag].append((recipe, hint))
    candidates: list[Candidate] = []
    for tag, lintian in tags.items():
        if tag in implemented or not lintian.rule_eligible:
            continue
        if selected_only and not lintian.default_selected:
            continue
        pairs = by_tag.get(tag, [])
        if not pairs:
            continue
        ptypes: dict[str, int] = defaultdict(int)
        for _recipe, hint in pairs:
            ptypes[hint.ptype] += 1
        owners = owner_dirs([hint for _, hint in pairs], lintian)
        candidates.append(
            Candidate(
                tag=lintian,
                primary_recipes=len({r.rel for r, _ in pairs if recipe_parent(r.rel) in owners}),
                all_recipes=len({r.rel for r, _ in pairs}),
                hint_lines=len(pairs),
                ptypes=dict(ptypes),
            )
        )
    candidates.sort(key=lambda c: (-c.primary_recipes, -c.hint_lines, c.tag.tag))
    return candidates


def pct(part: int, whole: int) -> str:
    return f"{(100 * part / whole):.1f}%" if whole else "-"


def oracle_totals(recipes: list[Recipe], tags: dict[str, LintianTag], ln_rules: list[RustRule]) -> dict[str, int]:
    by_tag = {rule.tag: rule for rule in ln_rules}
    totals = {
        "recipes": len(recipes),
        "lines": 0,
        "rule_eligible_lines": 0,
        "default_selected_lines": 0,
        "covered_lines": 0,
    }
    for recipe in recipes:
        for hint in recipe.hints:
            totals["lines"] += 1
            lintian = tags.get(hint.tag)
            if lintian is None or not lintian.rule_eligible:
                continue
            totals["rule_eligible_lines"] += 1
            if lintian.default_selected:
                totals["default_selected_lines"] += 1
            rule = by_tag.get(hint.tag)
            if rule is not None and PTYPE_TO_KIND.get(hint.ptype) in rule.kinds:
                totals["covered_lines"] += 1
    totals["recipes_with_covered"] = sum(
        1
        for recipe in recipes
        if any(
            (rule := by_tag.get(h.tag)) is not None and PTYPE_TO_KIND.get(h.ptype) in rule.kinds for h in recipe.hints
        )
    )
    return totals


def tag_totals(tags: dict[str, LintianTag], ln_tags: set[str]) -> dict[str, dict[str, int]]:
    rows: dict[str, dict[str, int]] = {}
    eligible = [t for t in tags.values() if t.rule_eligible]
    groups: dict[str, list[LintianTag]] = {
        "all": eligible,
        "default-selected": [t for t in eligible if t.default_selected],
    }
    for severity in SEVERITY_ORDER:
        groups[severity] = [t for t in eligible if t.severity == severity]
    for name, members in groups.items():
        rows[name] = {"total": len(members), "implemented": sum(1 for t in members if t.tag in ln_tags)}
    return rows


def check_totals(tags: dict[str, LintianTag], ln_tags: set[str]) -> list[tuple[str, int, int]]:
    per_check: dict[str, list[LintianTag]] = defaultdict(list)
    for tag in tags.values():
        if tag.rule_eligible:
            per_check[tag.check].append(tag)
    rows = [(check, len(members), sum(1 for t in members if t.tag in ln_tags)) for check, members in per_check.items()]
    rows.sort(key=lambda row: (-row[2], -row[1], row[0]))
    return rows


# --- rendering ----------------------------------------------------------------------------------


def table(headers: list[str], rows: list[list[str]]) -> str:
    widths = [max(len(str(cell)) for cell in column) for column in zip(headers, *rows, strict=True)]
    lines = ["  ".join(header.ljust(width) for header, width in zip(headers, widths, strict=True)).rstrip()]
    for row in rows:
        lines.append("  ".join(str(cell).ljust(width) for cell, width in zip(row, widths, strict=True)).rstrip())
    return "\n".join(lines)


def render_header(report: dict) -> list[str]:
    meta = report["meta"]
    lines = [
        "Lintian Parity report",
        f"  Parity oracle (vendored): lintian {meta['pin_version']} ({meta['pin_commit'][:8]})",
        f"  Tag catalogue:            {meta['lintian_data']} (lintian {meta['catalogue_version']})",
    ]
    if meta["catalogue_version"].split("+")[0].split("-")[0] != meta["pin_version"]:
        lines.append("  note: the tag catalogue differs from the pinned oracle; tag/severity drift is possible.")
    lines.append(
        f"  vendored recipes: {meta['recipes']}  expected hint lines: {meta['hint_lines']}"
        f"  LN Rules: {meta['ln_rules']}  DM Rules: {meta['dm_rules']}"
    )
    return lines


def render_coverage(report: dict) -> list[str]:
    lines = ["", "== Rule coverage (lintian tags that could be Rules; classification tags excluded, ADR 0019)"]
    rows = [
        [name, str(v["implemented"]), str(v["total"]), pct(v["implemented"], v["total"])]
        for name, v in report["tag_coverage"].items()
    ]
    lines.append(table(["tags", "LN Rules", "lintian", "%"], rows))
    oracle = report["oracle"]
    lines += ["", "== Oracle coverage (expected hint lines in the vendored lintian test-suite)"]
    lines.append(
        table(
            ["measure", "lines", "% of rule-eligible"],
            [
                ["all hint lines", str(oracle["lines"]), ""],
                ["rule-eligible tags", str(oracle["rule_eligible_lines"]), "100.0%"],
                [
                    "  of default-selected tags",
                    str(oracle["default_selected_lines"]),
                    pct(oracle["default_selected_lines"], oracle["rule_eligible_lines"]),
                ],
                [
                    "  produced by an LN Rule (Applicability matches)",
                    str(oracle["covered_lines"]),
                    pct(oracle["covered_lines"], oracle["rule_eligible_lines"]),
                ],
            ],
        )
    )
    lines.append(
        f"  recipes with at least one covered hint line: {oracle['recipes_with_covered']} of {oracle['recipes']}"
    )
    return lines


def render_rules(report: dict) -> list[str]:
    rows = []
    for item in report["rules"]:
        hints = item["hint_lines"]
        rows.append(
            [
                item["code"],
                item["tag"],
                item["lintian_severity"] or "?",
                "+".join(item["kinds"]) or "-",
                f"{hints['covered']}/{hints['covered'] + hints['uncovered']}",
                f"{item['primary_recipes']}/{item['all_recipes']}",
                "/".join(str(item["listed"][k]) for k in ("source_tree", "source_package", "binary_package")),
                item["status"],
                " ".join(item["flags"]) or "-",
            ]
        )
    lines = ["", "== Implemented LN Rules"]
    lines.append(
        table(
            [
                "code",
                "tag",
                "lintian",
                "applies",
                "hints ok/all",
                "recipes prim/all",
                "parity tree/dsc/deb",
                "status",
                "flags",
            ],
            rows,
        )
    )
    return lines


def render_checks(report: dict) -> list[str]:
    checks = report["checks"]
    touched = [c for c in checks if c["implemented"] > 0]
    lines = ["", f"== Lintian Checks touched ({len(touched)} of {len(checks)} Checks have an LN Rule)"]
    lines.append(
        table(
            ["check", "tags", "LN Rules", "%"],
            [[c["check"], str(c["total"]), str(c["implemented"]), pct(c["implemented"], c["total"])] for c in touched],
        )
    )
    return lines


def render_fidelity(report: dict) -> list[str]:
    lines = ["", "== Fidelity & infrastructure"]
    issues = report["issues"]
    if not issues:
        lines.append("  no issues")
    lines.extend(f"  - {issue}" for issue in issues)
    stale = report["stale_generated_tests"]
    freshness = "STALE (run import_lintian_parity.py --write-tests)" if stale else "up to date"
    lines.append(f"  generated tests/lintian_parity.rs is {freshness}")
    lines.append(
        "  Fixes: debmagic lint has no fix mechanism yet; lintian has none either (lintian-brush is separate)."
    )
    return lines


def render_candidates(report: dict) -> list[str]:
    candidates = report["candidates"]
    if not candidates:
        return []
    lines = [
        "",
        f"== Next-Rule candidates (top {len(candidates)}; default-selected, uncatalogued, with oracle recipes)",
    ]
    rows = [
        [
            c["tag"],
            c["severity"],
            c["check"],
            str(c["primary_recipes"]),
            str(c["all_recipes"]),
            str(c["hint_lines"]),
            ",".join(f"{k}:{v}" for k, v in sorted(c["ptypes"].items())),
        ]
        for c in candidates
    ]
    lines.append(table(["tag", "sev", "check", "prim", "recipes", "lines", "hint types"], rows))
    lines.append("  prim = recipes in the tag's own test directory; the remaining recipes hit the tag incidentally.")
    return lines


def render_text(report: dict) -> str:
    sections = [
        render_header(report),
        render_coverage(report),
        render_rules(report),
        render_checks(report),
        render_fidelity(report),
        render_candidates(report),
    ]
    return "\n".join(line for section in sections for line in section) + "\n"


# --- driver -------------------------------------------------------------------------------------


def build_report(args: argparse.Namespace) -> dict:
    root = importer.repo_root()
    suite = root / importer.SUITE_RELATIVE
    tags = load_lintian_tags(args.lintian_data)
    ln_rules = parse_rust_rules(root, "LN")
    dm_rules = parse_rust_rules(root, "DM")
    recipes = load_recipes(suite)
    pin_version, pin_commit = read_pin(root)
    listed = parse_listed_tests(root / importer.GENERATED_TESTS_RELATIVE)

    results: dict[tuple[str, str], str] | None = None
    if args.run:
        results = parse_test_log(run_parity_tests(root))
    elif args.test_log:
        results = parse_test_log(args.test_log.read_text(encoding="utf-8"))

    rows = analyse_rules(ln_rules, tags, recipes, listed, results)
    ln_tags = {rule.tag for rule in ln_rules}
    expected = expected_listed_tests(suite, root / importer.RULES_RELATIVE)
    stale = any(sorted(listed[kind]) != sorted(expected[kind]) for kind in KIND_TO_TEST)

    report: dict = {
        "meta": {
            "pin_version": pin_version,
            "pin_commit": pin_commit,
            "lintian_data": str(args.lintian_data),
            "catalogue_version": installed_lintian_version()
            if args.lintian_data == DEFAULT_LINTIAN_DATA
            else "unknown",
            "recipes": len(recipes),
            "hint_lines": sum(len(r.hints) for r in recipes),
            "ln_rules": len(ln_rules),
            "dm_rules": len(dm_rules),
            "origin_commits": origin_commits(suite),
        },
        "tag_coverage": tag_totals(tags, ln_tags),
        "oracle": oracle_totals(recipes, tags, ln_rules),
        "rules": [rule_json(row) for row in rows],
        "checks": [
            {"check": check, "total": total, "implemented": implemented}
            for check, total, implemented in check_totals(tags, ln_tags)
        ],
        "stale_generated_tests": stale,
        "candidates": [
            candidate_json(c)
            for c in find_candidates(tags, ln_tags, recipes, not args.all_severities)[: args.candidates]
        ],
    }
    report["issues"] = collect_issues(rows, stale, results) + provenance_issues(report["meta"])
    return report


def rule_json(row: RuleRow) -> dict:
    covered = sum(row.hints_by_kind.values())
    return {
        "code": row.rule.code,
        "tag": row.rule.tag,
        "file": row.rule.path,
        "kinds": sorted(row.rule.kinds),
        "lintian_severity": row.lintian.severity if row.lintian else None,
        "default_selected": row.rule.default_selected,
        "hint_lines": {"covered": covered, "uncovered": sum(row.hints_unsupported.values())},
        "uncovered_by_type": row.hints_unsupported,
        "primary_recipes": row.primary_recipes,
        "all_recipes": row.all_recipes,
        "listed": row.listed,
        "status": row.status,
        "flags": row.flags,
    }


def candidate_json(candidate: Candidate) -> dict:
    return {
        "tag": candidate.tag.tag,
        "severity": candidate.tag.severity,
        "check": candidate.tag.check,
        "primary_recipes": candidate.primary_recipes,
        "all_recipes": candidate.all_recipes,
        "hint_lines": candidate.hint_lines,
        "ptypes": candidate.ptypes,
    }


def collect_issues(rows: list[RuleRow], stale: bool, results: dict[tuple[str, str], str] | None) -> list[str]:
    issues = [f"{row.rule.code} {row.rule.tag}: {flag}" for row in rows for flag in row.flags]
    issues += [
        f"{row.rule.code} {row.rule.tag}: no Parity oracle recipes in vendored suite"
        for row in rows
        if row.all_recipes == 0
    ]
    issues += [
        f"{row.rule.code} {row.rule.tag}: no Parity case exercises its Applicability"
        for row in rows
        if row.status == "no-parity-case" and row.all_recipes > 0
    ]
    issues += [f"{row.rule.code} {row.rule.tag}: Parity FAIL" for row in rows if row.status == "FAIL"]
    if stale:
        issues.append("tests/lintian_parity.rs does not match vendored recipes + catalogued Rules")
    if results is not None and not results:
        issues.append("no Parity test results parsed from the cargo test output")
    return issues


def provenance_issues(meta: dict) -> list[str]:
    commits: dict[str, int] = meta["origin_commits"]
    pin = meta["pin_commit"]
    issues = []
    if len(commits) > 1:
        listing = ", ".join(
            f"{commit[:8]} x{count}" for commit, count in sorted(commits.items(), key=lambda kv: -kv[1])
        )
        issues.append(f"vendored recipes come from {len(commits)} lintian commits ({listing}); PIN says {pin[:8]}")
    elif commits and pin not in commits:
        issues.append(f"vendored recipes come from {next(iter(commits))[:8]} but PIN says {pin[:8]}")
    return issues


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--lintian-data", type=Path, default=DEFAULT_LINTIAN_DATA, help="lintian data dir with tags/")
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    parser.add_argument("--run", action="store_true", help="run `cargo test --test lintian_parity` and include results")
    parser.add_argument("--test-log", type=Path, help="include results from a saved `cargo test` log")
    parser.add_argument("--candidates", type=int, default=0, metavar="N", help="list the top N next-Rule candidates")
    parser.add_argument(
        "--all-severities",
        action="store_true",
        help="candidates: include info/pedantic/experimental tags (default: default-selected only)",
    )
    parser.add_argument("--strict", action="store_true", help="exit 1 when the report lists any issue")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    report = build_report(args)
    sys.stdout.write(json.dumps(report, indent=2) + "\n" if args.json else render_text(report))
    return 1 if args.strict and report["issues"] else 0


if __name__ == "__main__":
    sys.exit(main())
