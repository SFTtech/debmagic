#!/usr/bin/env python3
import shutil
import os

import sys
from pathlib import Path

packages_root = Path(__file__).parent.parent / "packages"
sys.path.append(str(packages_root / "debmagic-pkg" / "src"))

from debmagic.v0 import package, dh, Build

dhp = dh.Preset(dh_args=["--with", "python3", "--buildsystem=pybuild"])

pkg = package(
    preset=[dhp],
)

# TODO: add this section to our rust debmagic module to make setting this up less annoying
os.environ.update(
    {
        "PATH": f"/usr/share/cargo/bin:{os.environ['PATH']}",
        "CARGO": "/usr/share/cargo/bin/cargo",
        "CARGO_HOME": f"{pkg.base_dir}/debian/cargo_home",
        "CARGO_REGISTRY": f"{pkg.base_dir}/debian/cargo_registry",
        "DEB_CARGO_CRATE": f"{pkg.build_env.DEB_SOURCE}_{pkg.build_env.DEB_VERSION_UPSTREAM}",
        # debmagic-pkg is setuptools and ignores this. The rewrite below is what
        # makes resolution succeed; this only keeps maturin from enabling the
        # (now empty) default feature.
        "MATURIN_PEP517_ARGS": "--no-default-features",
    }
)

# Cargo resolves every feature name in the table, including ones that are not
# enabled, so `--no-default-features` still requires rusqlite to advertise
# `bundled`. Debian's librust-rusqlite-dev deletes that feature and links the
# system libsqlite3. Drop the reference for this build, then put the manifest
# back so a host or incremental tree is not left edited.
cargo_toml = Path("packages/debmagic/Cargo.toml")
bundled_sqlite = 'bundled-sqlite = ["rusqlite/bundled"]'
system_sqlite = "bundled-sqlite = []"


def use_system_sqlite() -> str:
    original = cargo_toml.read_text()
    if bundled_sqlite in original:
        cargo_toml.write_text(original.replace(bundled_sqlite, system_sqlite, 1))
        return original
    if system_sqlite in original:
        restored = original.replace(system_sqlite, bundled_sqlite, 1)
        return restored
    raise SystemExit(
        f"{cargo_toml}: expected {bundled_sqlite!r} so the Debian build can drop "
        "the rusqlite bundled feature that librust-rusqlite-dev removes"
    )


packages = {
    "debmagic": ("packages/debmagic", "debmagic"),
    "debmagic-pkg": ("packages/debmagic-pkg", "debmagic-pkg"),
}

cargo_lock = Path("Cargo.lock")
cargo_lock_saved = Path("Cargo.lock.saved")


def dh_auto(build: Build, stage: str, use_destdir: bool = False):
    for pkg_name, (path, python_pkg_name) in packages.items():
        destdir = f" --destdir debian/{pkg_name} " if use_destdir else ""
        build.cmd(
            f"{stage} -p {pkg_name} --sourcedirectory {path} --buildsystem=pybuild {destdir} -- --name {python_pkg_name}"
        )


@dhp.override
def dh_auto_configure(build: Build):
    build.cmd("cargo prepare-debian debian/cargo_registry --link-from-system", cwd="packages/debmagic")
    dh_auto(build, "dh_auto_configure")


@dhp.override
def dh_auto_build(build: Build):
    if cargo_lock.is_file():
        shutil.move(cargo_lock, cargo_lock_saved)
    dh_auto(build, "dh_auto_build")


@dhp.override
def dh_auto_install(build: Build):
    dh_auto(build, "dh_auto_install", use_destdir=True)


@dhp.override
def dh_auto_test(build: Build):
    dh_auto(build, "dh_auto_test")


@dhp.override
def dh_auto_clean(build: Build):
    if cargo_lock_saved.is_file():
        shutil.move(cargo_lock_saved, cargo_lock)
    dh_auto(build, "dh_auto_clean")


cargo_toml_original = use_system_sqlite()
try:
    pkg.pack()
finally:
    cargo_toml.write_text(cargo_toml_original)
