#!/usr/bin/env python3
"""Import every filled lintian recipe as Parity fixtures.

Clones (or uses) a pinned lintian tree, runs a Prepare.pm-shaped fill, and
writes test-package *sources* plus eval/hints + ORIGIN under
packages/debmagic/tests/lintian-parity, and regenerates
packages/debmagic/tests/lintian_parity.rs with one test per Source-tree
recipe whose hints mention a catalogued LN Tag.

- upload-* / source-* / upload-builder-only: filled Source tree (orig overlay + debian/)
- deb: filled DEBIAN/, root/, doc/, and builder scripts
- changes: filled test.changes and referenced-files

Does not build .deb / .dsc / .changes products (no dpkg-buildpackage, dpkg-source,
ar, gzip-into-deb, or md5sums). Tests build those artifacts themselves.
"""

from __future__ import annotations

import argparse
import json
import locale
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from collections.abc import Iterable
from pathlib import Path

LINTIAN_GIT = "https://salsa.debian.org/lintian/lintian.git"
SUITE_RELATIVE = Path("packages/debmagic/tests/lintian-parity")
RULES_RELATIVE = Path("packages/debmagic/src/lint/rules/lintian")
GENERATED_TESTS_RELATIVE = Path("packages/debmagic/tests/lintian_parity.rs")
SOURCE_TREE_SKELETONS = frozenset(
    {
        "upload-native",
        "upload-non-native",
        "source-native",
        "source-non-native",
        "upload-builder-only",
    }
)
DEB_SKELETON = "deb"
CHANGES_SKELETON = "changes"
IMPORTABLE_SKELETONS = SOURCE_TREE_SKELETONS | {DEB_SKELETON, CHANGES_SKELETON}
HINT_LINE = re.compile(r"^\S+\s+\((?P<ptype>[^)]+)\):\s+(?P<tag>\S+)(?:\s+(?P<extra>.*))?$")
TAG_ASSIGNMENT = re.compile(r'^tag = "([^"]+)",\s*$')
TEMPLATE_VAR = re.compile(r"\[%\s*\$([A-Za-z_][A-Za-z0-9_]*)\s*%\]")
TEMPLATE_SET = re.compile(r"^\s*([^()\s]+)\s*\(([^()\s]+)\)\s*$")
SKIP_FROM_WORK = frozenset(
    {
        "files",
        "fill-values",
        "fill-values.d",
    }
)


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def field_to_key(name: str) -> str:
    return name.lower().replace("-", "_")


def read_text(path: Path) -> tuple[str, str]:
    raw = path.read_bytes()
    try:
        return raw.decode("utf-8"), "utf-8"
    except UnicodeDecodeError:
        return raw.decode("latin-1"), "latin-1"


def read_deb822(path: Path) -> dict[str, str]:
    fields: dict[str, str] = {}
    current: str | None = None
    text, _encoding = read_text(path)
    for raw_line in text.splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if raw_line[0] in " \t":
            if current is None:
                raise ValueError(f"{path}: continuation without a field")
            extra = raw_line.strip()
            fields[current] = f"{fields[current]} {extra}".strip()
            continue
        name, sep, value = raw_line.partition(":")
        if not sep:
            raise ValueError(f"{path}: not a Deb822 field: {raw_line!r}")
        current = name
        fields[current] = value.strip()
    return fields


def merge_fields(base: dict[str, str], overlay: dict[str, str]) -> dict[str, str]:
    merged = dict(base)
    merged.update(overlay)
    return merged


def to_hash(fields: dict[str, str]) -> dict[str, str]:
    return {field_to_key(name): value for name, value in fields.items()}


def fill_text(template: str, values: dict[str, str], *, allow_leftover: bool = False) -> str:
    def replace(match: re.Match[str]) -> str:
        return values.get(match.group(1), "")

    filled = TEMPLATE_VAR.sub(replace, template)
    if not allow_leftover and "[%" in filled:
        raise ValueError(f"unexpanded template leftover: {filled[:120]!r}")
    return filled


def fill_hash(values: dict[str, str]) -> dict[str, str]:
    filled = dict(values)
    for index in range(2):
        allow_leftover = index == 0
        filled = {key: fill_text(value, filled, allow_leftover=allow_leftover) for key, value in filled.items()}
    return filled


def parse_placements(instructions: str) -> list[tuple[str, str | None]]:
    placements: list[tuple[str, str | None]] = []
    for raw_chunk in instructions.split(","):
        chunk = raw_chunk.strip()
        if not chunk:
            continue
        matched = TEMPLATE_SET.match(chunk)
        if matched:
            placements.append((matched.group(1), matched.group(2)))
        else:
            placements.append((chunk, None))
    return placements


def copy_contents(source: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    if not any(source.iterdir()):
        return
    shutil.copytree(source, destination, dirs_exist_ok=True, symlinks=False)


def iter_files(root: Path) -> Iterable[Path]:
    for path in root.rglob("*"):
        if path.is_file():
            yield path


def remove_surplus_templates(spec: Path, work: Path) -> None:
    for original in iter_files(spec):
        relative = original.relative_to(spec)
        template = work / f"{relative}.in"
        if template.is_file():
            template.unlink()


def fill_template_file(template: Path, generated: Path, values: dict[str, str]) -> None:
    generated.parent.mkdir(parents=True, exist_ok=True)
    text, encoding = read_text(template)
    generated.write_text(fill_text(text, values), encoding=encoding)
    shutil.copymode(template, generated)
    template.unlink()


def fill_whitelisted(work: Path, relative: str, whitelist: Path, values: dict[str, str]) -> None:
    names = read_deb822(whitelist).get("May-Generate", "").split()
    destination = work if relative == "." else work / relative
    for name in names:
        generated = destination / name
        template = Path(str(generated) + ".in")
        if template.is_file():
            fill_template_file(template, generated, values)


def fill_single(work: Path, relative: str, values: dict[str, str]) -> None:
    generated = work / relative
    template = Path(str(generated) + ".in")
    if template.is_file():
        fill_template_file(template, generated, values)


def rfc822date(epoch: int) -> str:
    previous = locale.setlocale(locale.LC_TIME, "C")
    try:
        return time.strftime("%a, %d %b %Y %H:%M:%S %z", time.localtime(epoch))
    finally:
        locale.setlocale(locale.LC_TIME, previous)


def latest_policy(lintian_root: Path) -> tuple[str, int]:
    releases = json.loads((lintian_root / "data/debian-policy/releases.json").read_text(encoding="utf-8"))
    latest = releases["releases"][0]
    return str(latest["version"]), int(latest["epoch"])


def recommended_debhelper_compat(lintian_root: Path) -> str:
    for line in (lintian_root / "data/debhelper/compat-level").read_text(encoding="utf-8").splitlines():
        if line.startswith("recommended="):
            return line.split("=", 1)[1].strip()
    raise ValueError(f"no recommended= in {lintian_root / 'data/debhelper/compat-level'}")


def host_architecture() -> str:
    result = subprocess.run(
        ["dpkg-architecture", "-qDEB_HOST_ARCH"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip()
    return "amd64"


def derive_versions(fields: dict[str, str]) -> None:
    version = fields.get("Version")
    if not version:
        return
    upstream = re.sub(r"-[^-]+$", "", version)
    upstream = re.sub(r"(-|^)(\d+):", r"\1", upstream)
    fields.setdefault("Upstream-Version", upstream)
    fields.setdefault("No-Epoch", re.sub(r"^\d+:", "", version))
    if "Prev-Version" not in fields:
        prev = "0.0.1"
        if fields.get("Type") != "native":
            prev += "-1"
        fields["Prev-Version"] = prev


def combine_build_fields(fields: dict[str, str]) -> None:
    parts = [fields.pop("Default-Build-Depends", ""), fields.pop("Extra-Build-Depends", "")]
    combined = ", ".join(part for part in parts if part)
    if combined:
        fields["Build-Depends"] = combined
    parts = [fields.pop("Default-Build-Conflicts", ""), fields.pop("Extra-Build-Conflicts", "")]
    combined = ", ".join(part for part in parts if part)
    if combined:
        fields["Build-Conflicts"] = combined


def load_fill_values_folder(work: Path, folder_name: str) -> dict[str, str]:
    folder = work / folder_name
    merged: dict[str, str] = {}
    if not folder.is_dir():
        return merged
    for path in sorted(folder.glob("*.values")):
        merged = merge_fields(merged, read_deb822(path))
    return merged


def prepare_work(lintian_root: Path, spec: Path, work: Path) -> dict[str, str]:
    testset = lintian_root / "t"
    defaults = read_deb822(testset / "defaults/fill-values")
    desc = read_deb822(spec / "fill-values")
    skeleton_name = desc["Skeleton"]
    skeleton = read_deb822(testset / "skeletons" / skeleton_name)
    fields = merge_fields(defaults, skeleton)

    for relative, name in parse_placements(fields.get("Template-Sets", "")):
        if name is None:
            raise ValueError(f"template set without a name: {relative}")
        source = testset / "templates" / name
        destination = work if relative == "." else work / relative
        copy_contents(source, destination)

    remove_surplus_templates(spec, work)
    copy_contents(spec, work)

    fields = merge_fields(fields, load_fill_values_folder(work, fields.get("Fill-Values-Folder", "")))
    fields = merge_fields(fields, desc)
    fields.setdefault("Source", fields["Testname"])
    fields.setdefault("Source-Path", str(work.resolve()))
    fields.setdefault("Spec-Path", str(spec.resolve()))

    policy_version, policy_epoch = latest_policy(lintian_root)
    fields.setdefault("Date", rfc822date(policy_epoch))
    fields.setdefault("Host-Architecture", host_architecture())
    fields.setdefault("Standards-Version", policy_version)
    fields.setdefault("Dh-Compat-Level", recommended_debhelper_compat(lintian_root))
    derive_versions(fields)
    combine_build_fields(fields)

    values = fill_hash(to_hash(fields))
    for relative, name in parse_placements(fields.get("Fill-Targets", "")):
        if name is None:
            fill_single(work, relative, values)
            continue
        fill_whitelisted(work, relative, testset / "whitelists" / name, values)
    return values


def run_hook(hook: Path, *args: Path | str) -> None:
    if not hook.is_file() or not hook.stat().st_mode & stat.S_IXUSR:
        return
    command = [str(hook), *[str(arg) for arg in args]]
    result = subprocess.run(command, check=False)
    if result.returncode != 0:
        print(
            f"warning: {hook.name} exited {result.returncode} for {args[0] if args else hook}",
            file=sys.stderr,
        )


def assemble_source_tree(work: Path, dest: Path, values: dict[str, str]) -> None:
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    orig = work / "orig"
    if orig.is_dir():
        copy_contents(orig, dest)
    if values.get("type") != "native":
        run_hook(work / "pre-upstream", dest)
    debian_src = work / "debian"
    if debian_src.is_dir():
        copy_contents(debian_src, dest / "debian")
    run_hook(work / "pre-build", dest)


def copy_filled_work(work: Path, dest: Path) -> None:
    """Copy filled recipe sources; leave package builds to the test runner."""
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    for child in work.iterdir():
        if child.name in SKIP_FROM_WORK:
            continue
        target = dest / child.name
        if child.is_dir():
            copy_contents(child, target)
        else:
            shutil.copy2(child, target)


def write_origin(dest: Path, version: str, commit: str, recipe_rel: str, skeleton: str) -> None:
    dest.write_text(
        "\n".join(
            [
                f"lintian {version} ({commit})",
                f"t/recipes/{recipe_rel}",
                f"Skeleton: {skeleton}",
                "",
                "Filled test-package sources from t/templates + t/skeletons and the",
                "build-spec overlay. Does not build .deb / .dsc / .changes; tests do that.",
                "eval/hints is transcribed verbatim.",
                "",
                "License: GPL-2+ (same as lintian; see lintian debian/copyright).",
                "",
            ]
        ),
        encoding="utf-8",
    )


def copy_eval(recipe: Path, dest: Path) -> None:
    eval_src = recipe / "eval"
    eval_dir = dest / "eval"
    eval_dir.mkdir(parents=True, exist_ok=True)
    if not eval_src.is_dir():
        return
    for path in eval_src.iterdir():
        if path.is_file() and not path.name.endswith(".in"):
            shutil.copy2(path, eval_dir / path.name)


def import_recipe(
    lintian_root: Path,
    recipe_rel: str,
    dest_root: Path,
    version: str,
    commit: str,
) -> Path:
    recipe = lintian_root / "t/recipes" / recipe_rel
    spec = recipe / "build-spec"
    desc = read_deb822(spec / "fill-values")
    skeleton = desc.get("Skeleton", "")
    if skeleton not in IMPORTABLE_SKELETONS:
        raise ValueError(f"{recipe_rel}: skeleton {skeleton!r} is not importable")

    dest = dest_root / Path(recipe_rel)
    with tempfile.TemporaryDirectory(prefix="debmagic-lintian-import-") as tmp:
        work = Path(tmp) / "work"
        work.mkdir()
        values = prepare_work(lintian_root, spec, work)
        if skeleton in SOURCE_TREE_SKELETONS:
            assemble_source_tree(work, dest, values)
        else:
            copy_filled_work(work, dest)

    copy_eval(recipe, dest)
    write_origin(dest / "ORIGIN", version, commit, recipe_rel, skeleton)
    return dest


def all_importable_recipes(lintian_root: Path) -> list[str]:
    recipes: list[str] = []
    root = lintian_root / "t/recipes"
    for fill_values in root.glob("**/build-spec/fill-values"):
        skeleton = read_deb822(fill_values).get("Skeleton", "")
        if skeleton not in IMPORTABLE_SKELETONS:
            continue
        recipe = fill_values.parent.parent
        recipes.append(str(recipe.relative_to(root)))
    recipes.sort()
    return recipes


def write_pin(dest_root: Path, version: str, commit: str) -> None:
    dest_root.mkdir(parents=True, exist_ok=True)
    (dest_root / "PIN").write_text(f"lintian {version}\ncommit {commit}\n", encoding="utf-8")


def strip_location_pointer(extra: str) -> str:
    before, sep, pointer = extra.rpartition(" [")
    if sep and pointer.endswith("]"):
        return before
    return extra


def parse_universal_hint_line(line: str) -> tuple[str, str, str] | None:
    line = line.strip()
    if not line or line.startswith("#"):
        return None
    match = HINT_LINE.match(line)
    if not match:
        return None
    extra = strip_location_pointer((match.group("extra") or "").strip())
    return match.group("ptype"), match.group("tag"), extra


def applies_to_source_tree(processable_type: str, extra: str) -> bool:
    if processable_type != "source":
        return False
    parts = extra.split()
    return not parts or not parts[0].endswith(".dsc")


def applies_to_binary_package(processable_type: str) -> bool:
    return processable_type in {"binary", "udeb"}


def catalogued_ln_tags(rules_dir: Path) -> set[str]:
    tags: set[str] = set()
    if not rules_dir.is_dir():
        return tags
    for path in rules_dir.rglob("*.rs"):
        for line in path.read_text(encoding="utf-8").splitlines():
            match = TAG_ASSIGNMENT.match(line.strip())
            if match:
                tags.add(match.group(1))
    return tags


def recipe_is_source_tree(recipe: Path) -> bool:
    return (recipe / "debian").is_dir()


def recipe_is_deb_skeleton(recipe: Path) -> bool:
    origin = recipe / "ORIGIN"
    if not origin.is_file():
        return False
    text, _encoding = read_text(origin)
    return any(line.strip() == "Skeleton: deb" for line in text.splitlines())


def discover_vendored_recipe_dirs(dest_root: Path) -> list[Path]:
    if not dest_root.is_dir():
        return []
    recipes = [origin.parent for origin in dest_root.glob("**/ORIGIN")]
    recipes.sort()
    return recipes


def has_catalogued_source_tree_hints(recipe: Path, tags: set[str]) -> bool:
    hints = recipe / "eval" / "hints"
    if not hints.is_file():
        return False
    text, _encoding = read_text(hints)
    for line in text.splitlines():
        parsed = parse_universal_hint_line(line)
        if parsed is None:
            continue
        processable_type, tag, extra = parsed
        if tag in tags and applies_to_source_tree(processable_type, extra):
            return True
    return False


def catalogued_source_tree_recipe_rels(dest_root: Path, tags: set[str]) -> list[str]:
    rels: list[str] = []
    for recipe in discover_vendored_recipe_dirs(dest_root):
        if not recipe_is_source_tree(recipe):
            continue
        if not has_catalogued_source_tree_hints(recipe, tags):
            continue
        rels.append(recipe.relative_to(dest_root).as_posix())
    rels.sort()
    return rels


def has_catalogued_binary_package_hints(recipe: Path, tags: set[str]) -> bool:
    hints = recipe / "eval" / "hints"
    if not hints.is_file():
        return False
    text, _encoding = read_text(hints)
    for line in text.splitlines():
        parsed = parse_universal_hint_line(line)
        if parsed is None:
            continue
        processable_type, tag, _extra = parsed
        if tag in tags and applies_to_binary_package(processable_type):
            return True
    return False


def catalogued_binary_package_recipe_rels(dest_root: Path, tags: set[str]) -> list[str]:
    rels: list[str] = []
    for recipe in discover_vendored_recipe_dirs(dest_root):
        if not recipe_is_deb_skeleton(recipe):
            continue
        if not has_catalogued_binary_package_hints(recipe, tags):
            continue
        rels.append(recipe.relative_to(dest_root).as_posix())
    rels.sort()
    return rels


def recipe_test_name(rel: str) -> str:
    return rel.replace("/", "_").replace("-", "_")


def _const_slice(name: str, recipes: list[str]) -> str:
    recipe_lits = ",\n".join(f'    "{rel}"' for rel in recipes)
    if recipes:
        recipe_lits += ","
    return f"const {name}: &[&str] = &[\n{recipe_lits}\n];"


def _test_cases(recipes: list[str], fn_name: str, helper: str) -> str:
    if not recipes:
        return ""
    cases = "\n".join(f'#[test_case("{rel}"; "{recipe_test_name(rel)}")]' for rel in recipes)
    return f"""
{cases}
fn {fn_name}(recipe: &str) {{
    common::{helper}(recipe);
}}
"""


def render_parity_tests(source_tree: list[str], binary_package: list[str]) -> str:
    return f"""//! Generated by `scripts/import_lintian_parity.py`. Do not edit.
//!
//! Source-tree and Binary-package Parity recipes whose `eval/hints`
//! mention a catalogued LN Tag.

mod common;

use test_case::test_case;

{_const_slice("SOURCE_TREE_RECIPES", source_tree)}

{_const_slice("BINARY_PACKAGE_RECIPES", binary_package)}

#[test]
fn listed_source_tree_recipes_match_vendored_catalogued() {{
    let listed: Vec<String> = SOURCE_TREE_RECIPES.iter().map(|rel| (*rel).to_string()).collect();
    assert_eq!(common::catalogued_source_tree_recipe_rels(), listed);
}}

#[test]
fn listed_binary_package_recipes_match_vendored_catalogued() {{
    let listed: Vec<String> = BINARY_PACKAGE_RECIPES.iter().map(|rel| (*rel).to_string()).collect();
    assert_eq!(common::catalogued_binary_package_recipe_rels(), listed);
}}
{_test_cases(source_tree, "lintian_parity_source_tree", "assert_source_tree_parity")}{_test_cases(binary_package, "lintian_parity_binary_package", "assert_binary_package_parity")}
"""


def generated_tests_path() -> Path:
    return repo_root() / GENERATED_TESTS_RELATIVE


def write_parity_tests(dest_root: Path) -> Path:
    tags = catalogued_ln_tags(repo_root() / RULES_RELATIVE)
    source_tree = catalogued_source_tree_recipe_rels(dest_root, tags)
    binary_package = catalogued_binary_package_recipe_rels(dest_root, tags)
    path = generated_tests_path()
    path.write_text(render_parity_tests(source_tree, binary_package), encoding="utf-8")
    subprocess.run(["rustfmt", str(path)], check=False)
    return path


def git_clone(version: str, dest: Path) -> str:
    subprocess.run(
        ["git", "clone", "--depth", "1", "--branch", version, LINTIAN_GIT, str(dest)],
        check=True,
    )
    return git_head(dest)


def git_head(path: Path) -> str:
    result = subprocess.run(
        ["git", "-C", str(path), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?", help="lintian version tag (for example 2.139.0)")
    parser.add_argument("--lintian-src", type=Path, help="existing lintian source tree (skip clone)")
    parser.add_argument(
        "--output",
        type=Path,
        default=repo_root() / SUITE_RELATIVE,
        help="Parity suite root (default: packages/debmagic/tests/lintian-parity)",
    )
    parser.add_argument(
        "--write-tests",
        action="store_true",
        help="regenerate tests/lintian_parity.rs from vendored recipes (also runs after import)",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if not args.version:
        if args.write_tests:
            print(write_parity_tests(args.output))
            return 0
        print("pass a lintian version to import, or --write-tests", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory(prefix="debmagic-lintian-src-") as tmp:
        if args.lintian_src:
            lintian_root = args.lintian_src.resolve()
            commit = git_head(lintian_root) if (lintian_root / ".git").exists() else "unknown"
        else:
            lintian_root = Path(tmp) / "lintian"
            commit = git_clone(args.version, lintian_root)

        selected = all_importable_recipes(lintian_root)
        failed: list[tuple[str, BaseException]] = []
        for recipe_rel in selected:
            dest = args.output / Path(recipe_rel)
            try:
                dest = import_recipe(lintian_root, recipe_rel, args.output, args.version, commit)
                print(dest)
            except (OSError, subprocess.CalledProcessError, UnicodeDecodeError, ValueError) as error:
                failed.append((recipe_rel, error))
                print(f"FAIL {recipe_rel}: {error}", file=sys.stderr)
                if dest.exists() and not (dest / "ORIGIN").is_file():
                    shutil.rmtree(dest)
        write_pin(args.output, args.version, commit)
        print(write_parity_tests(args.output))
        if failed:
            print(f"{len(failed)} of {len(selected)} recipes failed", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
