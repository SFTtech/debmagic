import json
import os
import re
import shlex
import subprocess
from pathlib import Path


def _run_output(cmd: str, input: str | None = None, env: dict[str, str] | None = None, cwd: Path | None = None) -> str:
    input_data = None
    if input:
        input_data = input.encode()
    return subprocess.check_output(shlex.split(cmd), input=input_data, env=env).strip().decode()


def get_flags(package_dir: Path, maint_options: str | None = None) -> dict[str, str]:
    """
    does what including "/usr/share/dpkg/buildflags.mk" would do.
    """

    result: dict[str, str] = os.environ.copy()
    if maint_options is not None:
        result["DEB_BUILD_MAINT_OPTIONS"] = maint_options
        # TODO more vars as parameters, e.g. DEB_CFLAGS_MAINT_APPEND

    # get build flags
    flags_raw = _run_output("dpkg-buildflags", env=result)
    for flag_line in flags_raw.splitlines():
        flag_name, _, flag_value = flag_line.partition("=")
        result[flag_name] = flag_value

    # ensure utility variables
    if result.get("DEB_BUILD_OS_RELEASE_ID") is None:
        result["DEB_BUILD_OS_RELEASE_ID"] = (
            subprocess.check_output(". /usr/lib/os-release && echo $ID", shell=True, env=result).decode().strip()
        )

    # architecture.mk
    if result.get("DEB_HOST_ARCH") is None:
        arch_flags_raw = _run_output("dpkg-architecture", env=result)
        for arch_flag_line in arch_flags_raw.splitlines():
            arch_flag_name, _, arch_flag_value = arch_flag_line.partition("=")
            result[arch_flag_name] = arch_flag_value

    # pkg-info.mk
    if result.get("DEB_SOURCE") is None or result.get("DEB_VERSION") is None:
        result["DEB_SOURCE"] = _run_output("dpkg-parsechangelog -SSource", env=result, cwd=package_dir)
        result["DEB_VERSION"] = _run_output("dpkg-parsechangelog -SVersion", env=result, cwd=package_dir)
        # this would return DEB_VERSION in pkg-info.mk if no epoch is in version.
        # instead, we return "0" as oritinally intended if no epoch is in version.
        result["DEB_VERSION_EPOCH"] = (
            "0" if ":" not in result["DEB_VERSION"] else re.sub(r"^([0-9]+):.*$", r"\1", result["DEB_VERSION"])
        )
        result["DEB_VERSION_EPOCH_UPSTREAM"] = re.sub(r"^(.*?)(-.*)?$", r"\1", result["DEB_VERSION"])
        result["DEB_VERSION_UPSTREAM_REVISION"] = re.sub(r"^([0-9]*:)?(.*?)$", r"\2", result["DEB_VERSION"])
        result["DEB_VERSION_UPSTREAM"] = re.sub(r"^([0-9]*:)(.*?)", r"\2", result["DEB_VERSION_EPOCH_UPSTREAM"])
        result["DEB_VERSION_REVISION"] = re.sub(r"^.*?-([^-]*)$", r"\1", result["DEB_VERSION"])
        result["DEB_DISTRIBUTION"] = _run_output("dpkg-parsechangelog -SDistribution", env=result, cwd=package_dir)
        result["DEB_TIMESTAMP"] = _run_output("dpkg-parsechangelog -STimestamp", env=result, cwd=package_dir)

        if result.get("SOURCE_DATE_EPOCH") is None:
            result["SOURCE_DATE_EPOCH"] = result["DEB_TIMESTAMP"]

    if result.get("ELF_PACKAGE_METADATA") is None:
        elf_meta = {
            "type": "deb",
            "os": subprocess.check_output(["awk", "-F=", "/^ID=/ {print $2}", "/etc/os-release"]).decode().strip(),
            "name": result.get("DEB_SOURCE") or result["DEB_SOURCE"],
            "version": result.get("DEB_VERSION") or result["DEB_VERSION"],
            "architecture": result.get("DEB_HOST_ARCH") or result["DEB_HOST_ARCH"],
        }
        if debug_url := result.get("DEB_BUILD_DEBUG_INFO_URL"):
            elf_meta["debugInfoUrl"] = debug_url

        result["ELF_PACKAGE_METADATA"] = json.dumps(elf_meta, separators=(",", ":"))

    return result
