#!/usr/bin/env python3
"""Point apt at a mirror by replacing the base image's default sources.

debmagic picks the base image itself, so the default sources file it ships
(classic `sources.list` or deb822 `*.sources`) is a known quantity: rather
than parsing and patching it in place, it is simply removed and replaced
with a single deb822 file that debmagic owns and fully controls. Unless
overridden, the suites/components enabled default to the release, its
`-updates` and its `-security` pockets - the same ones any base image
ships out of the box.
"""

from __future__ import annotations

import argparse
import glob
import os

OS_RELEASE_FILE = "/etc/os-release"

# Default apt configuration shipped by the base images debmagic uses;
# removed unconditionally in favour of MANAGED_SOURCES_FILE below.
DEFAULT_SOURCE_FILES = [
    "/etc/apt/sources.list",
    "/etc/apt/sources.list.d/ubuntu.sources",
    "/etc/apt/sources.list.d/debian.sources",
]

MANAGED_SOURCES_FILE = "/etc/apt/sources.list.d/debmagic.sources"

# Debian codenames with no separate updates/security/backports pockets.
DEBIAN_ROLLING_CODENAMES = {"unstable", "sid", "testing", "experimental"}
DEBIAN_WITHOUT_PROPOSED = {"unstable", "sid", "experimental"}


def read_os_release(path: str = OS_RELEASE_FILE) -> dict[str, str]:
    values: dict[str, str] = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            stripped_line = line.strip()
            if not stripped_line or stripped_line.startswith("#") or "=" not in stripped_line:
                continue
            key, _, value = stripped_line.partition("=")
            values[key] = value.strip().strip('"')
    return values


def default_suites(os_release: dict[str, str], codename: str) -> list[str]:
    if os_release.get("ID") == "debian" and codename in DEBIAN_ROLLING_CODENAMES:
        return [codename]
    return [codename, f"{codename}-updates", f"{codename}-security"]


def default_components(os_release: dict[str, str]) -> list[str]:
    if os_release.get("ID") == "ubuntu":
        return ["main", "restricted", "universe", "multiverse"]

    major = int((os_release.get("VERSION_ID") or "0").split(".")[0] or 0)
    if major >= 12:
        return ["main", "contrib", "non-free", "non-free-firmware"]
    return ["main", "contrib", "non-free"]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mirror",
        help="apt mirror base URI, e.g. http://archive.ubuntu.com/ubuntu/",
    )
    parser.add_argument(
        "--codename",
        help="target distribution codename; defaults to VERSION_CODENAME from the base image",
    )
    parser.add_argument(
        "--suite",
        dest="suites",
        action="append",
        help="suite/pocket to enable, e.g. noble or noble-security (repeatable); "
        "defaults to the release plus its '-updates' and '-security' pockets",
    )
    parser.add_argument(
        "--component",
        dest="components",
        action="append",
        help="component to enable, e.g. main or universe (repeatable); defaults to those enabled by the base image",
    )
    parser.add_argument(
        "--proposed",
        action="store_true",
        help="also enable the '<release>-proposed' pocket",
    )
    return parser.parse_args()


def iter_sources() -> list[tuple[str, list[str]]]:
    """Yield (uri, suites) from deb822 *.sources files and legacy sources.list."""
    sources: list[tuple[str, list[str]]] = []
    for path in glob.glob("/etc/apt/sources.list.d/*.sources"):
        uri: str | None = None
        suites: list[str] = []
        enabled = True
        with open(path, encoding="utf-8") as f:
            for line in [*f, "\n"]:
                stripped_line = line.strip()
                if not stripped_line:
                    if uri and suites and enabled:
                        sources.append((uri, suites))
                    uri, suites, enabled = None, [], True
                    continue
                key, _, value = stripped_line.partition(":")
                key, value = key.strip(), value.strip()
                if key == "URIs":
                    uri = value.split()[0]
                elif key == "Suites":
                    suites = value.split()
                elif key == "Enabled":
                    enabled = value.lower() != "no"
    for path in ["/etc/apt/sources.list", *glob.glob("/etc/apt/sources.list.d/*.list")]:
        try:
            f = open(path, encoding="utf-8")
        except FileNotFoundError:
            continue
        with f:
            for line in f:
                fields = line.split()
                if len(fields) >= 3 and fields[0] in {"deb", "deb-src"}:
                    fields = fields[1:]
                    if fields[0].startswith("["):
                        fields = fields[fields.index(next(f for f in fields if f.endswith("]"))) + 1 :]
                    if len(fields) >= 2:
                        sources.append((fields[0], fields[1:]))
    return sources


def configured_mirror(codename: str) -> str:
    for uri, suites in iter_sources():
        if codename in suites:
            return uri
    raise RuntimeError(f"could not determine the configured mirror for {codename}")


def proposed_suite(os_release: dict[str, str], codename: str) -> str:
    if os_release.get("ID") == "debian":
        if codename in DEBIAN_WITHOUT_PROPOSED:
            raise ValueError(f"Debian {codename} has no proposed-updates pocket")
        return f"{codename}-proposed-updates"
    return f"{codename}-proposed"


def main() -> None:
    args = parse_args()
    os_release = read_os_release()
    codename = args.codename or os_release["VERSION_CODENAME"]

    suites = args.suites or default_suites(os_release, codename)
    components = args.components or default_components(os_release)
    if args.proposed:
        suites = [*suites, proposed_suite(os_release, codename)]

    if not args.mirror:
        if not args.proposed:
            raise ValueError("--mirror or --proposed is required")
        suites = [proposed_suite(os_release, codename)]
        args.mirror = configured_mirror(codename)
        source_files = []
        source_groups = [(args.mirror, suites)]
    else:
        source_files = DEFAULT_SOURCE_FILES
        if os_release.get("ID") == "debian" and codename not in DEBIAN_ROLLING_CODENAMES:
            security_suite = f"{codename}-security"
            source_groups = [
                (args.mirror, [suite for suite in suites if suite != security_suite]),
                (configured_mirror(security_suite), [security_suite]),
            ]
        else:
            source_groups = [(args.mirror, suites)]

    for path in source_files:
        try:
            os.remove(path)
        except FileNotFoundError:
            pass

    os.makedirs(os.path.dirname(MANAGED_SOURCES_FILE), exist_ok=True)
    with open(MANAGED_SOURCES_FILE, "w", encoding="utf-8") as f:
        for index, (mirror, group_suites) in enumerate(source_groups):
            if index:
                f.write("\n")
            f.write("Types: deb deb-src\n")
            f.write(f"URIs: {mirror}\n")
            f.write(f"Suites: {' '.join(group_suites)}\n")
            f.write(f"Components: {' '.join(components)}\n")


if __name__ == "__main__":
    main()
