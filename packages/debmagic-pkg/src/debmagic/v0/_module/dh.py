"""
use dh from debmagic.
this preset splits up the dh sequences (dh build, dh binary) into the debmagic stages.
so in theory, using this preset is the same as using dh.
"""

import shlex
from enum import StrEnum
from pathlib import Path
from typing import Callable

from debmagic.common.utils import list_strip_head, prefix_idx, run_cmd

from .._build import Build
from .._package import Package
from .._preset import Preset as PresetBase


class DHSequenceID(StrEnum):
    clean = "clean"
    build = "build"
    build_arch = "build-arch"
    build_indep = "build-indep"
    install = "install"
    install_arch = "install-arch"
    install_indep = "install-indep"
    binary = "binary"
    binary_arch = "binary-arch"
    binary_indep = "binary-indep"


type DHOverride = Callable[[Build], None]


class Preset(PresetBase):
    def __init__(self, dh_args: list[str] | str | None = None):
        self._dh_args: list[str]
        if dh_args is None:
            self._dh_args = []
        elif isinstance(dh_args, str):
            self._dh_args = shlex.split(dh_args)
        else:
            self._dh_args = dh_args

        self._overrides: dict[str, DHOverride] = {}
        self._initialized = False

        # debmagic's stages, with matching commands from the dh sequence
        self._clean_seq: list[str] = []
        self._configure_seq: list[str] = []
        self._build_seq: list[str] = []
        self._test_seq: list[str] = []
        self._install_seq: list[str] = []
        self._package_seq: list[str] = []

        # all seen sequence cmd ids (the dh command script itself)
        self._seq_ids: set[str] = set()

    def initialize(self, src_pkg: Package) -> None:
        # get all steps the dh sequence would do
        self._populate_stages(self._dh_args, base_dir=src_pkg.base_dir)
        self._initialized = True

    def clean(self, build: Build):
        self._run_dh_seq_cmds(build, self._clean_seq)

    def configure(self, build: Build):
        self._run_dh_seq_cmds(build, self._configure_seq)

    def build(self, build: Build):
        self._run_dh_seq_cmds(build, self._build_seq)

    def test(self, build: Build):
        self._run_dh_seq_cmds(build, self._test_seq)

    def install(self, build: Build):
        self._run_dh_seq_cmds(build, self._install_seq)

    def package(self, build: Build):
        self._run_dh_seq_cmds(build, self._package_seq)

    def override(self, func: DHOverride) -> DHOverride:
        """
        decorator to override a dh sequence command
        """
        name = func.__code__.co_name  # ty:ignore[unresolved-attribute]
        if name not in self._seq_ids:
            raise ValueError(f"dh sequence doesn't contain your override {name!r}")
        self._overrides[name] = func
        return func

    def _run_dh_seq_cmds(self, build: Build, seq_cmds: list[str]) -> None:
        """one line of dh output"""
        if not self._initialized:
            raise Exception("dh.Preset().initialize() was never called")

        for seq_cmd in seq_cmds:
            cmd = shlex.split(seq_cmd)
            seq_id = cmd[0]

            if override_fun := self._overrides.get(seq_id):
                override_fun(build)
            else:
                build.cmd(cmd, cwd=build.source_dir)

    def _populate_stages(self, dh_args: list[str], base_dir: Path) -> None:
        """
        split up the dh sequences into debmagic's stages.
        this involves guessing, since dh only has "build" (=configure, build, test)
        and "binary" (=install, package).

        if you have a better idea how to map dh sequences to debmagic's stages, please tell us.
        """
        ## clean, which is 1:1 fortunately
        self._clean_seq = self._get_dh_seq(base_dir, dh_args, DHSequenceID.clean)

        ## untangle "build" to configure & build & test
        build_seq_raw = self._get_dh_seq(base_dir, dh_args, DHSequenceID.build)
        if build_seq_raw[-1] != "create-stamp debian/debhelper-build-stamp":
            raise RuntimeError("build stamp creation line missing from dh build sequence")
        build_seq = build_seq_raw[:-1]  # remove that stamp line

        auto_cfg_idx = prefix_idx("dh_auto_configure", build_seq)
        # up to including dh_auto_configure
        self._configure_seq = build_seq[: auto_cfg_idx + 1]

        auto_test_idx = prefix_idx("dh_auto_test", build_seq)

        # start one after dh_auto_configure, up to one before dh_auto_test
        # because changes in build sequence are likely, I guess?
        # with this approach, we have to guess the sequence splitting, anyway.
        self._build_seq = build_seq[auto_cfg_idx + 1 : auto_test_idx]
        # assume test is just the rest
        self._test_seq = build_seq[auto_test_idx:]

        ## untangle "binary" to install & package
        install_seq = self._get_dh_seq(base_dir, dh_args, DHSequenceID.install)
        self._install_seq = list_strip_head(install_seq, build_seq_raw)
        binary_seq = self._get_dh_seq(base_dir, dh_args, DHSequenceID.binary)
        self._package_seq = list_strip_head(binary_seq, install_seq)

        # register all sequence items for validity checks
        for seq in (
            self._clean_seq,
            self._configure_seq,
            self._build_seq,
            self._test_seq,
            self._install_seq,
            self._package_seq,
        ):
            for seq_cmd in seq:
                cmd = shlex.split(seq_cmd)
                cmd_id = cmd[0]
                self._seq_ids.add(cmd_id)

    def _get_dh_seq(self, base_dir: Path, dh_args: list[str], seq: DHSequenceID) -> list[str]:
        cmd = ["dh", str(seq), "--no-act", *dh_args]
        proc = run_cmd(cmd, cwd=base_dir, capture_output=True, text=True)
        lines = proc.stdout.splitlines()
        return [line.strip() for line in lines]
