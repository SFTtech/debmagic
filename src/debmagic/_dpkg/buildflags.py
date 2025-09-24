import os
import subprocess
import json
import re


def get_flags(maint_options: str | None = None) -> dict[str, str]:
    """
    does what including "/usr/share/dpkg/buildflags.mk" would do.
    """
    source_dir = os.getcwd()  # TODO get dynamically like in build.

    env = os.environ.copy()
    if maint_options is not None:
        env["DEB_BUILD_MAINT_OPTIONS"] = maint_options
        # TODO more vars as parameters, e.g. DEB_CFLAGS_MAINT_APPEND

    result: dict[str, str] = dict()

    # get build flags
    flags_raw = subprocess.check_output(["dpkg-buildflags"], env=env)
    for flag_line in flags_raw.decode().splitlines():
        flag_name, _, flag_value = flag_line.partition("=")
        result[flag_name] = flag_value

    # ensure utility variables
    if env.get("DEB_BUILD_OS_RELEASE_ID") is None:
        result["DEB_BUILD_OS_RELEASE_ID"] = (
            subprocess.check_output(
                ". /usr/lib/os-release && echo $ID", shell=True, env=env
            )
            .decode()
            .strip()
        )

    # architecture.mk
    if env.get("DEB_HOST_ARCH") is None:
        arch_flags_raw = subprocess.check_output(["dpkg-architecture"], env=env)
        for arch_flag_line in arch_flags_raw.decode().splitlines():
            arch_flag_name, _, arch_flag_value = arch_flag_line.partition("=")
            result[arch_flag_name] = arch_flag_value

    # pkg-info.mk
    if env.get("DEB_SOURCE") is None or env.get("DEB_VERSION") is None:
        result["DEB_SOURCE"] = (
            subprocess.check_output(
                ["dpkg-parsechangelog", "-SSource"], env=env, cwd=source_dir
            )
            .decode()
            .strip()
        )
        result["DEB_VERSION"] = (
            subprocess.check_output(
                ["dpkg-parsechangelog", "-SVersion"], env=env, cwd=source_dir
            )
            .decode()
            .strip()
        )
        # this returns the DEB_VERSION in pkg-info.mk if no epoch is present. here we return "0".
        result["DEB_VERSION_EPOCH"] = (
            "0"
            if ":" not in result["DEB_VERSION"]
            else re.sub(r"^([0-9]+):.*$", r"\1", result["DEB_VERSION"])
        )
        result["DEB_VERSION_EPOCH_UPSTREAM"] = (
            subprocess.check_output(
                ["sed", "-e", r"s/-[^-]*$//"], input=result["DEB_VERSION"].encode()
            )
            .decode()
            .strip()
        )
        result["DEB_VERSION_UPSTREAM_REVISION"] = (
            subprocess.check_output(
                ["sed", "-e", r"s/^[0-9]*://"], input=result["DEB_VERSION"].encode()
            )
            .decode()
            .strip()
        )
        result["DEB_VERSION_UPSTREAM"] = (
            subprocess.check_output(
                ["sed", "-e", r"s/^[0-9]*://"],
                input=result["DEB_VERSION_EPOCH_UPSTREAM"].encode(),
            )
            .decode()
            .strip()
        )
        result["DEB_VERSION_REVISION"] = (
            subprocess.check_output(
                ["sed", "-e", r"s/^.*-\([^-]*\)$/\1/"],
                input=result["DEB_VERSION"].encode(),
            )
            .decode()
            .strip()
        )
        result["DEB_DISTRIBUTION"] = (
            subprocess.check_output(
                ["dpkg-parsechangelog", "-SDistribution"], env=env, cwd=source_dir
            )
            .decode()
            .strip()
        )
        result["DEB_TIMESTAMP"] = (
            subprocess.check_output(
                ["dpkg-parsechangelog", "-STimestamp"], env=env, cwd=source_dir
            )
            .decode()
            .strip()
        )

        if env.get("SOURCE_DATE_EPOCH") is None:
            result["SOURCE_DATE_EPOCH"] = result["DEB_TIMESTAMP"]

    if env.get("ELF_PACKAGE_METADATA") is None:
        elf_meta = {
            "type": "deb",
            "os": subprocess.check_output(
                ["awk", "-F=", "/^ID=/ {print $2}", "/etc/os-release"]
            )
            .decode()
            .strip(),
            "name": result.get("DEB_SOURCE") or env["DEB_SOURCE"],
            "version": result.get("DEB_VERSION") or env["DEB_VERSION"],
            "architecture": result.get("DEB_HOST_ARCH") or env["DEB_HOST_ARCH"],
        }
        if debug_url := env.get("DEB_BUILD_DEBUG_INFO_URL"):
            elf_meta["debugInfoUrl"] = debug_url

        result["ELF_PACKAGE_METADATA"] = json.dumps(elf_meta, indent=4)

    return result
