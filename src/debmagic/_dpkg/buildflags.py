import json
import os
import subprocess
from pathlib import Path

from .._package_version import PackageVersion
from .._utils import run_cmd


def _cmd(cmd: str, input_data: str | None = None, env: dict[str, str] | None = None, cwd: Path | None = None) -> str:
    return run_cmd(cmd, check=True, env=env, input=input_data, text=True, capture_output=True).stdout.strip()


def get_pkg_env(package_dir: Path, maint_options: str | None = None) -> tuple[dict[str, str], PackageVersion]:
    """
    does what including "/usr/share/dpkg/buildflags.mk" would do.
    """

    result: dict[str, str] = os.environ.copy()
    if maint_options is not None:
        result["DEB_BUILD_MAINT_OPTIONS"] = maint_options
        # TODO more vars as parameters, e.g. DEB_CFLAGS_MAINT_APPEND

    # get build flags
    flags_raw = _cmd("dpkg-buildflags", env=result)
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
        arch_flags_raw = _cmd("dpkg-architecture", env=result)
        for arch_flag_line in arch_flags_raw.splitlines():
            arch_flag_name, _, arch_flag_value = arch_flag_line.partition("=")
            result[arch_flag_name] = arch_flag_value

    # pkg-info.mk
    if result.get("DEB_SOURCE") is None or result.get("DEB_VERSION") is None:
        result["DEB_SOURCE"] = _cmd("dpkg-parsechangelog -SSource", env=result, cwd=package_dir)
        result["DEB_VERSION"] = _cmd("dpkg-parsechangelog -SVersion", env=result, cwd=package_dir)
        version = PackageVersion.from_str(result["DEB_VERSION"])

        # this would return DEB_VERSION in pkg-info.mk if no epoch is in version.
        # instead, we return "0" as oritinally intended if no epoch is in version.
        result["DEB_VERSION_EPOCH"] = version.epoch
        result["DEB_VERSION_EPOCH_UPSTREAM"] = version.epoch_upstream
        result["DEB_VERSION_UPSTREAM_REVISION"] = version.upstream_revision
        result["DEB_VERSION_UPSTREAM"] = version.upstream
        result["DEB_VERSION_REVISION"] = version.revision

        result["DEB_DISTRIBUTION"] = _cmd("dpkg-parsechangelog -SDistribution", env=result, cwd=package_dir)
        result["DEB_TIMESTAMP"] = _cmd("dpkg-parsechangelog -STimestamp", env=result, cwd=package_dir)

        if result.get("SOURCE_DATE_EPOCH") is None:
            result["SOURCE_DATE_EPOCH"] = result["DEB_TIMESTAMP"]
    else:
        version = PackageVersion.from_str(result["DEB_VERSION"])

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

    return result, version
