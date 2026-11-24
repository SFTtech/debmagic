"""Ecosystem test for `debmagic lint`: run it on real Debian archive packages and diff it against the
lintian results the Debian infrastructure currently reports for the same package versions.

PROTOTYPE. Needs network access and `psql`; it is a measurement, not a CI gate (ADR 0012 keeps the
vendored lintian test-suite as the CI oracle; this complements it with real-world volume).

Oracle
    UDD's `lintian_results` table (public read-only mirror, udd-mirror.debian.net). It is the data
    behind the lintian link on tracker.debian.org (https://udd.debian.org/lintian/). Every row is one
    lintian hint for a (source, version), with the lintian version that produced it (`--report` prints it).
    Source-level hints have an empty `package`; binary hints carry package, version and architecture.

Packages
    The default set is `samba` (always) plus Debian's key packages (UDD `key_packages`, ~6.5k sources),
    of which a seeded sample of `--sample N` (default 25) is checked. `--all` checks the whole set
    (about 23k Subjects and 24 GB of downloads: a long run); `--pool archive` samples from every
    source package instead of the key ones; `--packages a b c` replaces the set.

    A sample is reproducible only while UDD's tables stay the same (new uploads change versions, key
    packages come and go), so a set to rerun later is pinned: `--write-package-list FILE` writes the
    selection (`--list-only` stops after it, with no downloads or lint), and `--package-list FILE`
    runs exactly that file. Lines are `source version` (a bare `source` takes whatever sid has now);
    the header records the UDD release, the date and the oracle's lintian version.
    `tests/ecosystem/packages-400-seed20261123.txt` is the committed 400-package sample.

Subjects (ADR 0016, 0017: one Subject per lint run)
    For each selected source package of the chosen release: the `.dsc` (Source package) and every
    `.deb` of the chosen architectures (Binary package), all fetched from the archive pool and
    checked against UDD's sha256 sums. Downloads overlap with linting; downloads and oracle
    snapshots are cached, so reruns only re-lint.

Comparison
    Only Tags debmagic implements (a Rule on the matching Subject kind) can be judged; every other
    oracle Tag is listed as *uncovered*, ranked by how many Subjects carry it. On covered Tags:

    - match       both sides report the Tag on the Subject
    - missed      lintian reports it, debmagic does not (false negative); includes a hint debmagic
                  suppressed with an override that lintian does not suppress
    - extra       debmagic reports it, lintian does not (false positive)
    - unapplied   an extra where lintian suppresses the Tag by an override and debmagic reports it as
                  a normal hint: "override not applied", a real finding of the override pass (ADR 0057)
    - overridden  both sides suppress the Tag by an override (debmagic's `O:` line, lintian's
                  `overridden`/`masked` row)

    debmagic runs with `--show-overrides` (see `--lint-arg`), so its overridden Diagnostics arrive as
    `O:` lines. `mismatched-override` and `unused-override` (LN1779, LN1780) come from the override pass;
    they are judged like any covered Tag once the Rust catalogue lists them (until then, uncovered).

    On matched Tags the hint contexts (`[path:line]`, ...) are compared as multisets, and the
    severity debmagic assigns is compared with lintian's tag type. Tag renames between lintian
    releases are folded onto the current name using the installed lintian's tag catalogue.

Agent work packets
    Besides the aggregate `report.md` / `report.json`, every Tag with a disagreement gets its own
    Markdown packet under `<work-dir>/rules/`: the Rule file, the counts, every affected Subject
    (not a truncated sample) with the artifact path, the differing hint lines and a copy-paste
    `debmagic lint --select <code>` command. `rules/index.md` ranks the packets by how many
    Subjects they affect, so a large run can be split across agents one packet per Rule.

Performance (`--perf`, on by default; `--perf off` skips it)
    Every Subject is also linted by the installed lintian, and wall time, CPU time (user+system of the
    process tree) and peak RSS are recorded per tool with `wait4`. Two lintian variants:

    - `lintian (same tags)`  `--tags-from-file` with exactly the Tags debmagic implements for that
                             Subject kind: comparable work, the headline number
    - `lintian (default)`    lintian's default profile, what users run today (more checks, so slower)

    Everything is downloaded before it is linted, so both tools read warm page-cache files. Each run
    starts a fresh process (cold lintian lab, no daemon). `--perf-repeat N` takes the median of N runs.
    Timings are only meaningful for a release build, which is the default build profile.

Groups (`--group on`; off by default)
    The UDD oracle lints every file alone: each `.deb` as a group of one installable, the `.dsc` with no
    installables. Evidence: lintian 2.144 cannot emit cannot-check-whether-usr-share-doc-symlink-points-
    to-foreign-package when the link target is in the group, yet UDD reports it for binaries whose
    target is a sibling .deb of the same source. So the default lints each Subject in its own run too.
    `--group on` lints each source package's `.dsc` and `.deb`s in one run (ADR 0054) and splits the
    output per Subject by `package (kind)`; its disagreements against UDD are expected for Group Rules,
    whose reference is lintian run on the same full group. A group over `--max-group-mb` falls back
    to one run per Subject.

Progress
    Each stage logs a line, and downloads and linting show a tqdm bar on stderr (`--progress off`
    for plain logs; a non-TTY disables the bars automatically). `-v` keeps a line per Subject.

Usage
    just lint-ecosystem                                   # samba + 25 key packages, correctness and perf
    just lint-ecosystem --packages hello dgit --perf off
    just lint-ecosystem --sample 100 --seed 7 --perf tags
    just lint-ecosystem --all --perf off                    # whole key-package pool, work packets for agents
    just lint-ecosystem --package-list tests/ecosystem/packages-400-seed20261123.txt --perf off
    just lint-ecosystem --sample 400 --seed 20261123 --list-only --write-package-list FILE
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import os
import re
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
from collections import Counter, defaultdict
from concurrent.futures import Future, ThreadPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any, Iterator

from tqdm import tqdm

sys.path.insert(0, str(Path(__file__).resolve().parent))

import lint_parity_report as parity

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_WORK_DIR = ROOT / "target" / "lint-ecosystem"
PROFILE_DIRS = {"release": "release", "debug": "debug"}
# The default package set: these are always checked, together with a sample of the key packages.
DEFAULT_PINNED = ("samba",)
DEFAULT_SAMPLE = 25
QUERY_CHUNK = 100  # (source, version) pairs per UDD query; psql takes the statement as one argv entry
TOOL_DEBMAGIC = "debmagic"
TOOL_LINTIAN_TAGS = "lintian (same tags)"
TOOL_LINTIAN_FULL = "lintian (default)"
SHORT_NAMES = {TOOL_DEBMAGIC: "debmagic", TOOL_LINTIAN_TAGS: "lintian/tags", TOOL_LINTIAN_FULL: "lintian/default"}
ARCHIVE_URL = "https://deb.debian.org/debian"
UDD_HOST = "udd-mirror.debian.net"
# Public, read-only credentials published at https://udd-mirror.debian.net/.
UDD_USER = "udd-mirror"
UDD_PASSWORD = "udd-mirror"

# lintian tag types that are visible output, versus ones lintian hides on purpose.
VISIBLE_TYPES = frozenset({"error", "warning", "information", "pedantic", "experimental"})
SUPPRESSED_TYPES = frozenset({"overridden", "masked"})
# `classification` hints are lintian bookkeeping, not diagnostics (ADR 0019).
# `O` is an overridden Diagnostic, printed by `debmagic lint --show-overrides` (ADR 0057).
OVERRIDDEN_SEVERITY = "overridden"
SEVERITY_LETTER = {"E": "error", "W": "warning", "I": "information", "P": "pedantic", "O": OVERRIDDEN_SEVERITY}

DIAGNOSTIC_LINE = re.compile(
    r"^(?P<sev>[EWIPO]): (?P<pkg>\S+) \((?P<kind>source|binary)\): (?P<tag>\S+) "
    r"\((?P<code>[A-Z]{2}\d{4})\)(?: (?P<context>.*))?$"
)
FINISHED_LINE = re.compile(r"^Finished in ", re.MULTILINE)
SAFE_LITERAL = re.compile(r"^[A-Za-z0-9.+:~_-]+$")


@dataclass
class Progress:
    """How this run renders progress: bars (unless `disable`) and a line per Subject (`verbose`).

    `disable=None` leaves the decision to tqdm (bars only when stderr is a TTY); `main` replaces it
    with a bool from `--progress`.
    """

    disable: bool | None = None
    verbose: bool = False

    def line(self, text: str) -> None:
        """A line above the bars, or a plain line when no bars are drawn."""
        if self.verbose or self.disable is not False:
            tqdm.write(text, file=sys.stderr)


PROGRESS = Progress()


def log(stage: str, message: str) -> None:
    """One progress line per stage, with a wall-clock stamp so a long run is readable after the fact."""
    print(f"[{time.strftime('%H:%M:%S')}] {stage}: {message}", file=sys.stderr, flush=True)


def bar(total: int | None, desc: str, unit: str = "it", **kwargs: Any) -> tqdm:
    """A progress bar on stderr over `total` units, disabled by `--progress off` or a non-TTY under `auto`."""
    return tqdm(
        total=total,
        desc=desc,
        unit=unit,
        file=sys.stderr,
        dynamic_ncols=True,
        **{"disable": PROGRESS.disable, **kwargs},
    )


def iter_bar[T](items: list[T], desc: str, unit: str = "it", **kwargs: Any) -> Iterator[T]:
    """Walk `items` behind a progress bar with the same enablement rules as `bar`."""
    yield from tqdm(
        items,
        desc=desc,
        unit=unit,
        file=sys.stderr,
        dynamic_ncols=True,
        **{"disable": PROGRESS.disable, **kwargs},
    )


@dataclass(frozen=True)
class ArchiveFile:
    name: str
    size: int
    sha256: str
    url: str


@dataclass
class SourceEntry:
    source: str
    version: str
    component: str
    files: list[ArchiveFile]
    binaries: list[BinaryEntry] = field(default_factory=list)

    @property
    def dsc(self) -> ArchiveFile:
        return next(f for f in self.files if f.name.endswith(".dsc"))

    @property
    def total_size(self) -> int:
        return sum(f.size for f in self.files)


@dataclass
class BinaryEntry:
    package: str
    version: str
    architecture: str
    file: ArchiveFile


@dataclass(frozen=True)
class OracleRow:
    package: str  # empty for source-level hints
    package_version: str
    architecture: str
    tag_type: str
    tag: str
    information: str


@dataclass(frozen=True)
class Diagnostic:
    severity: str  # error/warning/information/pedantic
    package: str
    kind: str
    tag: str
    code: str
    context: str


@dataclass(frozen=True)
class Metrics:
    """What one tool run cost: wall seconds, CPU seconds of the whole process tree, peak RSS in MB."""

    wall: float
    cpu: float
    rss_mb: float


@dataclass
class Timed:
    returncode: int
    output: str
    metrics: Metrics
    timed_out: bool = False


@dataclass
class RunResult:
    diagnostics: list[Diagnostic]
    ok: bool
    metrics: Metrics
    error: str = ""


@dataclass(frozen=True)
class SubjectKey:
    kind: str  # source | binary
    name: str
    version: str
    architecture: str = ""

    def label(self) -> str:
        arch = f":{self.architecture}" if self.architecture else ""
        return f"{self.kind} {self.name}{arch} {self.version}"


def sql_literal(value: str) -> str:
    """Quote a package name, version or release for SQL. Archive identifiers only; anything else is a bug."""
    if not SAFE_LITERAL.match(value):
        raise ValueError(f"refusing to put {value!r} into a query")
    return f"'{value}'"


def udd_query(sql: str) -> list[dict[str, str]]:
    env = {**os.environ, "PGPASSWORD": UDD_PASSWORD, "PGCONNECT_TIMEOUT": "30"}
    try:
        proc = subprocess.run(
            ["psql", "-h", UDD_HOST, "-U", UDD_USER, "-d", "udd", "-X", "-q", "--csv", "-c", sql],
            capture_output=True,
            text=True,
            env=env,
            timeout=600,
            check=False,
        )
    except FileNotFoundError:
        sys.exit("psql not found: install postgresql-client (the UDD mirror is a public Postgres database)")
    if proc.returncode != 0:
        sys.exit(f"UDD query failed:\n{proc.stderr.strip()}\n--\n{sql}")
    return list(csv.DictReader(io.StringIO(proc.stdout)))


def pool_dir(component: str, source: str) -> str:
    prefix = source[:4] if source.startswith("lib") else source[:1]
    return f"{ARCHIVE_URL}/pool/{component}/{prefix}/{source}"


def parse_checksums(sha256_field: str, base_url: str) -> list[ArchiveFile]:
    files = []
    for line in sha256_field.splitlines():
        parts = line.split()
        if len(parts) == 3:
            files.append(ArchiveFile(name=parts[2], size=int(parts[1]), sha256=parts[0], url=f"{base_url}/{parts[2]}"))
    return files


def chunks[T](items: list[T], size: int) -> list[list[T]]:
    return [items[i : i + size] for i in range(0, len(items), size)]


def names_filter(names: list[str] | tuple[str, ...], negate: bool = False) -> str:
    listed = ", ".join(sql_literal(n) for n in names)
    return f"s.source {'not in' if negate else 'in'} ({listed})"


# The oracle's lintian version, read in the same query. `lintian_results_idx` is (lintian_version, source,
# version): without a lintian_version predicate the (source, version) lookups below cannot use it and
# plan as a sequential scan of the 8.7M-row table per package (about 400 full scans). The mirror holds one
# lintian version today; the comparison is against that version only, and rows of other lintian versions
# are deliberately not compared. `max` is an index-only backward probe of one row.
ORACLE_LINTIAN_VERSION = "(select max(lintian_version) from lintian_results)"


def query_sources(release: str, where: list[str], order_limit: str = "") -> list[SourceEntry]:
    """Source packages of the release that have lintian results for their current version."""
    conditions = [
        "s.distribution = 'debian'",
        f"s.release = {sql_literal(release)}",
        "exists (select 1 from lintian_results l where l.lintian_version = "
        f"{ORACLE_LINTIAN_VERSION} and l.source = s.source and l.version = s.version)",
        *where,
    ]
    rows = udd_query(
        "select s.source, s.version, s.component, s.checksums_sha256 from sources s "
        f"where {' and '.join(conditions)} {order_limit}"
    )
    entries = []
    for row in rows:
        files = parse_checksums(row["checksums_sha256"], pool_dir(row["component"], row["source"]))
        if any(f.name.endswith(".dsc") for f in files):
            entries.append(SourceEntry(row["source"], row["version"], row["component"], files))
    return entries


def drop_oversized(entries: list[SourceEntry], max_mb: int) -> list[SourceEntry]:
    kept = []
    for entry in entries:
        if entry.total_size > max_mb * 1_000_000:
            print(f"skip {entry.source}: source is {entry.total_size / 1e6:.0f} MB", file=sys.stderr)
        else:
            kept.append(entry)
    return kept


Pin = tuple[str, str]  # (source, version); an empty version pins whatever sid has for the name


def read_package_list(path: Path) -> list[Pin]:
    """A package list file: `source [version]` per line, `#` starts a comment, blank lines are skipped."""
    pins: dict[Pin, None] = {}
    for number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        fields = raw.split("#", 1)[0].split()
        if not fields:
            continue
        if len(fields) > 2:
            sys.exit(f"{path}:{number}: expected `source [version]`, got {raw.strip()!r}")
        pin = (fields[0], fields[1] if len(fields) == 2 else "")
        for value in pin:
            if value and not SAFE_LITERAL.match(value):
                sys.exit(f"{path}:{number}: refusing package or version {value!r}")
        pins[pin] = None
    if not pins:
        sys.exit(f"{path}: no packages listed")
    return list(pins)


def pin_matches(pin: Pin, entry: SourceEntry) -> bool:
    return pin[0] == entry.source and pin[1] in ("", entry.version)


def pins_filter(pins: list[Pin]) -> str:
    """SQL for source packages named by a list: by name, and by (name, version) where the list gives one."""
    clauses = []
    named = [source for source, version in pins if not version]
    if named:
        clauses.append(names_filter(named))
    pairs = [(source, version) for source, version in pins if version]
    if pairs:
        listed = ", ".join(f"({sql_literal(source)}, {sql_literal(version)})" for source, version in pairs)
        clauses.append(f"(s.source, s.version) in ({listed})")
    return f"({' or '.join(clauses)})"


def select_pinned(args: argparse.Namespace) -> list[SourceEntry]:
    """The packages of `--package-list`, in file order. A pinned version no longer in sid is reported and skipped."""
    pins = read_package_list(args.package_list)
    log("select", f"resolving {len(pins)} listed source package(s) from {args.package_list} in {args.release} via UDD")
    found = query_sources(args.release, [pins_filter(pins)])
    for pin in pins:
        if not any(pin_matches(pin, entry) for entry in found):
            label = f"{pin[0]} {pin[1]}".strip()
            print(f"skip {label}: not in {args.release} or no lintian results for that version", file=sys.stderr)
    order = {pin: index for index, pin in enumerate(pins)}
    found.sort(key=lambda e: min(order[pin] for pin in pins if pin_matches(pin, e)))
    return drop_oversized(found, args.max_source_mb)


def select_sources(args: argparse.Namespace) -> list[SourceEntry]:
    """The packages to check: `--package-list`, `--packages`, else the pinned ones plus the (sampled) pool."""
    if args.package_list:
        return select_pinned(args)
    if args.packages:
        log("select", f"resolving {len(args.packages)} named source package(s) in {args.release} via UDD")
        entries = query_sources(args.release, [names_filter(args.packages)])
        for name in sorted(set(args.packages) - {e.source for e in entries}):
            print(f"skip {name}: not in {args.release} or no lintian results for its current version", file=sys.stderr)
        return drop_oversized(entries, args.max_source_mb)

    log("select", f"resolving pinned source package(s) {', '.join(DEFAULT_PINNED)} in {args.release} via UDD")
    pinned = query_sources(args.release, [names_filter(DEFAULT_PINNED)])
    for name in sorted(set(DEFAULT_PINNED) - {e.source for e in pinned}):
        print(f"skip {name}: not in {args.release} or no lintian results for its current version", file=sys.stderr)
    where = [names_filter(DEFAULT_PINNED, negate=True)]
    if args.pool == "key":
        where.append("s.source in (select source from key_packages)")
    # Over-fetch when sampling: oversized packages are dropped below. The order is a stable hash of the seed.
    order_limit = (
        ""
        if args.all
        else f"order by md5(s.source || {sql_literal(str(args.seed))}), s.source, s.version limit {args.sample * 4}"
    )
    scope = (
        "the whole key-package pool" if args.all else f"a sample of {args.sample} ({args.pool} pool, seed {args.seed})"
    )
    log("select", f"querying {scope} in {args.release} via UDD")
    pool = drop_oversized(query_sources(args.release, where, order_limit), args.max_source_mb)
    selected = drop_oversized(pinned, args.max_source_mb) + (pool if args.all else pool[: args.sample])
    log("select", f"{len(selected)} source package(s) selected")
    return selected


def describe_selection(args: argparse.Namespace) -> str:
    if args.package_list:
        return f"read from the list {args.package_list.name}"
    if args.packages:
        return f"--packages {' '.join(args.packages)}"
    if args.all:
        return f"--all (the whole {args.pool} pool)"
    return f"--sample {args.sample} --seed {args.seed} --pool {args.pool}"


def write_package_list(args: argparse.Namespace, entries: list[SourceEntry]) -> None:
    """Pin the selection as `source version` lines, with the snapshot it was made from in a header."""
    lintian = ", ".join(oracle_lintian_versions(entries, args.work_dir)) or "unknown"
    header = [
        "# debmagic lint ecosystem package list: one `source version` per line, `#` starts a comment.",
        f"# Selection: {describe_selection(args)}; release {args.release}; "
        f"sources over {args.max_source_mb} MB dropped.",
        f"# UDD snapshot: {time.strftime('%Y-%m-%d', time.gmtime())} (udd-mirror.debian.net, "
        f"release {args.release}); oracle lintian {lintian}.",
        "# Rerun: just lint-ecosystem --package-list <this file>",
        "# Regenerate: just lint-ecosystem --sample N --seed S --list-only --write-package-list <this file>",
        "# A pinned version sid no longer lists is skipped with a message. A bare name takes every version",
        "# sid lists for it (the Sources index can hold two, as for gcc-16).",
    ]
    body = [f"{e.source} {e.version}" for e in sorted(entries, key=lambda e: (e.source, e.version))]
    args.write_package_list.parent.mkdir(parents=True, exist_ok=True)
    args.write_package_list.write_text("\n".join([*header, *body]) + "\n", encoding="utf-8")
    log("select", f"wrote {len(body)} package(s) to {args.write_package_list}")


def attach_binaries(entries: list[SourceEntry], args: argparse.Namespace) -> None:
    archs = ", ".join(sql_literal(a) for a in ["all", *args.arch])
    by_source = {(e.source, e.version): e for e in entries}
    arch_label = ", ".join(["all", *args.arch])
    log("binaries", f"querying .debs of {len(entries)} source package(s) for architecture(s) {arch_label}")
    batches = chunks(entries, QUERY_CHUNK)
    for batch in iter_bar(batches, "binary metadata", unit="batch"):
        pairs = ", ".join(f"({sql_literal(e.source)}, {sql_literal(e.version)})" for e in batch)
        rows = udd_query(
            "select source, source_version, package, version, architecture, filename, size, sha256 from packages "
            f"where distribution = 'debian' and release = {sql_literal(args.release)} "
            f"and (source, source_version) in ({pairs}) and architecture in ({archs}) and filename like '%.deb'"
        )
        for row in rows:
            deb = ArchiveFile(
                name=Path(row["filename"]).name,
                size=int(row["size"]),
                sha256=row["sha256"],
                url=f"{ARCHIVE_URL}/{row['filename']}",
            )
            if deb.size > args.max_deb_mb * 1_000_000:
                print(f"skip {row['package']}: .deb is {deb.size / 1e6:.0f} MB", file=sys.stderr)
                continue
            binary = BinaryEntry(row["package"], row["version"], row["architecture"], deb)
            by_source[(row["source"], row["source_version"])].binaries.append(binary)


ORACLE_INFO_LIMIT = 1000  # UDD cuts lintian_results.information at this many characters


def dedupe_oracle_rows(rows: list[OracleRow]) -> list[OracleRow]:
    """Keep one copy of each exactly repeated row, in order. Lintian emits a hint once per package, so a repeat with
    every field equal is an artefact of the UDD snapshot. A row whose information reached ORACLE_INFO_LIMIT is kept
    however often it repeats: UDD cut the text there, so identical copies may still be distinct hints."""
    seen: set[OracleRow] = set()
    kept: list[OracleRow] = []
    for row in rows:
        if len(row.information) < ORACLE_INFO_LIMIT and row in seen:
            continue
        seen.add(row)
        kept.append(row)
    return kept


def load_oracle(entries: list[SourceEntry], work_dir: Path, refresh: bool) -> dict[tuple[str, str], list[OracleRow]]:
    """Oracle rows per (source, version), snapshotted under the work dir so a run can be redone offline. The
    snapshot keeps UDD's rows as returned; the duplicates are dropped on read (see dedupe_oracle_rows)."""
    snapshot_dir = work_dir / "oracle"
    snapshot_dir.mkdir(parents=True, exist_ok=True)
    oracle: dict[tuple[str, str], list[OracleRow]] = {}
    missing = []
    for entry in entries:
        path = snapshot_dir / f"{entry.source}_{entry.version.replace(':', '%3a')}.json"
        if path.exists() and not refresh:
            rows = [OracleRow(**row) for row in json.loads(path.read_text())["rows"]]
            oracle[(entry.source, entry.version)] = dedupe_oracle_rows(rows)
        else:
            missing.append(entry)
    cached = len(entries) - len(missing)
    log(
        "oracle",
        f"{cached} of {len(entries)} snapshot(s) cached"
        + (f", querying {len(missing)} via UDD" if missing else "")
        + (", --refresh-oracle forces a re-query" if refresh else ""),
    )
    batches = chunks(missing, QUERY_CHUNK)
    for batch in iter_bar(batches, "oracle snapshots", unit="batch") if missing else batches:
        pairs = ", ".join(f"({sql_literal(e.source)}, {sql_literal(e.version)})" for e in batch)
        rows = udd_query(
            "select source, version, lintian_version, package, package_version, architecture, tag_type, tag, "
            f"information from lintian_results where lintian_version = {ORACLE_LINTIAN_VERSION} "
            f"and (source, version) in ({pairs}) and tag_type <> 'classification'"
        )
        grouped: dict[tuple[str, str], list[dict[str, str]]] = defaultdict(list)
        for row in rows:
            grouped[(row["source"], row["version"])].append(row)
        for entry in batch:
            raw = grouped.get((entry.source, entry.version), [])
            parsed = [
                OracleRow(
                    r["package"], r["package_version"], r["architecture"], r["tag_type"], r["tag"], r["information"]
                )
                for r in raw
            ]
            versions = sorted({r["lintian_version"] for r in raw})
            path = snapshot_dir / f"{entry.source}_{entry.version.replace(':', '%3a')}.json"
            path.write_text(
                json.dumps(
                    {
                        "fetched": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                        "lintian": versions,
                        "rows": [asdict(r) for r in parsed],
                    }
                )
            )
            oracle[(entry.source, entry.version)] = dedupe_oracle_rows(parsed)
    return oracle


def oracle_lintian_versions(entries: list[SourceEntry], work_dir: Path) -> list[str]:
    versions: set[str] = set()
    for entry in entries:
        path = work_dir / "oracle" / f"{entry.source}_{entry.version.replace(':', '%3a')}.json"
        if path.exists():
            versions.update(json.loads(path.read_text()).get("lintian", []))
    return sorted(versions)


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fetch(file: ArchiveFile, directory: Path) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    dest = directory / file.name
    if dest.exists() and sha256_of(dest) == file.sha256:
        return dest
    part = dest.with_suffix(dest.suffix + ".part")
    request = urllib.request.Request(file.url, headers={"User-Agent": "debmagic-lint-ecosystem"})
    with urllib.request.urlopen(request, timeout=120) as response, part.open("wb") as out:
        while chunk := response.read(1 << 20):
            out.write(chunk)
    if sha256_of(part) != file.sha256:
        part.unlink()
        raise RuntimeError(f"sha256 mismatch for {file.url}")
    part.replace(dest)
    return dest


def subject_dir(work_dir: Path, entry: SourceEntry) -> Path:
    return work_dir / "archive" / f"{entry.source}_{entry.version.replace(':', '%3a')}"


def download_entry(entry: SourceEntry, work_dir: Path, progress: tqdm | None = None) -> dict[Path, str]:
    """Fetch the source files and every `.deb` of one source package; returns failures by path.

    `progress` is advanced once per file, whether it came from the cache or the network.
    """
    directory = subject_dir(work_dir, entry)
    failures: dict[Path, str] = {}
    for file in [*entry.files, *(b.file for b in entry.binaries)]:
        try:
            fetch(file, directory)
        except Exception as error:
            failures[directory / file.name] = str(error)
        finally:
            if progress is not None:
                progress.update(1)
                progress.set_postfix_str(entry.source)
    return failures


def start_downloads(
    entries: list[SourceEntry], work_dir: Path, workers: int
) -> tuple[ThreadPoolExecutor, dict[str, Future[dict[Path, str]]], tqdm]:
    """Download in the background, so linting the first package does not wait for the last one."""
    total = sum(len(e.files) + len(e.binaries) for e in entries)
    progress = bar(total, "downloads", unit="file", position=0)
    pool = ThreadPoolExecutor(max_workers=workers)
    return pool, {e.source: pool.submit(download_entry, e, work_dir, progress) for e in entries}, progress


def parse_diagnostics(output: str) -> list[Diagnostic]:
    diagnostics = []
    for line in output.splitlines():
        match = DIAGNOSTIC_LINE.match(line)
        if match:
            diagnostics.append(
                Diagnostic(
                    severity=SEVERITY_LETTER[match["sev"]],
                    package=match["pkg"],
                    kind=match["kind"],
                    tag=match["tag"],
                    code=match["code"],
                    context=(match["context"] or "").strip(),
                )
            )
    return diagnostics


def timed_run(cmd: list[str], timeout: float) -> Timed:
    """Run a command, merging stderr into stdout, and measure the whole process tree with `wait4`."""
    with tempfile.TemporaryFile() as sink:
        started = time.monotonic()
        proc = subprocess.Popen(
            cmd, stdout=sink, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, start_new_session=True
        )
        timed_out = threading.Event()

        def kill() -> None:
            timed_out.set()
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

        # Blocking in wait4 (no polling) keeps the wall time free of sleep granularity.
        timer = threading.Timer(timeout, kill)
        timer.start()
        try:
            _, status, usage = os.wait4(proc.pid, 0)
            wall = time.monotonic() - started
        finally:
            timer.cancel()
        proc.returncode = os.waitstatus_to_exitcode(status)
        sink.seek(0)
        output = sink.read().decode(errors="replace")
    metrics = Metrics(wall, usage.ru_utime + usage.ru_stime, usage.ru_maxrss / 1024)  # ru_maxrss is KiB on Linux
    return Timed(proc.returncode, output, metrics, timed_out.is_set())


def run_debmagic(debmagic: Path, subject: Path | list[Path], lint_args: list[str], timeout: int) -> RunResult:
    paths = subject if isinstance(subject, list) else [subject]
    run = timed_run([str(debmagic), "lint", "--color", "never", *lint_args, *(str(path) for path in paths)], timeout)
    if run.timed_out:
        return RunResult([], False, run.metrics, f"timed out after {timeout}s")
    # Exit 1 means "found errors" as well as "crashed" (ADR 0011); only the closing summary tells them apart.
    if not FINISHED_LINE.search(run.output):
        return RunResult([], False, run.metrics, run.output.strip()[-600:] or f"exit {run.returncode}, no output")
    return RunResult(parse_diagnostics(run.output), True, run.metrics)


def lintian_command(subject: Path, tags_file: Path | None) -> list[str]:
    # --no-cfg: ignore the user's and system's lintianrc, so every machine runs the same profile.
    # --fail-on none: lintian exits 2 for an error-level finding by default, which is the same code as a real
    # failure; with it off, any non-zero exit is a failure.
    command = ["lintian", "--no-cfg", "--fail-on", "none"]
    if tags_file is not None:
        command += ["--tags-from-file", str(tags_file)]
    return [*command, str(subject)]


def run_lintian(subject: Path, tags_file: Path | None, timeout: int) -> tuple[Metrics | None, str]:
    """Metrics of one lintian run, or (None, why). With `--fail-on none` lintian exits 0 whatever it finds."""
    run = timed_run(lintian_command(subject, tags_file), timeout)
    if run.timed_out:
        return None, f"timed out after {timeout}s"
    if run.returncode != 0:
        return None, run.output.strip()[-300:] or f"exit {run.returncode}"
    return run.metrics, ""


def lintian_tag_files(covered: dict[str, dict[str, str]], lintian_data: Path, directory: Path) -> dict[str, Path]:
    """Per Subject kind, a `--tags-from-file` file with the Tags debmagic implements and this lintian knows.

    An unknown tag name is fatal to lintian, so renamed-away or removed Tags are filtered out.
    """
    # Name-Spaced Tags (team checks) are known to lintian as `<check>/<tag>`, not by their bare name.
    known = (
        {t.tag: t.lintian_name for t in parity.load_lintian_tags(lintian_data).values()}
        if lintian_data.is_dir()
        else {}
    )
    directory.mkdir(parents=True, exist_ok=True)
    files = {}
    for kind, tags in covered.items():
        path = directory / f"tags-{kind}.txt"
        path.write_text("".join(f"{known[tag]}\n" for tag in sorted(tags) if tag in known))
        files[kind] = path
    return files


def median_metrics(runs: list[Metrics]) -> Metrics:
    return Metrics(
        statistics.median(r.wall for r in runs),
        statistics.median(r.cpu for r in runs),
        statistics.median(r.rss_mb for r in runs),
    )


@dataclass
class TagStats:
    tag: str
    code: str = ""
    match: int = 0
    missed: int = 0
    extra: int = 0
    unapplied: int = 0  # the `extra` ones where lintian suppresses the Tag by an override
    overridden: int = 0  # both sides suppress the Tag by an override
    instances_oracle: int = 0
    instances_debmagic: int = 0
    instances_matched: int = 0
    severity_mismatch: Counter[tuple[str, str]] = field(default_factory=Counter)
    examples: dict[str, list[str]] = field(default_factory=lambda: defaultdict(list))

    def note(self, bucket: str, text: str, limit: int) -> None:
        if len(self.examples[bucket]) < limit:
            self.examples[bucket].append(text)


@dataclass
class Uncovered:
    tag: str
    tag_type: str
    subjects: int = 0
    instances: int = 0


@dataclass(frozen=True)
class Disagreement:
    """One Tag's disagreement on one Subject, kept in full so a packet can be handed to an agent.

    `examples` on `TagStats` stays a truncated, human-readable sample for `report.md`; this is the
    complete record, with the artifact path needed to reproduce the run.
    """

    tag: str
    code: str
    bucket: str  # missed | extra | context | severity
    subject: str  # SubjectKey.label()
    kind: str
    path: str
    lintian: tuple[str, ...] = ()
    debmagic: tuple[str, ...] = ()
    note: str = ""


@dataclass
class PerfRecord:
    """One Subject measured with every enabled tool; only complete records enter the aggregates."""

    label: str
    kind: str
    input_mb: float
    runs: dict[str, Metrics]


@dataclass
class Comparison:
    stats: dict[str, TagStats] = field(default_factory=dict)
    uncovered: dict[str, Uncovered] = field(default_factory=dict)
    subjects: int = 0
    failures: list[tuple[str, str]] = field(default_factory=list)
    native_diagnostics: int = 0
    seconds: float = 0.0
    perf: list[PerfRecord] = field(default_factory=list)
    perf_failures: list[tuple[str, str, str]] = field(default_factory=list)
    disagreements: list[Disagreement] = field(default_factory=list)

    def note_disagreement(self, record: Disagreement) -> None:
        self.disagreements.append(record)


def canonical_tags(lintian_data: Path) -> dict[str, str]:
    """Old tag name -> current name, from the installed lintian's catalogue (empty if unavailable).

    Name-Spaced Tags (team checks) reach UDD as `<check>/<tag>` while debmagic reports the bare name, so the
    spaced name folds onto the bare one too."""
    if not lintian_data.is_dir():
        return {}
    tags = parity.load_lintian_tags(lintian_data).values()
    renames = {old: tag.tag for tag in tags for old in tag.renamed_from}
    renames.update({tag.lintian_name: tag.tag for tag in tags if tag.lintian_name != tag.tag})
    return renames


def covered_tags(root: Path, renames: dict[str, str]) -> dict[str, dict[str, str]]:
    """Tag -> Rule code for each Subject kind, from the Rust Rule catalogue.

    Every Applicability is kept, not only the `source` / `binary` Subjects this suite lints: the
    `changes`, `buildinfo` and `group` Rules share Tags with them and must not break the catalogue.
    """
    covered: dict[str, dict[str, str]] = defaultdict(dict)
    for rule in parity.parse_rust_rules(root, "LN"):
        for kind in rule.kinds:
            covered[kind][renames.get(rule.tag, rule.tag)] = rule.code
    return dict(covered)


def subject_oracle(key: SubjectKey, rows: list[OracleRow]) -> list[OracleRow]:
    if key.kind == "source":
        return [r for r in rows if not r.package]
    return [
        r
        for r in rows
        if r.package == key.name and r.package_version == key.version and r.architecture == key.architecture
    ]


def resolve_truncated(oracle: Counter[str], ours: Counter[str], truncated: Counter[str]) -> None:
    """Replace each oracle text that UDD cut at ORACLE_INFO_LIMIT with the debmagic text it is a prefix of.

    The cut lands anywhere, mid-word included, so a prefix match is all the oracle can confirm. Each debmagic
    text absorbs at most as many cut rows as it has copies left over after the exact matches."""
    spare = ours - oracle
    for text, count in truncated.items():
        for _ in range(count):
            full = next((t for t in sorted(spare) if spare[t] > 0 and t.startswith(text)), None)
            if full is None:
                break
            spare[full] -= 1
            oracle[text] -= 1
            oracle[full] += 1
    oracle += Counter()  # drop the zero counts


def compare_subject(
    comparison: Comparison,
    key: SubjectKey,
    oracle_rows: list[OracleRow],
    diagnostics: list[Diagnostic],
    covered: dict[str, str],
    renames: dict[str, str],
    example_limit: int,
    path: Path | None = None,
) -> None:
    visible: dict[str, Counter[str]] = defaultdict(Counter)
    visible_types: dict[str, set[str]] = defaultdict(set)
    suppressed: set[str] = set()
    truncated: dict[str, Counter[str]] = defaultdict(Counter)
    for row in oracle_rows:
        tag = renames.get(row.tag, row.tag)
        if row.tag_type in VISIBLE_TYPES:
            text = " ".join(row.information.split())
            visible[tag][text] += 1
            if len(row.information) >= ORACLE_INFO_LIMIT:
                truncated[tag][text] += 1
            visible_types[tag].add(row.tag_type)
        elif row.tag_type in SUPPRESSED_TYPES:
            suppressed.add(tag)

    actual: dict[str, Counter[str]] = defaultdict(Counter)
    overridden: dict[str, Counter[str]] = defaultdict(Counter)
    actual_severity: dict[str, set[str]] = defaultdict(set)
    codes: dict[str, str] = {}
    for diag in diagnostics:
        if diag.code.startswith("DM"):
            comparison.native_diagnostics += 1
            continue
        tag = renames.get(diag.tag, diag.tag)
        codes[tag] = diag.code
        text = " ".join(diag.context.split())
        if diag.severity == OVERRIDDEN_SEVERITY:
            overridden[tag][text] += 1
        else:
            actual[tag][text] += 1
            actual_severity[tag].add(diag.severity)

    for tag, texts in visible.items():
        resolve_truncated(texts, actual.get(tag, Counter()) + overridden.get(tag, Counter()), truncated[tag])

    judge_tags(
        comparison,
        key,
        str(path) if path is not None else "",
        Tags(visible, visible_types, suppressed, actual, overridden, actual_severity, codes),
        covered,
        example_limit,
    )


@dataclass(frozen=True)
class Tags:
    """One Subject's Tag views: what lintian says (visible and suppressed), what debmagic said (plain and
    overridden), their types and the Rule codes."""

    visible: dict[str, Counter[str]]
    visible_types: dict[str, set[str]]
    suppressed: set[str]
    actual: dict[str, Counter[str]]
    overridden: dict[str, Counter[str]]
    actual_severity: dict[str, set[str]]
    codes: dict[str, str]


def judge_tags(
    comparison: Comparison,
    key: SubjectKey,
    artifact: str,
    tags: Tags,
    covered: dict[str, str],
    example_limit: int,
) -> None:
    """Bucket one Subject's Tags and keep every disagreement in full for the agent packets.

    Each Tag is judged on two questions: does lintian show it, and does debmagic show it (plain) or hide
    it (overridden). Lintian's suppressed hints are the ones an override silenced there.
    """
    visible, visible_types, suppressed = tags.visible, tags.visible_types, tags.suppressed
    actual, overridden, codes = tags.actual, tags.overridden, tags.codes
    for tag in set(visible) | set(actual) | set(overridden) | suppressed:
        if tag not in covered and tag not in actual and tag not in overridden:
            if tag in visible:
                row = comparison.uncovered.setdefault(tag, Uncovered(tag, "/".join(sorted(visible_types[tag]))))
                row.subjects += 1
                row.instances += sum(visible[tag].values())
            continue
        stats = comparison.stats.setdefault(tag, TagStats(tag, covered.get(tag, codes.get(tag, ""))))
        label = key.label()
        in_oracle, in_actual = tag in visible, tag in actual
        hidden_here = tag in overridden
        if in_oracle and in_actual:
            judge_match(comparison, stats, key, artifact, label, tags, tag, example_limit)
        elif in_oracle:
            stats.missed += 1
            stats.instances_oracle += sum(visible[tag].values())
            why = " (debmagic suppressed it with an override)" if hidden_here else ""
            stats.note("missed", f"{label}: {sorted(visible[tag].elements())[:3]}{why}", example_limit)
            comparison.note_disagreement(
                Disagreement(
                    tag,
                    stats.code,
                    "missed",
                    label,
                    key.kind,
                    artifact,
                    lintian=tuple(sorted(visible[tag].elements())),
                    note="debmagic suppressed it with an override" if hidden_here else "",
                )
            )
        elif in_actual:
            stats.extra += 1
            stats.instances_debmagic += sum(actual[tag].values())
            unapplied = tag in suppressed
            if unapplied:
                stats.unapplied += 1
            note = "override not applied: lintian suppresses this Tag here" if unapplied else ""
            stats.note(
                "extra",
                f"{label}: {sorted(actual[tag].elements())[:3]}" + (f" ({note})" if note else ""),
                example_limit,
            )
            comparison.note_disagreement(
                Disagreement(
                    tag,
                    stats.code,
                    "extra",
                    label,
                    key.kind,
                    artifact,
                    debmagic=tuple(sorted(actual[tag].elements())),
                    note=note,
                )
            )
        elif tag in suppressed and hidden_here:
            stats.overridden += 1
        elif hidden_here:
            # Hidden by an override in debmagic, with no hint from lintian at all: a false positive that the
            # override hides from the user, so it still counts against the Rule.
            stats.extra += 1
            stats.instances_debmagic += sum(overridden[tag].values())
            hidden = sorted(overridden[tag].elements())[:3]
            stats.note("extra", f"{label}: {hidden} (hidden by an override)", example_limit)
            comparison.note_disagreement(
                Disagreement(
                    tag,
                    stats.code,
                    "extra",
                    label,
                    key.kind,
                    artifact,
                    debmagic=tuple(sorted(overridden[tag].elements())),
                    note="hidden by an override; lintian has no hint",
                )
            )


def judge_match(
    comparison: Comparison,
    stats: TagStats,
    key: SubjectKey,
    artifact: str,
    label: str,
    tags: Tags,
    tag: str,
    example_limit: int,
) -> None:
    """A Tag both sides show: compare the contexts and the severity."""
    visible, actual = tags.visible, tags.actual
    stats.match += 1
    both = visible[tag] & actual[tag]
    stats.instances_oracle += sum(visible[tag].values())
    stats.instances_debmagic += sum(actual[tag].values())
    stats.instances_matched += sum(both.values())
    if visible[tag] != actual[tag]:
        only_oracle = sorted((visible[tag] - actual[tag]).elements())
        only_debmagic = sorted((actual[tag] - visible[tag]).elements())
        stats.note(
            "context",
            f"{label}: lintian only {only_oracle[:3]}, debmagic only {only_debmagic[:3]}",
            example_limit,
        )
        comparison.note_disagreement(
            Disagreement(
                tag, stats.code, "context", label, key.kind, artifact, tuple(only_oracle), tuple(only_debmagic)
            )
        )
    if not tags.actual_severity[tag] & tags.visible_types[tag] and "experimental" not in tags.visible_types[tag]:
        lintian_type = "/".join(sorted(tags.visible_types[tag]))
        ours = "/".join(sorted(tags.actual_severity[tag]))
        stats.severity_mismatch[(lintian_type, ours)] += 1
        comparison.note_disagreement(
            Disagreement(
                tag,
                stats.code,
                "severity",
                label,
                key.kind,
                artifact,
                note=f"lintian {lintian_type}, debmagic {ours}",
            )
        )


def ratio_stats(values: list[float]) -> dict[str, float]:
    return {"median": statistics.median(values), "min": min(values), "max": max(values)}


def perf_summary(records: list[PerfRecord]) -> dict[str, dict[str, Any]]:
    """Aggregates per Subject kind and overall, over Subjects that every present tool measured.

    `speedup` is lintian's cost divided by debmagic's: 10 means debmagic needs a tenth of lintian's time.
    """
    summary: dict[str, dict[str, Any]] = {}
    for group in ("source", "binary", "all"):
        pool = [r for r in records if group in ("all", r.kind)]
        tools = [
            t for t in (TOOL_DEBMAGIC, TOOL_LINTIAN_TAGS, TOOL_LINTIAN_FULL) if pool and all(t in r.runs for r in pool)
        ]
        if not pool or TOOL_DEBMAGIC not in tools:
            continue
        per_tool: dict[str, dict[str, float]] = {}
        for tool in tools:
            runs = [r.runs[tool] for r in pool]
            per_tool[tool] = {
                "wall_total": sum(m.wall for m in runs),
                "wall_median": statistics.median(m.wall for m in runs),
                "cpu_total": sum(m.cpu for m in runs),
                "rss_median": statistics.median(m.rss_mb for m in runs),
                "rss_max": max(m.rss_mb for m in runs),
            }
        speedups: dict[str, dict[str, object]] = {}
        for tool in tools:
            if tool == TOOL_DEBMAGIC:
                continue
            ratios = [r.runs[tool].wall / max(r.runs[TOOL_DEBMAGIC].wall, 1e-9) for r in pool]
            speedups[tool] = {
                "wall_total": per_tool[tool]["wall_total"] / max(per_tool[TOOL_DEBMAGIC]["wall_total"], 1e-9),
                "cpu_total": per_tool[tool]["cpu_total"] / max(per_tool[TOOL_DEBMAGIC]["cpu_total"], 1e-9),
                "per_subject": ratio_stats(ratios),
            }
        summary[group] = {"subjects": len(pool), "tools": per_tool, "speedup": speedups}
    return summary


def render_perf(comparison: Comparison, summary: dict[str, dict[str, Any]], worst: int) -> list[str]:
    if not summary:
        return []
    lines = ["", "## Performance: debmagic vs lintian", ""]
    for group, data in summary.items():
        lines += [
            f"### {group} Subjects ({data['subjects']})",
            "",
            "| tool | wall total | wall median | CPU total | peak RSS median / max |",
            "| --- | ---: | ---: | ---: | ---: |",
        ]
        tools: dict[str, dict[str, float]] = data["tools"]
        for tool, m in tools.items():
            lines.append(
                f"| {tool} | {m['wall_total']:.1f}s | {m['wall_median']:.2f}s | {m['cpu_total']:.1f}s | "
                f"{m['rss_median']:.0f} / {m['rss_max']:.0f} MB |"
            )
        speedups: dict[str, dict[str, object]] = data["speedup"]
        lines.append("")
        for tool, sp in speedups.items():
            per: dict[str, float] = sp["per_subject"]
            lines.append(
                f"- debmagic vs {tool}: {sp['wall_total']:.1f}x less wall time in total, "
                f"{sp['cpu_total']:.1f}x less CPU; per Subject median {per['median']:.1f}x "
                f"(range {per['min']:.1f}x to {per['max']:.1f}x)"
            )
        lines.append("")

    # The Subjects where debmagic is relatively slowest: where a regression or a missing fast path would hide.
    reference = TOOL_LINTIAN_TAGS if any(TOOL_LINTIAN_TAGS in r.runs for r in comparison.perf) else TOOL_LINTIAN_FULL
    complete = [r for r in comparison.perf if TOOL_DEBMAGIC in r.runs and reference in r.runs]
    ranked = sorted(complete, key=lambda r: r.runs[reference].wall / max(r.runs[TOOL_DEBMAGIC].wall, 1e-9))
    if ranked:
        lines += [
            f"### Subjects where debmagic is relatively slowest (vs {reference})",
            "",
            "| Subject | input | debmagic | lintian | speedup |",
            "| --- | ---: | ---: | ---: | ---: |",
        ]
        for r in ranked[:worst]:
            d, other = r.runs[TOOL_DEBMAGIC], r.runs[reference]
            lines.append(
                f"| {r.label} | {r.input_mb:.1f} MB | {d.wall:.2f}s | {other.wall:.2f}s | "
                f"{other.wall / max(d.wall, 1e-9):.1f}x |"
            )
    if comparison.perf_failures:
        lines += ["", "### Perf runs excluded (tool failed)", ""]
        lines += [f"- {label} [{tool}]: {message}" for label, tool, message in comparison.perf_failures]
    return lines


def pct(part: int, whole: int) -> str:
    return "n/a" if whole == 0 else f"{100 * part / whole:.0f}%"


def render(comparison: Comparison, meta: dict[str, object], top_uncovered: int, top_slow: int = 5) -> str:
    lines = ["# debmagic lint ecosystem report", ""]
    lines += [f"- {key}: {value}" for key, value in meta.items()]
    stats = sorted(comparison.stats.values(), key=lambda s: (-(s.missed + s.extra), s.tag))
    total = Counter()
    for s in stats:
        total.update(match=s.match, missed=s.missed, extra=s.extra, unapplied=s.unapplied, overridden=s.overridden)
    lines += [
        "",
        f"Subjects compared: {comparison.subjects}; debmagic time {comparison.seconds:.0f}s; "
        f"native (DM) diagnostics ignored: {comparison.native_diagnostics}",
        f"Covered Tag x Subject hits: match {total['match']}, missed {total['missed']}, extra {total['extra']} "
        f"(override not applied {total['unapplied']}), overridden on both sides {total['overridden']}",
        f"Recall {pct(total['match'], total['match'] + total['missed'])}, "
        f"precision {pct(total['match'], total['match'] + total['extra'])}",
        "",
        "## Covered Tags",
        "",
        "| Tag | Code | match | missed | extra | unapplied | overridden (both) | contexts matched |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |",
    ]
    for s in stats:
        lines.append(
            f"| {s.tag} | {s.code} | {s.match} | {s.missed} | {s.extra} | {s.unapplied} | {s.overridden} | "
            f"{s.instances_matched}/{s.instances_oracle} (debmagic {s.instances_debmagic}) |"
        )

    disagreements = [s for s in stats if s.examples or s.severity_mismatch]
    if disagreements:
        lines += ["", "## Disagreements", ""]
        for s in disagreements:
            lines.append(f"### {s.tag} ({s.code})")
            for bucket, items in s.examples.items():
                lines += [f"- {bucket}:"] + [f"  - {item}" for item in items]
            for (oracle_type, ours), count in s.severity_mismatch.items():
                lines.append(f"- severity: lintian {oracle_type}, debmagic {ours} ({count} Subjects)")

    ranked = sorted(comparison.uncovered.values(), key=lambda u: (-u.subjects, u.tag))
    lines += [
        "",
        f"## Uncovered Tags (top {top_uncovered} by Subjects carrying them)",
        "",
        "| Tag | lintian type | Subjects | hints |",
        "| --- | --- | ---: | ---: |",
    ]
    lines += [f"| {u.tag} | {u.tag_type} | {u.subjects} | {u.instances} |" for u in ranked[:top_uncovered]]
    lines.append(f"\n{len(ranked)} distinct uncovered Tags in total.")

    lines += render_perf(comparison, perf_summary(comparison.perf), top_slow)

    if comparison.failures:
        lines += ["", "## debmagic failures (excluded from the numbers above)", ""]
        lines += [f"- {label}: {message}" for label, message in comparison.failures]
    return "\n".join(lines) + "\n"


def tool_version(argv: list[str], lines: int = 1) -> str:
    """The first `lines` non-empty lines of what a host tool prints for its version, joined by '; ';
    'not found' if it cannot run."""
    try:
        run = subprocess.run(argv, capture_output=True, text=True, check=False, timeout=20)
    except (OSError, subprocess.TimeoutExpired):
        return "not found"
    text = [line.strip() for line in (run.stdout or run.stderr).splitlines() if line.strip()]
    return "; ".join(text[:lines]) or "unknown"


def host_tool_versions() -> dict[str, str]:
    """The host tools whose output can change the hints (LN0270 quotes groff's messages), so a report must say which
    ones produced it. The debmagic commit is the tree the binary was built from."""
    commit = subprocess.run(
        ["git", "-C", str(ROOT), "describe", "--always", "--dirty"], check=False, capture_output=True, text=True
    )
    return {
        "groff": tool_version(["groff", "--version"]),
        "man-db": tool_version(["man", "--version"]),
        "file (libmagic)": tool_version(["file", "--version"], lines=2),
        "lintian (host)": tool_version(["lintian", "--version"]),
        "debmagic commit": commit.stdout.strip() or "unknown",
    }


def report_json(comparison: Comparison, host_tools: dict[str, str]) -> dict[str, Any]:
    return {
        "host_tools": host_tools,
        "subjects": comparison.subjects,
        "failures": comparison.failures,
        "tags": {
            s.tag: {
                "code": s.code,
                "match": s.match,
                "missed": s.missed,
                "extra": s.extra,
                "unapplied": s.unapplied,
                "overridden": s.overridden,
                "contexts": [s.instances_matched, s.instances_oracle, s.instances_debmagic],
                "examples": s.examples,
            }
            for s in comparison.stats.values()
        },
        "uncovered": [asdict(u) for u in comparison.uncovered.values()],
        "disagreements": [asdict(d) for d in comparison.disagreements],
        "perf": {
            "summary": perf_summary(comparison.perf),
            "subjects": [asdict(r) for r in comparison.perf],
            "failures": comparison.perf_failures,
        },
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(__doc__ or "").split("\n\n")[0], formatter_class=argparse.RawTextHelpFormatter
    )
    choice = parser.add_argument_group(
        "package selection", f"default: {', '.join(DEFAULT_PINNED)} plus a sample of {DEFAULT_SAMPLE} key packages"
    )
    choice.add_argument("--packages", nargs="+", metavar="SOURCE", help="check exactly these source packages")
    choice.add_argument("--sample", type=int, metavar="N", help=f"sample size from the pool (default {DEFAULT_SAMPLE})")
    choice.add_argument("--all", action="store_true", help="check the whole pool, not a sample (a long run)")
    choice.add_argument(
        "--pool", choices=["key", "archive"], default="key", help="where the sample comes from (default key packages)"
    )
    choice.add_argument("--seed", default="0", help="sample seed (default 0)")
    choice.add_argument(
        "--package-list",
        type=Path,
        metavar="FILE",
        help="check exactly the `source [version]` lines of FILE (a pinned set, see --write-package-list)",
    )
    choice.add_argument(
        "--write-package-list",
        type=Path,
        metavar="FILE",
        help="write the selected packages to FILE as a pinned `source version` list",
    )
    choice.add_argument(
        "--list-only",
        action="store_true",
        help="select (and write --write-package-list) without downloading or linting",
    )
    parser.add_argument("--release", default="sid", help="archive release to take packages from (default sid)")
    parser.add_argument(
        "--arch", nargs="+", default=["amd64"], help="binary architectures besides `all` (default amd64)"
    )
    parser.add_argument("--kinds", nargs="+", choices=["source", "binary"], default=["source", "binary"])
    parser.add_argument("--max-source-mb", type=int, default=30, help="skip sources larger than this (default 30)")
    parser.add_argument("--max-deb-mb", type=int, default=30, help="skip .debs larger than this (default 30)")
    parser.add_argument("--work-dir", type=Path, default=DEFAULT_WORK_DIR, help="cache and output directory")
    parser.add_argument("--refresh-oracle", action="store_true", help="re-query UDD instead of using the snapshot")
    parser.add_argument(
        "--profile",
        choices=list(PROFILE_DIRS),
        default="release",
        help="cargo profile to build and run (default release: debug timings are not representative)",
    )
    parser.add_argument("--debmagic", type=Path, help="debmagic binary (default target/<profile>/debmagic)")
    parser.add_argument("--no-build", action="store_true", help="do not `cargo build -p debmagic` first")
    parser.add_argument(
        "--lint-arg",
        action="append",
        default=None,
        help="extra argument for `debmagic lint`, repeatable (default: --select LN --show-overrides)",
    )
    parser.add_argument(
        "--lintian-data", type=Path, default=parity.DEFAULT_LINTIAN_DATA, help="lintian tag catalogue, for renames"
    )
    perf = parser.add_argument_group("performance comparison with lintian")
    perf.add_argument(
        "--perf",
        choices=["off", "tags", "all"],
        default="all",
        help="off: skip; tags: lintian restricted to the Tags debmagic implements; all: also lintian's default profile",
    )
    parser.add_argument(
        "--group",
        choices=["on", "off"],
        default="off",
        help=(
            "off (default): one debmagic run per Subject, as the UDD oracle runs lintian; on: one run per\n"
            "source package (ADR 0054), compared against the same per-file oracle (see Groups above)"
        ),
    )
    parser.add_argument(
        "--max-group-mb",
        type=int,
        default=200,
        help="with --group, lint a source package one Subject per run above this input size (default 200)",
    )
    perf.add_argument("--perf-repeat", type=int, default=1, metavar="N", help="runs per tool and Subject (median)")
    perf.add_argument("--lintian-timeout", type=int, default=1800, help="seconds per lintian run")
    perf.add_argument("--top-slow", type=int, default=5, help="Subjects listed where debmagic is relatively slowest")
    parser.add_argument("--timeout", type=int, default=300, help="seconds per debmagic run")
    parser.add_argument("--workers", type=int, default=4, help="parallel downloads")
    parser.add_argument("--examples", type=int, default=3, help="disagreement examples per Tag and kind")
    parser.add_argument("--top-uncovered", type=int, default=20)
    parser.add_argument(
        "--progress",
        choices=["auto", "off", "on"],
        default="auto",
        help="tqdm progress bars on stderr: auto draws them only on a TTY (default), off plain logs, on always",
    )
    parser.add_argument("-v", "--verbose", action="store_true", help="also log a line per Subject")
    parser.add_argument("--fail-on-diff", action="store_true", help="exit 1 when any covered Tag is missed or extra")
    args = parser.parse_args(argv)
    if args.packages and (args.all or args.sample is not None):
        parser.error("--packages replaces the default set; it cannot be combined with --all or --sample")
    if args.package_list and (args.packages or args.all or args.sample is not None):
        parser.error("--package-list replaces the default set; it cannot be combined with the other selections")
    if args.all and args.sample is not None:
        parser.error("--all and --sample are alternatives")
    if args.sample is None:
        args.sample = DEFAULT_SAMPLE
    if args.group == "on" and args.perf != "off":
        parser.error("--group on needs --perf off: performance is measured one Subject per run")
    args.group = args.group == "on"
    return args


def print_plan(entries: list[SourceEntry], kinds: list[str]) -> None:
    subjects = (len(entries) if "source" in kinds else 0) + sum(len(e.binaries) for e in entries)
    megabytes = sum(e.total_size + sum(b.file.size for b in e.binaries) for e in entries) / 1e6
    print(
        f"{len(entries)} source packages, {subjects} Subjects, {megabytes:.0f} MB in the archive "
        "(cached files are reused)",
        file=sys.stderr,
    )


def measure_perf(
    args: argparse.Namespace,
    debmagic: Path,
    lint_args: list[str],
    key: SubjectKey,
    path: Path,
    first: RunResult,
    tag_files: dict[str, Path],
    input_mb: float,
    comparison: Comparison,
) -> str:
    """Time lintian on the Subject (and debmagic again when repeating); returns a short progress note."""
    label = key.label()
    debmagic_runs = [first.metrics]
    for _ in range(args.perf_repeat - 1):
        again = run_debmagic(debmagic, path, lint_args, args.timeout)
        if again.ok:
            debmagic_runs.append(again.metrics)
    runs = {TOOL_DEBMAGIC: median_metrics(debmagic_runs)}
    variants: list[tuple[str, Path | None]] = [(TOOL_LINTIAN_TAGS, tag_files[key.kind])]
    if args.perf == "all":
        variants.append((TOOL_LINTIAN_FULL, None))
    for tool, tags_file in variants:
        collected: list[Metrics] = []
        for _ in range(args.perf_repeat):
            metrics, why = run_lintian(path, tags_file, args.lintian_timeout)
            if metrics is None:
                comparison.perf_failures.append((label, tool, why))
                break
            collected.append(metrics)
        else:
            runs[tool] = median_metrics(collected)
    if len(runs) == len(variants) + 1:
        comparison.perf.append(PerfRecord(label, key.kind, input_mb, runs))
    return ", ".join(f"{SHORT_NAMES[tool]} {m.wall:.2f}s" for tool, m in runs.items())


def subjects_of(entry: SourceEntry, kinds: list[str], work_dir: Path) -> list[tuple[SubjectKey, Path, float]]:
    """The Subjects of a source package: (key, file to lint, input size in MB)."""
    directory = subject_dir(work_dir, entry)
    subjects: list[tuple[SubjectKey, Path, float]] = []
    if "source" in kinds:
        subjects.append(
            (SubjectKey("source", entry.source, entry.version), directory / entry.dsc.name, entry.total_size / 1e6)
        )
    subjects += [
        (SubjectKey("binary", b.package, b.version, b.architecture), directory / b.file.name, b.file.size / 1e6)
        for b in entry.binaries
    ]
    return subjects


def rule_paths(root: Path, renames: dict[str, str]) -> dict[str, str]:
    """Tag -> Rule source path, from the Rust Rule catalogue (empty when lintian does not know the Tag)."""
    return {renames.get(rule.tag, rule.tag): rule.path for rule in parity.parse_rust_rules(root, "LN")}


def disagreement_packets(comparison: Comparison, root: Path, renames: dict[str, str]) -> dict[str, str]:
    """One Markdown packet per Tag with a disagreement, plus an `index.md` ranking them.

    A packet is self-contained: what the Rule is, how much it disagrees, and every affected Subject
    with the artifact path and hint lines, so an agent needs no other file to reproduce and fix it.
    """
    paths = rule_paths(root, renames)
    by_tag: dict[str, list[Disagreement]] = defaultdict(list)
    for record in comparison.disagreements:
        by_tag[record.tag].append(record)
    ordered = sorted(
        by_tag.items(),
        key=lambda item: (-len({(r.subject, r.bucket) for r in item[1]}), item[0]),
    )
    packets: dict[str, str] = {}
    index = [
        "# Ecosystem work packets",
        "",
        "One packet per Rule with a disagreement against the UDD lintian oracle. Fix the Rule,",
        "rerun its Subject with the command in the packet, then re-run the ecosystem suite.",
        "",
        "| Tag | Code | Rule | Subjects | missed | extra | context | severity |",
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for tag, records in ordered:
        code = next((r.code for r in records if r.code), "")
        subjects = {r.subject for r in records}
        buckets = Counter(r.bucket for r in records)
        name = f"{code or 'no-code'}_{tag}.md"
        rel = f"rules/{name}"
        packets[name] = render_packet(tag, code, records, paths.get(tag, ""), rel)
        index.append(
            f"| {tag} | {code or '?'} | {f'`{paths[tag]}`' if tag in paths else '?'} | {len(subjects)} | "
            f"{buckets['missed']} | {buckets['extra']} | {buckets['context']} | {buckets['severity']} |"
        )
    if packets:
        packets["index.md"] = "\n".join(index) + "\n"
    return packets


def source_of(path: str) -> str:
    """The source package of an archive Subject path (`<work-dir>/archive/<source>_<version>/<file>`)."""
    return Path(path).parent.name.split("_", 1)[0] if path else ""


def render_packet(tag: str, code: str, records: list[Disagreement], rule_path: str, rel: str) -> str:
    subjects = {r.subject for r in records}
    buckets = Counter(r.bucket for r in records)
    sources = sorted({source for r in records if (source := source_of(r.path))})
    lines = [
        f"# {tag} ({code or 'no code'})",
        "",
        f"- Rule: {f'`{rule_path}`' if rule_path else 'not in the Rule catalogue'}",
        f"- Disagreements: {len(subjects)} Subjects; missed {buckets['missed']}, extra {buckets['extra']} "
        f"(override not applied {sum(r.note.startswith('override not applied') for r in records)}), "
        f"context {buckets['context']}, severity {buckets['severity']}",
        "- Oracle: UDD `lintian_results` (what tracker.debian.org reports); see the parent `report.md`",
        f"- Packet: `{rel}`",
        "- Closure: the Rule's native tests cover the new behaviour, `just lint-parity` is unchanged, "
        "and the Subject below no longer disagrees (an `override not applied` extra is the override pass, "
        "ADR 0057, not the Rule)",
        "",
        "## Reproduce",
        "",
    ]
    for subject, path in sorted({(r.subject, r.path) for r in records if r.path}):
        lines.append(f"- `{subject}`: `debmagic lint --select {code or 'LN'} {path}`")
    if sources:
        lines += [
            "",
            "Re-run the comparison after the fix (its cache is warm):",
            "",
            "```shell",
            f"just lint-ecosystem --packages {' '.join(sources)} --perf off",
            "```",
        ]
    lines += ["", "## Subjects", ""]
    grouped: dict[str, list[Disagreement]] = defaultdict(list)
    for record in records:
        grouped[record.subject].append(record)
    for subject in sorted(grouped):
        lines += [f"### {subject}", ""]
        for record in grouped[subject]:
            if record.path:
                lines.append(f"- artifact: `{record.path}`")
            if record.bucket == "severity":
                lines.append(f"- severity: {record.note}")
                continue
            if record.note and record.bucket != "severity":
                lines.append(f"- note: {record.note}")
            if record.lintian:
                label = "lintian only" if record.bucket == "context" else "lintian"
                lines.append(f"- {label}:")
                lines += [f"  - {hint}" for hint in record.lintian]
            if record.debmagic:
                label = "debmagic only" if record.bucket == "context" else "debmagic"
                lines.append(f"- {label}:")
                lines += [f"  - {hint}" for hint in record.debmagic]
        lines.append("")
    return "\n".join(lines) + "\n"


def write_report(
    args: argparse.Namespace,
    comparison: Comparison,
    entries: list[SourceEntry],
    debmagic: Path,
    lint_args: list[str],
    renames: dict[str, str],
) -> str:
    meta: dict[str, object] = {
        "release": args.release,
        "lintian (oracle)": ", ".join(oracle_lintian_versions(entries, args.work_dir)) or "unknown",
        "oracle": f"UDD lintian_results via {UDD_HOST} (what tracker.debian.org links to)",
        "debmagic lint args": " ".join(lint_args),
        "debmagic binary": f"{debmagic} ({args.profile} profile)",
        "source packages": ", ".join(e.source for e in entries) if len(entries) <= 40 else len(entries),
    }
    host_tools = host_tool_versions()
    meta.update({f"host {name}": version for name, version in host_tools.items()})
    if args.perf != "off":
        installed = subprocess.run(["lintian", "--version"], capture_output=True, text=True, check=False).stdout.strip()
        meta["perf"] = f"{args.perf}, {args.perf_repeat} run(s) per tool, {os.cpu_count()} CPUs, {installed}"
        if args.profile == "debug":
            meta["perf warning"] = "debug build: debmagic timings are not representative"
    packets = disagreement_packets(comparison, ROOT, renames)
    if packets:
        meta["work packets (agents)"] = f"{len(packets) - 1} Rules, see rules/index.md"
    args.work_dir.mkdir(parents=True, exist_ok=True)
    (args.work_dir / "report.json").write_text(json.dumps(report_json(comparison, host_tools), indent=2))
    text = render(comparison, meta, args.top_uncovered, args.top_slow)
    (args.work_dir / "report.md").write_text(text)
    packet_dir = args.work_dir / "rules"
    if packet_dir.is_dir():
        shutil.rmtree(packet_dir)  # stale packets from an earlier run would look like current failures
    for name, body in packets.items():
        destination = packet_dir / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(body)
    return text


@dataclass
class Run:
    """What every Subject of this run needs: the tool, its arguments, and the Rule catalogue views."""

    args: argparse.Namespace
    debmagic: Path
    lint_args: list[str]
    renames: dict[str, str]
    covered: dict[str, dict[str, str]]
    tag_files: dict[str, Path]
    perf_on: bool


def build_debmagic(args: argparse.Namespace) -> Path:
    """Build (unless `--no-build`) and locate the debmagic binary; fail early on a missing tool."""
    if not args.no_build:
        log("build", f"cargo build -p debmagic ({args.profile})")
        cargo = ["cargo", "build", "-q", "-p", "debmagic", *(["--release"] if args.profile == "release" else [])]
        subprocess.run(cargo, cwd=ROOT, check=True)
        log("build", "done")
    debmagic = args.debmagic or ROOT / "target" / PROFILE_DIRS[args.profile] / "debmagic"
    if not debmagic.exists():
        sys.exit(f"{debmagic} not found; build it or pass --debmagic")
    if args.perf != "off" and shutil.which("lintian") is None:
        sys.exit("lintian not found: install it, or pass --perf off")
    return debmagic


GROUP_FAILED = SubjectKey("group", "", "")


def lint_group(
    run: Run, entry: SourceEntry, failures: dict[Path, str], comparison: Comparison
) -> dict[SubjectKey, RunResult] | None:
    """One debmagic run over the downloaded Subjects of `entry`, split back into a result per Subject by the
    `package (kind)` of each line. A failed run is returned under GROUP_FAILED for every Subject. None for a
    group over `--max-group-mb`: debmagic holds a whole group in memory (gcc-16-cross, 1.6 GB of .debs, needs
    more than 10 GB), so such a group is linted one Subject per run."""
    sized = subjects_of(entry, run.args.kinds, run.args.work_dir)
    if sum(mb for _, _, mb in sized) > run.args.max_group_mb:
        PROGRESS.line(f"{entry.source}: group over {run.args.max_group_mb} MB, linting one Subject per run")
        return None
    subjects = [(key, path) for key, path, _ in sized]
    paths = [path for _, path in subjects if path not in failures]
    if not paths:
        return {GROUP_FAILED: RunResult([], False, Metrics(0, 0, 0), "download failed")}
    result = run_debmagic(run.debmagic, paths, run.lint_args, run.args.timeout)
    comparison.seconds += result.metrics.wall
    if not result.ok:
        return {GROUP_FAILED: result}
    split: dict[SubjectKey, RunResult] = {GROUP_FAILED: RunResult([], False, result.metrics, "not in group output")}
    for key, _ in subjects:
        diagnostics = [d for d in result.diagnostics if d.kind == key.kind and d.package == key.name]
        split[key] = RunResult(diagnostics, True, result.metrics)
    return split


def lint_subjects(
    run: Run,
    entries: list[SourceEntry],
    oracle: dict[tuple[str, str], list[OracleRow]],
    pending: dict[str, Future[dict[Path, str]]],
) -> Comparison:
    """Lint every Subject, compare it with its oracle rows, and report progress as it goes."""
    args = run.args
    subjects = (len(entries) if "source" in args.kinds else 0) + sum(len(e.binaries) for e in entries)
    perf_note = " and lintian (perf)" if run.perf_on else ""
    log("lint", f"linting {subjects} Subject(s) with {' '.join(run.lint_args)}{perf_note}")
    comparison = Comparison()
    lint_bar = bar(subjects, "lint", unit="subject", position=1)
    for entry in entries:
        failures = pending[entry.source].result()
        rows = oracle[(entry.source, entry.version)]
        grouped = lint_group(run, entry, failures, comparison) if args.group else None
        for key, path, input_mb in subjects_of(entry, args.kinds, args.work_dir):
            lint_bar.set_postfix_str(key.label()[:60])
            try:
                if path in failures:
                    comparison.failures.append((key.label(), f"download: {failures[path]}"))
                    PROGRESS.line(f"skip {key.label()}: download failed")
                    continue
                if grouped is not None:
                    result = grouped.get(key) or grouped[GROUP_FAILED]
                else:
                    result = run_debmagic(run.debmagic, path, run.lint_args, args.timeout)
                    comparison.seconds += result.metrics.wall
                if not result.ok:
                    comparison.failures.append((key.label(), result.error))
                    PROGRESS.line(f"FAILED {key.label()}: {result.error.splitlines()[0][:120]}")
                    continue
                comparison.subjects += 1
                compare_subject(
                    comparison,
                    key,
                    subject_oracle(key, rows),
                    result.diagnostics,
                    run.covered[key.kind],
                    run.renames,
                    args.examples,
                    path,
                )
                note = f"debmagic {result.metrics.wall:.2f}s"
                if run.perf_on:
                    note = measure_perf(
                        args, run.debmagic, run.lint_args, key, path, result, run.tag_files, input_mb, comparison
                    )
                summary = f"{key.label()}: {len(result.diagnostics)} diagnostics, {note}"
                if PROGRESS.verbose or PROGRESS.disable is not False:
                    PROGRESS.line(summary)
                else:
                    lint_bar.set_postfix_str(summary[:80])
            finally:
                lint_bar.update(1)
    lint_bar.close()
    log(
        "lint",
        f"done: {comparison.subjects} Subject(s) compared, {len(comparison.failures)} failure(s), "
        f"debmagic {comparison.seconds:.0f}s, {len(comparison.disagreements)} disagreement(s) across "
        f"{len({d.tag for d in comparison.disagreements})} Tag(s)",
    )
    return comparison


def main(argv: list[str] | None = None) -> int:
    started = time.monotonic()
    args = parse_args(argv)
    PROGRESS.disable = args.progress == "off" or (args.progress == "auto" and not sys.stderr.isatty())
    PROGRESS.verbose = args.verbose
    log("start", f"lint ecosystem run into {args.work_dir} (progress: {args.progress})")
    debmagic = build_debmagic(args)
    lint_args = args.lint_arg if args.lint_arg is not None else ["--select", "LN", "--show-overrides"]

    entries = select_sources(args)
    if "binary" in args.kinds:
        attach_binaries(entries, args)
    print_plan(entries, args.kinds)
    oracle = load_oracle(entries, args.work_dir, args.refresh_oracle)
    if args.write_package_list:
        write_package_list(args, entries)
    if args.list_only:
        return 0
    downloads, pending, download_bar = start_downloads(entries, args.work_dir, args.workers)
    log("downloads", f"{download_bar.total} file(s) queued with {args.workers} worker(s)")

    renames = canonical_tags(args.lintian_data)
    covered = covered_tags(ROOT, renames)
    tag_files = lintian_tag_files(covered, args.lintian_data, args.work_dir / "perf") if args.perf != "off" else {}
    run = Run(args, debmagic, lint_args, renames, covered, tag_files, args.perf != "off")
    try:
        comparison = lint_subjects(run, entries, oracle, pending)
    finally:
        downloads.shutdown()
        download_bar.close()

    log("report", "writing report.md, report.json and the agent work packets")
    text = write_report(args, comparison, entries, debmagic, lint_args, renames)
    print(text)
    log("report", f"wrote {args.work_dir}/report.md and report.json")
    if (args.work_dir / "rules" / "index.md").exists():
        log("report", f"work packets for agents: {args.work_dir}/rules/index.md")
    log("done", f"finished in {time.monotonic() - started:.0f}s")
    differs = any(s.missed or s.extra for s in comparison.stats.values())
    return 1 if args.fail_on_diff and differs else 0


if __name__ == "__main__":
    sys.exit(main())
