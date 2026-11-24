#!/usr/bin/env python3
"""Print the Rule-facing `debmagic lint` API as a compact index.

Generated from the source on every run, so it cannot go stale: public items of the
inspectable modules (with the first sentence of their doc comment), the testers, and the
shared helper modules that live inside `rules/`. Meant as the one read an agent or a person
needs before writing a Rule, instead of grepping `pub fn` across `src/lint/`.

    just lint-api              # everything
    just lint-api file_tree    # only modules whose path contains "file_tree"
"""

from __future__ import annotations

import argparse
import re
import sys
from collections.abc import Iterator
from pathlib import Path

LINT = Path("packages/debmagic/src/lint")
# Plumbing a Rule never calls.
SKIP_MODULES = {
    "mod.rs",
    "registry.rs",
    "run.rs",
    "intent.rs",
    "selection.rs",
    "timing.rs",
    "binary_package_reader.rs",
    "source_package.rs",
    "subject.rs",
    "gzip.rs",
}
# Context methods that are plumbing, not inspectables.
SKIP_METHODS = {"bind", "bind_tree", "bind_package", "finish", "shared_once"}
# Data constructors in context.rs only the runner uses.
SKIP_IMPLS = {"DiagnosticSink", "SourceTreeData", "BinaryPackageData", "SourcePackageData"}

ITEM = re.compile(r"^pub(?:\([a-z]+\))?\s+(?:const\s+fn|fn|struct|enum|trait|type|const|static)\s+(\w+)")
IMPL = re.compile(r"^impl(?:<[^>]*>)?\s+(?:(?P<trait>[\w:<>, ']+?)\s+for\s+)?(?P<type>\w+)")
METHOD = re.compile(r"^    pub(?:\([a-z]+\))?\s+(?:const\s+)?fn\s+(\w+)")
HELPER_FN = re.compile(r"^pub(?:\((?:crate|super)\))?\s+fn\s+(\w+)")
MACRO = re.compile(r"^macro_rules!\s+(\w+)")


def first_sentence(doc: list[str]) -> str:
    text = " ".join(line.strip() for line in doc).strip()
    if not text:
        return ""
    match = re.match(r"(.+?(?<!e\.g)(?<!i\.e)(?<!etc)\.)(\s+[A-Z`\[(]|$)", text)
    sentence = match[1] if match else text
    return sentence if len(sentence) <= 160 else sentence[:157] + "..."


def signature(lines: list[str], start: int) -> str:
    """The fn signature from `start` up to its body or `;`, on one line."""
    parts: list[str] = []
    for line in lines[start : start + 12]:
        parts.append(line.strip())
        if line.rstrip().endswith(("{", ";")) or " where" in line:
            break
    text = " ".join(parts)
    text = re.sub(r"\s*(\{|where\b).*$", "", text).rstrip(";").strip()
    text = re.sub(r"^pub(\([a-z]+\))?\s+", "", text)
    return re.sub(r"\s+", " ", text).replace("( ", "(").replace(" )", ")").replace(",)", ")")


def module_doc(lines: list[str]) -> str:
    doc = [line[3:] for line in lines if line.startswith("//!")]
    return first_sentence(doc)


def production_lines(lines: list[str]) -> Iterator[tuple[int, str | None]]:
    """Lines outside `#[cfg(test)]` items; a skipped item yields one `None`."""
    test_pending = False
    in_test_block = False
    for number, line in enumerate(lines):
        if in_test_block:
            in_test_block = not line.startswith("}")
        elif line.startswith("#[cfg(test)]"):
            test_pending = True
        elif test_pending and line[:1].isalpha():
            test_pending = False
            in_test_block = line.rstrip().endswith("{")
            yield number, None
        else:
            yield number, line


def index_module(path: Path) -> list[str]:
    lines = path.read_text(encoding="utf-8").splitlines()
    out: list[str] = []
    doc: list[str] = []
    impl: str | None = None
    skip_impl = False
    for number, line in production_lines(lines):
        if line is None:
            doc = []
            continue
        stripped = line.strip()
        if stripped.startswith("///"):
            doc.append(stripped[3:])
            continue
        if stripped.startswith("#[") or stripped.startswith("//"):
            continue
        impl_match = IMPL.match(line)
        if impl_match:
            impl = impl_match["type"]
            trait = impl_match["trait"]
            skip_impl = impl in SKIP_IMPLS or (trait is not None and trait.split("<")[0] not in {"Default"})
            if not skip_impl:
                out.append(f"  impl {impl}:")
            doc = []
            continue
        if line.startswith("}"):
            impl = None
            skip_impl = False
            doc = []
            continue
        if impl is not None:
            method = METHOD.match(line)
            if method and not skip_impl and method[1] not in SKIP_METHODS:
                summary = first_sentence(doc)
                out.append(f"    {signature(lines, number)}" + (f"  // {summary}" if summary else ""))
            if not line.startswith("    ") or stripped:
                if not stripped.startswith("///"):
                    doc = [] if not line.startswith("    #") else doc
            continue
        item = ITEM.match(line) or MACRO.match(line)
        if item:
            summary = first_sentence(doc)
            bare = re.sub(r"^pub(\([a-z]+\))?\s+", "", stripped)
            head = signature(lines, number) if " fn " in f" {line}" else re.sub(r"\s*(\{|\(|;|=).*$", "", bare)
            out.append(f"  {head}" + (f"  // {summary}" if summary else ""))
        if stripped:
            doc = []
    return out


def helper_modules(rules: Path) -> list[str]:
    """Shared, non-Rule modules inside `rules/` (scanners, parsers) with their pub fns."""
    out: list[str] = []
    for path in sorted(rules.rglob("*.rs")):
        if path.name == "mod.rs" or re.match(r"(ln|dm)\d{4}_", path.name):
            continue
        parent = (path.parent / "mod.rs").read_text(encoding="utf-8") if (path.parent / "mod.rs").is_file() else ""
        if re.search(rf"#\[cfg\(test\)\]\s*(pub(\([a-z]+\))?\s+)?mod\s+{re.escape(path.stem)}\s*;", parent):
            continue
        lines = path.read_text(encoding="utf-8").splitlines()
        names: list[str] = []
        for _number, line in production_lines(lines):
            match = HELPER_FN.match(line or "")
            if match:
                names.append(match[1])
        if not names:
            continue
        rel = path.relative_to(rules).as_posix()
        doc = module_doc(lines)
        out.append(f"  {rel}: {', '.join(names)}" + (f"  // {doc}" if doc else ""))
    return out


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("filter", nargs="?", default="", help="only modules whose path contains this text")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args(argv)
    lint = args.root / LINT

    modules = sorted(
        path
        for path in [*lint.glob("*.rs"), *lint.glob("*/*.rs")]
        if path.name not in SKIP_MODULES and path.parent.name not in {"rules", "snapshots"}
    )
    print("# debmagic lint API (generated by scripts/lint_api.py; Rule-facing items only)")
    print("# Rules: `impl SourceRule` (Source tree + Source package) / `impl BinaryPackageRule`; see rule.rs.")
    for path in modules:
        rel = path.relative_to(lint).as_posix()
        if args.filter not in rel:
            continue
        body = index_module(path)
        body = [
            line
            for i, line in enumerate(body)
            if not (line.startswith("  impl ") and (i + 1 == len(body) or not body[i + 1].startswith("    ")))
        ]
        if not body:
            continue
        doc = module_doc(path.read_text(encoding="utf-8").splitlines())
        print(f"\n## {rel}" + (f" — {doc}" if doc else ""))
        print("\n".join(body))
    if args.filter in "rules/":
        print("\n## rules/lintian helper modules (reuse before writing a new scanner)")
        print("\n".join(helper_modules(lint / "rules" / "lintian")))
    return 0


if __name__ == "__main__":
    sys.exit(main())
