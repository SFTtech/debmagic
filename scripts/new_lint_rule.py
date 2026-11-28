#!/usr/bin/env python3
"""Scaffold the LN Rule for one lintian Tag and register it.

Does the mechanical part of "catalogue the Rule" (docs/develop/import-lintian-rule.md §2):

- creates `src/lint/rules/lintian/<module>/ln<code>_<snake_tag>.rs` with the doc comment slot,
  `declare_rule!` (default Selection and Severity from the `.tag` file), the Rule trait impl for
  each Subject kind the vendored recipes exercise, and a compiling test skeleton;
- creates the module (`mod.rs` plus `mod`/`pub(crate) use` in `rules/lintian/mod.rs`) when the
  Check has none yet, otherwise adds the Rule to the module's `mod.rs`;
- registers the Rule in `registry.rs` (`use`, `all_rules`, and the Subject lists).

The Rule body is a stub (it reports nothing); the tests do not exercise it yet. Read
`just lintian-tag <tag>` for what to implement.

    just new-rule <tag>                  # next free Code, kinds from the vendored recipes
    just new-rule <tag> --code LN0850    # a pre-assigned Code (batch runs)
    just new-rule <tag> --kind source    # or binary, both, changes, buildinfo, group (or a comma list)
    just new-rule <tag> --dry-run        # print the plan only
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import lint_parity_report as report
import lintian_tag as tagtool

ROOT = tagtool.ROOT
LINT = ROOT / "packages/debmagic/src/lint"
REGISTRY = LINT / "registry.rs"
RULES_MOD = LINT / "rules/lintian/mod.rs"

KIND_TRAIT = {
    "source": "SourceRule",
    "binary": "BinaryPackageRule",
    "changes": "ChangesRule",
    "buildinfo": "BuildinfoRule",
    "group": "GroupRule",
    "override": "OverrideRule",
}
KIND_CONTEXT = {
    "source": "SourceContext",
    "binary": "BinaryPackageContext",
    "changes": "ChangesContext",
    "buildinfo": "BuildinfoContext",
    "group": "GroupContext",
    "override": "OverrideContext",
}
# The Context types of Group artifacts live in `group_context`, the others in `context`.
KIND_CONTEXT_MODULE = {
    "source": "context",
    "binary": "context",
    "changes": "group_context",
    "buildinfo": "group_context",
    "group": "group_context",
    "override": "override_pass",
}
KIND_REGISTRY_LIST = {
    "source": "source_rules",
    "binary": "binary_package_rules",
    "changes": "changes_rules",
    "buildinfo": "buildinfo_rules",
    "group": "group_rules",
    "override": "override_rules",
}
KIND_TESTER = {
    "source": ("SourceTreeTester", "TestCase"),
    "binary": ("BinaryPackageTester", "BinaryPackageCase"),
    "changes": ("ChangesTester", "ChangesCase"),
    "buildinfo": ("BuildinfoTester", "BuildinfoCase"),
    "group": ("GroupTester", "GroupCase"),
    "override": ("OverrideTester", "OverrideCase"),
}


def kinds_from_recipes(tag: str) -> list[str]:
    if tag in report.OVERRIDE_PASS_TAGS:
        return ["override"]
    kinds: set[str] = set()
    for recipe in report.load_recipes(tagtool.SUITE):
        for hint in recipe.hints:
            if hint.tag == tag:
                kinds.add(report.PTYPE_TO_KIND.get(hint.ptype, ""))
    kinds.discard("")
    return sorted(kinds) or ["binary"]


def mentions_group(lintian: Path, check: str) -> bool:
    """The Check reads the Group (`$self->group`): its Tags may need `--kind group`."""
    module = tagtool.check_module(lintian, check)
    return module is not None and "$self->group" in module.read_text(encoding="utf-8", errors="replace")


def render_rule(tag: report.LintianTag, struct: str, code: str, kinds: list[str]) -> str:
    imports = ["use crate::declare_rule;"]
    for kind in kinds:
        imports.append(f"use crate::lint::{KIND_CONTEXT_MODULE[kind]}::{KIND_CONTEXT[kind]};")
    imports.append("use crate::lint::diagnostic::{Diagnostic, Location};")
    traits = ", ".join(sorted(KIND_TRAIT[kind] for kind in kinds))
    imports.append(f"use crate::lint::rule::{{{traits}}};" if len(kinds) > 1 else f"use crate::lint::rule::{traits};")
    constructor = "error" if tag.severity == "error" else "warn"
    impls = "\n".join(
        f"""impl {KIND_TRAIT[kind]} for {struct} {{
    fn run(&self, _ctx: &mut {KIND_CONTEXT[kind]}<'_>) {{
        // TODO: port lintian's `{tag.check}` Check (just lintian-tag {tag.tag}).
        // Report with `ctx.diagnostic(Diagnostic::{constructor}(extra).with_location(Location {{ .. }}))`.
        let _ = (Diagnostic::{constructor}(""), None::<Location>);
    }}
}}
"""
        for kind in kinds
    )
    tests = []
    for kind in kinds:
        tester, case = KIND_TESTER[kind]
        tests.append(
            f"""    #[test]
    fn {kind if kind != "override" else "override_pass"}() {{
        // Pass and fail cases; `.test_and_snapshot()` once there are fail cases (`just snap-accept`).
        let pass = Vec::<{case}>::new();
        let fail = Vec::<{case}>::new();
        {tester}::new({struct}, pass, fail).test();
    }}
"""
        )
    tester_imports = ", ".join(sorted(name for kind in kinds for name in (KIND_TESTER[kind][1], KIND_TESTER[kind][0])))
    return f"""{chr(10).join(imports)}

pub struct {struct};

declare_rule! {{
    /// TODO: what the Rule reports, in lintian's words, and the cases it skips.
    {struct},
    code = "{code}",
    tag = "{tag.tag}",
    default_selected = {str(tag.default_selected).lower()},
    experimental = {str(tag.experimental).lower()},
}}

{impls}
#[cfg(test)]
mod tests {{
    use crate::lint::tester::{{{tester_imports}}};

    use super::*;

{chr(10).join(tests)}}}
"""


def insert_before(text: str, anchor: str, line: str, occurrence: int = 0) -> str:
    """Insert `line` before the `occurrence`-th `anchor` (a line that must exist)."""
    positions = [m.start() for m in re.finditer(re.escape(anchor), text)]
    if len(positions) <= occurrence:
        raise SystemExit(f"cannot place {line.strip()!r}: anchor {anchor!r} not found")
    at = positions[occurrence]
    return text[:at] + line + text[at:]


def register_in_registry(struct: str, kinds: list[str]) -> None:
    text = REGISTRY.read_text(encoding="utf-8")
    text = text.replace("\nuse super::rules::", f"\nuse super::rules::{struct};\nuse super::rules::", 1)
    for function in ["all_rules", *[KIND_REGISTRY_LIST[k] for k in kinds]]:
        # The list is the first `&[ ... ]` of the function (`cargo fmt` keeps a short one on a single line,
        # and the lists of the Group kinds start empty: `&[]`). A Rule line is `&Name,`: no `]` inside.
        function_at = re.search(rf"pub\(crate\) fn {function}\(\)[^{{]*\{{\n", text)
        if function_at is None:
            raise SystemExit(f"registry.rs: list {function} not found")
        close = text.index("]", text.index("&[", function_at.end()))
        before = text[:close].rstrip()
        comma = "" if before.endswith(("[", ",")) else ","
        text = f"{before}{comma}\n        &{struct},\n    {text[close:]}"
    REGISTRY.write_text(text, encoding="utf-8")


def register_in_module(directory: Path, stem: str, struct: str, new_module: bool, check: str) -> None:
    mod_rs = directory / "mod.rs"
    allow = "#[allow(non_snake_case)]\n" if "__" in stem else ""
    if new_module:
        directory.mkdir(parents=True, exist_ok=True)
        mod_rs.write_text(
            f"//! Rules for lintian's `{check}` Check.\n\n{allow}mod {stem};\n\npub(crate) use {stem}::{struct};\n",
            encoding="utf-8",
        )
        module = directory.name
        text = RULES_MOD.read_text(encoding="utf-8")
        lines = text.splitlines(keepends=True)
        mods = [i for i, line in enumerate(lines) if re.match(r"mod \w+;", line)]
        later = next((i for i in mods if lines[i].strip() > f"mod {module};"), mods[-1] + 1)
        lines.insert(later, f"mod {module};\n")
        uses = [i for i, line in enumerate(lines) if line.startswith("pub(crate) use ")]
        later = next((i for i in uses if lines[i].split()[2].split("::")[0] > module), uses[-1] + 1)
        while later < len(lines) and lines[later - 1].rstrip().endswith(","):
            later += 1  # do not split a multi-line `use`
        lines.insert(later, f"pub(crate) use {module}::{struct};\n")
        RULES_MOD.write_text("".join(lines), encoding="utf-8")
        return
    text = mod_rs.read_text(encoding="utf-8")
    mods = list(re.finditer(r"^mod \w+;\n", text, re.MULTILINE))
    at = mods[-1].end() if mods else 0
    text = text[:at] + f"{allow}mod {stem};\n" + text[at:]
    uses = list(re.finditer(r"^pub\(crate\) use [^;]*;\n", text, re.MULTILINE))
    at = uses[-1].end() if uses else len(text)
    text = text[:at] + f"pub(crate) use {stem}::{struct};\n" + text[at:]
    mod_rs.write_text(text, encoding="utf-8")
    # `rules/lintian/mod.rs` re-exports the module's Rules; add the new one next to its siblings.
    module = directory.name
    text = RULES_MOD.read_text(encoding="utf-8")
    single = re.search(rf"^pub\(crate\) use {re.escape(module)}::(\w+);\n", text, re.MULTILINE)
    braced = re.search(rf"^pub\(crate\) use {re.escape(module)}::\{{(.*?)\}};\n", text, re.MULTILINE | re.DOTALL)
    if braced:
        names = braced[1].rstrip().rstrip(",")
        text = text[: braced.start(1)] + f"{names}, {struct}," + text[braced.end(1) :]
    elif single:
        text = text[: single.end()] + f"pub(crate) use {module}::{struct};\n" + text[single.end() :]
    else:
        text = text.rstrip("\n") + f"\npub(crate) use {module}::{struct};\n"
    RULES_MOD.write_text(text, encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("tag", help="lintian Tag name")
    parser.add_argument("--code", help="LN Code to use (default: the next free one)")
    parser.add_argument(
        "--kind",
        help="Applicability: source, binary, both (source+binary), changes, buildinfo, group, override "
        "(the override pass, for unused-override and mismatched-override); a comma list "
        "(default: from recipes; a Group Rule must be asked for)",
    )
    parser.add_argument("--lintian-src", type=Path, help="lintian source tree at the PIN version")
    parser.add_argument("--dry-run", action="store_true", help="print the plan, change nothing")
    args = parser.parse_args(argv)

    tags = report.load_lintian_tags(tagtool.lintian_source(args.lintian_src))
    tag = tags.get(args.tag)
    if tag is None:
        raise SystemExit(f"no Tag {args.tag!r} in the pinned lintian (check `just lintian-tag {args.tag}`)")
    if not tag.rule_eligible:
        raise SystemExit(f"{args.tag} is a classification tag, not a Rule")
    rules = report.parse_rust_rules(ROOT, "LN")
    if any(rule.tag == tag.tag for rule in rules):
        raise SystemExit(f"{tag.tag} is already catalogued")
    code = args.code or tagtool.next_code(rules)
    if not re.fullmatch(r"LN\d{4}", code) or any(rule.code == code for rule in rules):
        raise SystemExit(f"{code!r} is not a free LN Code")
    if args.kind:
        kinds = sorted(
            {k for part in args.kind.split(",") for k in (["source", "binary"] if part == "both" else [part])}
        )
        unknown = [k for k in kinds if k not in KIND_TRAIT]
        if unknown:
            raise SystemExit(f"unknown --kind {unknown[0]!r}; expected one of {', '.join(KIND_TRAIT)}, both")
    else:
        kinds = kinds_from_recipes(tag.tag)
        if mentions_group(tagtool.lintian_source(args.lintian_src), tag.check):
            print(f"note: the {tag.check} Check reads `$self->group`; pass --kind group if {tag.tag} is a Group Rule")

    struct = tagtool.camel(tag.tag)
    stem = f"{code.lower()}_{tagtool.snake(tag.tag)}"
    by_tag = {rule.tag: rule for rule in rules}
    siblings = [by_tag[t.tag] for t in tags.values() if t.check == tag.check and t.tag in by_tag]
    directory = ROOT / tagtool.module_dir(tag.check, siblings)
    new_module = not (directory / "mod.rs").is_file()
    rule_file = directory / f"{stem}.rs"
    suffix = " (new module)" if new_module else ""
    print(f"{struct} {code} ({'+'.join(kinds)}): {rule_file.relative_to(ROOT)}{suffix}")
    if args.dry_run:
        return 0

    directory.mkdir(parents=True, exist_ok=True)
    rule_file.write_text(render_rule(tag, struct, code, kinds), encoding="utf-8")
    register_in_module(directory, stem, struct, new_module, tag.check)
    register_in_registry(struct, kinds)
    print("registered in registry.rs and rules/lintian/mod.rs; now: cargo fmt --all && cargo build -p debmagic")
    return 0


if __name__ == "__main__":
    sys.exit(main())
