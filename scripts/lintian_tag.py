"""Everything deterministic about one lintian Tag, for porting it to an LN Rule.

One command instead of exploring the lintian source and the Parity suite by hand:

- the `.tag` file and what follows from it (default Selection, Diagnostic constructor);
- catalogue state: the LN Code if catalogued, else the next free Code and the file to create;
- the Check module (whole when short, otherwise the sub index and every emission site);
- the lintian API the Check uses, mapped to debmagic (`docs/develop/lintian-api-map.md`);
- the `data/` tables it loads and whether they are vendored;
- vendored recipes whose `eval/hints` name the Tag, with their hint lines, and the Subjects
  (Rule traits) those imply; recipes lintian has that are not vendored;
- the other Tags of the same Check and their state;
- the exact native and Parity test commands.

The lintian source is the pinned version (`tests/lintian-parity/PIN`), cloned once into
`~/.cache/debmagic/lintian/<version>` (or `--lintian-src`, or `$DEBMAGIC_LINTIAN_SRC`).

    just lintian-tag statically-linked-binary
    just lintian-tag statically-linked-binary --full     # whole Check module
    just lintian-tag statically-linked-binary --run      # also run the Tag's Parity cases
"""

from __future__ import annotations

import argparse
import difflib
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import import_lintian_parity as importer
import lint_parity_report as report

ROOT = Path(__file__).resolve().parent.parent
SUITE = ROOT / "packages/debmagic/tests/lintian-parity"
TESTS_RS = ROOT / "packages/debmagic/tests/lintian_parity.rs"
RULES = ROOT / "packages/debmagic/src/lint/rules/lintian"
DATA = ROOT / "packages/debmagic/src/lint/data"
API_MAP = ROOT / "docs/develop/lintian-api-map.md"
SHORT_CHECK_LINES = 250
EMISSION_CONTEXT = 30


# --- lintian source -----------------------------------------------------------------------------


def lintian_source(explicit: Path | None) -> Path:
    version, commit = report.read_pin(ROOT)
    if explicit is None and os.environ.get("DEBMAGIC_LINTIAN_SRC"):
        explicit = Path(os.environ["DEBMAGIC_LINTIAN_SRC"])
    if explicit is not None:
        return explicit.resolve()
    cache = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "debmagic/lintian" / version
    if not (cache / "lib/Lintian").is_dir():
        cache.parent.mkdir(parents=True, exist_ok=True)
        print(f"cloning lintian {version} into {cache} (once)", file=sys.stderr)
        importer.git_clone(version, cache)
    head = importer.git_head(cache) if (cache / ".git").exists() else "unknown"
    if commit != "unknown" and not head.startswith(commit[:12]):
        print(f"warning: {cache} is at {head[:12]}, PIN says {commit[:12]}", file=sys.stderr)
    return cache


def find_tag_file(lintian: Path, tag: str) -> Path | None:
    matches = list((lintian / "tags").glob(f"*/{tag}.tag")) or list((lintian / "tags").rglob(f"{tag}.tag"))
    return matches[0] if matches else None


def check_module(lintian: Path, check: str) -> Path | None:
    parts = ["".join(word[:1].upper() + word[1:] for word in part.split("-")) for part in check.split("/")]
    path = lintian / "lib/Lintian/Check" / ("/".join(parts) + ".pm")
    if path.is_file():
        return path
    package = "Lintian::Check::" + "::".join(parts)
    for candidate in (lintian / "lib/Lintian/Check").rglob("*.pm"):
        text = candidate.read_text(encoding="utf-8", errors="replace")
        if re.search(rf"^package {re.escape(package)};", text, re.MULTILINE | re.IGNORECASE):
            return candidate
    return None


def strip_perl(text: str) -> list[str]:
    """The module without its license header and trailing POD/editor settings."""
    lines = text.splitlines()
    start = next((i for i, line in enumerate(lines) if line.startswith("package ")), 0)
    end = next((i for i, line in enumerate(lines) if line.strip() == "1;" and i > start), len(lines))
    return lines[start : end + 1]


def emission_windows(lines: list[str], tag: str) -> list[tuple[int, int]]:
    hits = [i for i, line in enumerate(lines) if re.search(rf"['\"]{re.escape(tag)}['\"]", line)]
    windows: list[tuple[int, int]] = []
    for hit in hits:
        start, end = max(0, hit - EMISSION_CONTEXT), min(len(lines), hit + 6)
        if windows and start <= windows[-1][1]:
            windows[-1] = (windows[-1][0], end)
        else:
            windows.append((start, end))
    return windows


# --- debmagic side ------------------------------------------------------------------------------


def api_rows(check_text: str) -> list[tuple[str, str, str]]:
    """Rows of the API map whose lintian name the Check uses."""
    rows: list[tuple[str, str, str]] = []
    if not API_MAP.is_file():
        return rows
    for line in API_MAP.read_text(encoding="utf-8").splitlines():
        cells = [cell.strip() for cell in re.split(r"(?<!\\)\|", line.strip())[1:-1]]
        if len(cells) != 3 or cells[0] in {"lintian", "---"}:
            continue
        token = re.search(r"`([^`]+)`", cells[0])
        if token is None:
            continue
        name = token[1]
        if name.startswith("sub "):
            pattern = rf"^sub {re.escape(name[4:])}\b"
        elif "->" in name:
            pattern = re.escape(name.split("->", 1)[1] if name.startswith("$") else name) + r"\b"
            pattern = rf"->{pattern}" if name.startswith("$item") or name.startswith("fields") else pattern
        else:
            pattern = rf"\b{re.escape(name)}\b"
        if re.search(pattern, check_text, re.MULTILINE):
            rows.append((name, cells[1].replace("\\|", "|"), cells[2]))
    return rows


def data_usage(check_text: str) -> tuple[list[str], list[str]]:
    loads = sorted(set(re.findall(r"data->load\(\s*'([^']+)'", check_text)))
    accessors = sorted(set(re.findall(r"data->(\w+)", check_text)) - {"load", "value", "all_entries", "recognizes"})
    return loads, accessors


def snake(tag: str) -> str:
    return re.sub(r"[^a-z0-9]", "_", tag.lower())


def camel(tag: str) -> str:
    return "".join(word[:1].upper() + word[1:] for word in re.split(r"[^A-Za-z0-9]+", tag) if word)


def next_code(rules: list[report.RustRule]) -> str:
    used = [int(rule.code[2:]) for rule in rules]
    return f"LN{max(used, default=0) + 1:04d}"


def module_dir(check: str, siblings: list[report.RustRule]) -> str:
    if siblings:
        dirs = Counter(Path(rule.path).parent.as_posix() for rule in siblings)
        return dirs.most_common(1)[0][0]
    parts = check.split("/")
    for length in range(len(parts), 0, -1):
        candidate = RULES / re.sub(r"[^a-z0-9]", "_", "_".join(parts[:length]).lower())
        if candidate.is_dir():
            return candidate.relative_to(ROOT).as_posix()
    return (RULES / re.sub(r"[^a-z0-9]", "_", check.replace("/", "_").lower())).relative_to(ROOT).as_posix()


# --- recipes ------------------------------------------------------------------------------------


def skeleton_of(recipe_dir: Path) -> str:
    origin = recipe_dir / "ORIGIN"
    if origin.is_file():
        match = re.search(r"^Skeleton:\s*(\S+)", origin.read_text(encoding="utf-8", errors="replace"), re.MULTILINE)
        if match:
            return match[1]
    fill = recipe_dir / "build-spec/fill-values"
    if fill.is_file():
        return importer.read_deb822(fill).get("Skeleton", "?")
    return "?"


def lintian_recipes_with_tag(lintian: Path, tag: str) -> list[str]:
    rels: list[str] = []
    recipes = lintian / "t/recipes"
    for hints in recipes.glob("**/eval/hints"):
        text = hints.read_text(encoding="utf-8", errors="replace")
        if any((parsed := importer.parse_universal_hint_line(line)) and parsed[1] == tag for line in text.splitlines()):
            rels.append(hints.parent.parent.relative_to(recipes).as_posix())
    return sorted(rels)


SUBJECTS = {
    "source": "SourceRule (Source tree + Source package), or GroupRule if the Check emits it via $self->group",
    "binary": "BinaryPackageRule",
    "udeb": "BinaryPackageRule (udeb)",
    "changes": "ChangesRule (the Group's .changes; just new-rule --kind changes)",
    "buildinfo": "BuildinfoRule (the Group's .buildinfo; just new-rule --kind buildinfo)",
    "override": "OverrideRule (the override pass; just new-rule --kind override)",
}


# --- output -------------------------------------------------------------------------------------


def section(title: str) -> None:
    print(f"\n## {title}")


def print_tag(tag: report.LintianTag, tag_file: Path | None) -> None:
    section("Tag file")
    if tag_file is not None:
        print(tag_file.read_text(encoding="utf-8", errors="replace").rstrip())
    constructor = "Diagnostic::error" if tag.severity == "error" else "Diagnostic::warn"
    if tag.severity not in {"error", "warning"}:
        constructor += f" + .with_severity(Severity::{tag.severity.capitalize()})"
    print(
        f"\n=> default_selected = {str(tag.default_selected).lower()}, "
        f"experimental = {str(tag.experimental).lower()}, constructor: {constructor}"
    )


def print_catalogue(tag: report.LintianTag, tags: dict[str, report.LintianTag], ln_rules: list[report.RustRule]) -> str:
    """Print catalogue state; return the Rule's Code (assigned or next free)."""
    section("Catalogue")
    by_tag = {rule.tag: rule for rule in ln_rules}
    existing = by_tag.get(tag.tag)
    if existing is not None:
        print(f"catalogued: {existing.code} {existing.struct} ({', '.join(sorted(existing.kinds))}) {existing.path}")
        return existing.code
    code = next_code(ln_rules)
    siblings = [by_tag[t.tag] for t in tags.values() if t.check == tag.check and t.tag in by_tag]
    directory = module_dir(tag.check, siblings)
    print(f"not catalogued. Next free Code: {code} (a batch orchestrator assigns Codes instead)")
    print(f"struct {camel(tag.tag)}; file {directory}/{code.lower()}_{snake(tag.tag)}.rs")
    if not (ROOT / directory).is_dir():
        print(f"new module directory {directory}/ (add `mod` + `pub(crate) use` in rules/lintian/mod.rs)")
    return code


def print_check(lintian: Path, tag: report.LintianTag, full: bool) -> str:
    """Print the Check module (or its emission sites); return its source."""
    section(f"Check {tag.check}")
    module = check_module(lintian, tag.check)
    if module is None:
        print("Check module not found")
        return ""
    rel = module.relative_to(lintian).as_posix()
    check_text = module.read_text(encoding="utf-8", errors="replace")
    lines = strip_perl(check_text)
    if full or len(lines) <= SHORT_CHECK_LINES:
        print(f"{rel} ({len(lines)} lines)\n```perl\n" + "\n".join(lines) + "\n```")
        return check_text
    subs = [f"{i + 1}:{line.split('{')[0].strip()}" for i, line in enumerate(lines) if line.startswith("sub ")]
    print(f"{rel} ({len(lines)} lines; --full for all). subs: " + ", ".join(subs))
    for start, end in emission_windows(lines, tag.tag):
        print(f"```perl\n# lines {start + 1}-{end}\n" + "\n".join(lines[start:end]) + "\n```")
    return check_text


def print_api(check_text: str) -> None:
    rows = api_rows(check_text)
    if rows:
        section("lintian API used -> debmagic (docs/develop/lintian-api-map.md)")
        for name, debmagic, notes in rows:
            print(f"- {name} -> {debmagic}" + (f"  [{notes}]" if notes else ""))
    loads, accessors = data_usage(check_text)
    if loads or accessors:
        section("data/ tables")
        for load in loads:
            vendored = (DATA / load).exists()
            print(f"- {load}: {'vendored' if vendored else 'NOT vendored: copy from the pinned clone data/'}")
        for accessor in accessors:
            print(f"- data->{accessor} (lib/Lintian/Data accessor; see which file it reads)")


def print_recipes(lintian: Path, tag: report.LintianTag, recipes: list[report.Recipe]) -> set[str]:
    """Print the oracle recipes for the Tag; return the vendored recipe rels."""
    section("Recipes (oracle)")
    with_tag = [(recipe, [h for h in recipe.hints if h.tag == tag.tag]) for recipe in recipes]
    with_tag = [(recipe, hints) for recipe, hints in with_tag if hints]
    ptypes: Counter[str] = Counter()
    for recipe, hints in with_tag:
        print(f"- {recipe.rel} [{skeleton_of(SUITE / recipe.rel)}]")
        for hint in hints:
            ptypes[hint.ptype] += 1
            print(f"    ({hint.ptype}) {hint.extra}")
    if not with_tag:
        print("no vendored recipe names this Tag: no Parity possible without importing one")
    vendored = {recipe.rel for recipe, _ in with_tag}
    missing = [rel for rel in lintian_recipes_with_tag(lintian, tag.tag) if rel not in vendored]
    if missing:
        print("not vendored (import needed for Parity):")
        for rel in missing:
            print(f"- {rel} [{skeleton_of(lintian / 't/recipes' / rel)}]")
    if tag.tag in report.OVERRIDE_PASS_TAGS:
        print("\n=> implement: " + SUBJECTS["override"])
    elif ptypes:
        print("\n=> implement: " + "; ".join(f"{SUBJECTS.get(p, p)} ({n} lines)" for p, n in sorted(ptypes.items())))
    return vendored


def print_siblings(
    tag: report.LintianTag,
    tags: dict[str, report.LintianTag],
    ln_rules: list[report.RustRule],
    recipes: list[report.Recipe],
) -> None:
    section(f"Tags of Check {tag.check}")
    by_tag = {rule.tag: rule for rule in ln_rules}
    index: dict[str, int] = defaultdict(int)
    for recipe in recipes:
        for name in {h.tag for h in recipe.hints}:
            index[name] += 1
    for other in sorted((t for t in tags.values() if t.check == tag.check), key=lambda t: t.tag):
        state = by_tag[other.tag].code if other.tag in by_tag else "-"
        mark = " <-" if other.tag == tag.tag else ""
        print(f"- {state:6} {other.severity:14} {index.get(other.tag, 0):3} recipes  {other.tag}{mark}")


def print_commands(tag: report.LintianTag, code: str, vendored: set[str]) -> list[str]:
    """Print the test commands; return the generated Parity case filters."""
    section("Commands")
    listed = report.parse_listed_tests(TESTS_RS)
    cases = sorted(
        f"{report.KIND_TO_TEST[kind]}::{name}"
        for kind, rels in listed.items()
        for rel, name in rels.items()
        if rel in vendored
    )
    module = f"{code.lower()}_{snake(tag.tag)}"
    print(f"native:  cargo test -p debmagic --bin debmagic {module}")
    print(f"snapshots: just snap-accept {module}")
    print("list new Parity cases: python3 scripts/import_lintian_parity.py --write-tests")
    if cases:
        print("parity:  cargo test -p debmagic --test lintian_parity -- " + " ".join(cases))
    elif vendored:
        print("parity:  no generated case yet (catalogue the Rule, then --write-tests)")
    print("finally: just fmt lint test")
    return cases


def run_parity(cases: list[str]) -> int:
    section("Parity run")
    command = ["cargo", "test", "-p", "debmagic", "--test", "lintian_parity", "--", *cases]
    result = subprocess.run(command, cwd=ROOT, check=False, capture_output=True, text=True)
    for line in (result.stdout + result.stderr).splitlines():
        if line.startswith(("test ", "test result", "  left", " right")) or "panicked" in line:
            print(line)
    return result.returncode


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("tag", help="lintian Tag name")
    parser.add_argument("--lintian-src", type=Path, help="lintian source tree at the PIN version")
    parser.add_argument("--full", action="store_true", help="print the whole Check module")
    parser.add_argument("--run", action="store_true", help="run this Tag's generated Parity cases")
    args = parser.parse_args(argv)

    lintian = lintian_source(args.lintian_src)
    tags = report.load_lintian_tags(lintian)
    version, _commit = report.read_pin(ROOT)
    tag = tags.get(args.tag)
    if tag is None:
        renamed = [t.tag for t in tags.values() if args.tag in t.renamed_from]
        close = ", ".join(difflib.get_close_matches(args.tag, tags, 5))
        print(
            f"no Tag {args.tag!r} in lintian {version} ({f'renamed to {renamed[0]}' if renamed else f'close: {close}'})"
        )
        return 2

    print(f"# {tag.tag} (lintian {version})")
    print_tag(tag, find_tag_file(lintian, tag.tag))
    if not tag.rule_eligible:
        print("\nClassification tag: not a Rule (ADR 0019). Stop.")
        return 0
    ln_rules = report.parse_rust_rules(ROOT, "LN")
    code = print_catalogue(tag, tags, ln_rules)
    print_api(print_check(lintian, tag, args.full))
    recipes = report.load_recipes(SUITE)
    vendored = print_recipes(lintian, tag, recipes)
    print_siblings(tag, tags, ln_rules, recipes)
    cases = print_commands(tag, code, vendored)
    if args.run and cases:
        return run_parity(cases)
    return 0


if __name__ == "__main__":
    sys.exit(main())
