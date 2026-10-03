#!/usr/bin/env python3
"""Run dput-ng pre-upload hooks for debmagic's pre_upload_commands.

debmagic invokes this once per upload; it reads the .changes path and target
spec from the DEBMAGIC_UPLOAD_* environment variables.

With dput-ng installed, its own machinery runs the pre-upload hooks enabled
in the dput-ng profile named like the target (e.g. `ppa:user/repo`; pick
another with --profile), exactly as `dput` would.

Without dput-ng, a shim provides the python API hooks use and every
"pre": true hook found runs, importing hook modules from the scripts/ dirs.
Hooks and scripts resolve like dput-ng itself: /usr/share/dput-ng,
/etc/dput.d and ~/.dput.d, with later dirs overriding earlier ones per key.

Pass --config-dir to use different dirs. A hook failure (or any unexpected
error) exits non-zero, which makes debmagic abort the upload.

Usage in debmagic.toml:
[upload]
pre_upload_commands = ["python3 /usr/share/debmagic/dput_ng_hooks.py"]
"""

from __future__ import annotations

import argparse
import importlib
import importlib.util
import json
import os
import sys
from pathlib import Path
from types import ModuleType
from typing import Any

from debian import deb822

# order: later dirs override earlier ones per key
DEFAULT_CONFIG_DIRS = [
    Path("/usr/share/dput-ng"),
    Path("/etc/dput.d"),
    Path("~/.dput.d").expanduser(),
]

BUTTON_YES = "yes"
BUTTON_NO = "no"
BUTTON_CANCEL = "cancel"
BUTTON_OK = "ok"
ALL_BUTTONS = [BUTTON_YES, BUTTON_NO, BUTTON_CANCEL, BUTTON_OK]


class DputError(Exception):
    pass


class DcutError(Exception):
    pass


class DputConfigurationError(DputError):
    pass


class NoSuchConfigError(DputError):
    pass


class InvalidConfigError(DputError):
    pass


class ChangesFileException(DputError):  # noqa: N818 — name is fixed by the dput-ng API
    pass


class DscFileException(DputError):  # noqa: N818 — name is fixed by the dput-ng API
    pass


class UploadException(DputError):  # noqa: N818 — name is fixed by the dput-ng API
    pass


class HookException(DputError):  # noqa: N818 — name is fixed by the dput-ng API
    """Raised by hooks to abort the upload."""


class NoSuchHostError(DputError):
    pass


class Changes:
    """Dict-like view over the .changes file, as dput-ng hands to hooks."""

    def __init__(self, path: Path):
        self.path = path
        with path.open(encoding="utf-8") as f:
            self.paragraph = deb822.Changes(f)

    def __contains__(self, key: str) -> bool:
        return key in self.paragraph

    def __getitem__(self, key: str) -> str:
        return self.paragraph[key]

    def get(self, key: str, default: str | None = None) -> str | None:
        return self.paragraph.get(key, default)

    def get_changes_file(self) -> str:
        return str(self.path)

    def get_filename(self) -> str:
        return self.path.name

    def get_package_name(self) -> str:
        return self.paragraph["Source"]

    def get_dsc(self) -> str | None:
        return next((path for path in self.get_files() if path.endswith(".dsc")), None)

    def get_files(self) -> list[str]:
        """Absolute paths of the files referenced by the .changes."""
        return [str(self.path.parent / entry["name"]) for entry in self.paragraph["Files"]]


class Interface:
    """Stand-in for dput-ng's CLI interface, mirroring its semantics."""

    def initialize(self, **kwargs) -> None:
        pass

    def shutdown(self) -> None:
        pass

    def boolean(self, title: str, message: str, question_type=None, default=None) -> bool:
        if question_type is None:
            question_type = [BUTTON_YES, BUTTON_NO]
        choices = ", ".join(b.upper() if b == default else b for b in question_type)
        answer = self.question(title, f"{message} [{choices}]")
        answer = answer.lower()
        if not answer:
            return default in (BUTTON_OK, BUTTON_YES)
        for length in range(1, len(answer) + 1):
            buttons = [b for b in ALL_BUTTONS if b.startswith(answer[:length])]
            if not buttons:
                break
            if len(buttons) == 1:
                return buttons[0] in (BUTTON_OK, BUTTON_YES)
        return False

    def message(self, title: str, message: str, question_type=None) -> None:
        if title:
            print(f"{title}: ", end="")
        print(message)

    def list(self, title: str, message: str, selections=None):
        # dput's own CLI interface does not implement this either
        raise NotImplementedError

    def question(self, title: str, message: str, echo_input: bool = True) -> str:
        prompt = f"{title}: " if title else ""
        prompt += f"{message} "
        try:
            if echo_input:
                return input(prompt).strip()
            import getpass

            return getpass.getpass(prompt)
        except EOFError:
            # no interactive stdin: hooks decide what an empty answer means
            return ""

    def password(self, title: str, message: str) -> str:
        return self.question(title, message, echo_input=False)


def _shim_module(name: str) -> Any:
    """ModuleType typed as Any: shim modules grow arbitrary attributes by design."""
    return ModuleType(name)


def install_dput_shim() -> None:
    """Provide the dput modules hooks import, without dput-ng installed."""
    dput = _shim_module("dput")
    exceptions = _shim_module("dput.exceptions")
    for name, obj in globals().items():
        if isinstance(obj, type) and issubclass(obj, (DputError, DcutError)):
            setattr(exceptions, name, obj)
    dput.exceptions = exceptions

    core = _shim_module("dput.core")

    class _Logger:
        def __getattr__(self, level: str):
            def log(message: str, *args) -> None:
                print(f"dput hook: {level}: {message % args if args else message}", file=sys.stderr)

            return log

    core.logger = _Logger()
    dput.core = core

    interface = _shim_module("dput.interface")
    for name in ("BUTTON_YES", "BUTTON_NO", "BUTTON_CANCEL", "BUTTON_OK", "ALL_BUTTONS"):
        setattr(interface, name, globals()[name])
    dput.interface = interface

    sys.modules["dput"] = dput
    sys.modules["dput.exceptions"] = exceptions
    sys.modules["dput.core"] = core
    sys.modules["dput.interface"] = interface


def load_hooks(config_dirs: list[Path]) -> list[dict]:
    hooks: dict[str, dict] = {}
    for config_dir in config_dirs:
        hooks_dir = config_dir / "hooks"
        if not hooks_dir.is_dir():
            continue
        for path in sorted(hooks_dir.glob("*.json")):
            with path.open(encoding="utf-8") as f:
                hooks.setdefault(path.stem, {}).update(json.load(f))
    return [hook for hook in hooks.values() if hook.get("pre")]


def run_hook(hook: dict, changes: Changes, profile: dict, interface: Interface) -> None:
    # like dput.util.load_obj: "dput.hooks.lintian.lintian" is module + function
    module_name, function_name = hook["path"].rsplit(".", 1)
    module = importlib.import_module(module_name)
    getattr(module, function_name)(changes, profile, interface)


def run_dput_ng(changes_path: Path, profile_name: str, config_dirs: list[Path] | None) -> int:
    """Run the profile's pre-upload hooks through the installed dput-ng."""
    import dput.core  # ty: ignore[unresolved-import]
    from dput.changes import parse_changes_file  # ty: ignore[unresolved-import]
    from dput.exceptions import DputError as RealDputError  # ty: ignore[unresolved-import]
    from dput.hook import run_pre_hooks  # ty: ignore[unresolved-import]
    from dput.profile import load_profile  # ty: ignore[unresolved-import]

    if config_dirs:
        # dput-ng weighs its locations: the first dir has the lowest priority
        dput.core.CONFIG_LOCATIONS = {
            str(config_dir): 10 * (len(config_dirs) - i) for i, config_dir in enumerate(config_dirs)
        }

    try:
        profile = load_profile(profile_name)
    except RealDputError as e:
        print(f"debmagic: loading dput-ng profile {profile_name} failed, pick one with --profile: {e}", file=sys.stderr)
        return 1

    try:
        changes = parse_changes_file(str(changes_path), str(changes_path.parent))
        if "hooks" not in profile:
            print(f"debmagic: dput-ng profile {profile_name} enables no hooks")
            return 0
        run_pre_hooks(changes, profile)
    except RealDputError as e:
        print(f"debmagic: dput-ng hook failed for profile {profile_name}: {e}", file=sys.stderr)
        return 1
    return 0


def run_shim(changes_path: Path, target: str, config_dirs: list[Path]) -> int:
    """Run every pre-upload hook with the emulated dput-ng API."""
    install_dput_shim()
    for config_dir in config_dirs:
        scripts_dir = (config_dir / "scripts").resolve()
        if scripts_dir.is_dir() and str(scripts_dir) not in sys.path:
            sys.path.insert(0, str(scripts_dir))

    changes = Changes(changes_path)
    profile = {
        "name": target,
        "incoming": os.environ.get("DEBMAGIC_UPLOAD_TARGET_INCOMING", ""),
        "fqdn": os.environ.get("DEBMAGIC_UPLOAD_TARGET_SERVER", ""),
    }
    interface = Interface()

    for hook in load_hooks(config_dirs):
        description = hook.get("description", hook["path"])
        print(f"debmagic: running dput hook: {description}")
        try:
            run_hook(hook, changes, profile, interface)
        except HookException as e:
            print(e)
            return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--changes", default=os.environ.get("DEBMAGIC_UPLOAD_CHANGES"))
    parser.add_argument(
        "--target",
        default=os.environ.get("DEBMAGIC_UPLOAD_TARGET"),
        help="the upload target spec, e.g. ppa:user/repo",
    )
    parser.add_argument(
        "--profile",
        help="dput-ng profile whose hooks run, when dput-ng is installed (default: the target spec)",
    )
    parser.add_argument(
        "-c",
        "--config-dir",
        dest="config_dirs",
        action="append",
        type=Path,
        help="dput.d-style config dir with hooks/ and scripts/; repeatable, "
        "later dirs override earlier ones (default: /usr/share/dput-ng, /etc/dput.d, ~/.dput.d)",
    )
    args = parser.parse_args()

    if not args.changes or not args.target:
        parser.error("need --changes and --target (or DEBMAGIC_UPLOAD_* env vars)")

    changes = Path(args.changes).absolute()
    if importlib.util.find_spec("dput") is not None:
        return run_dput_ng(changes, args.profile or args.target, args.config_dirs)
    return run_shim(changes, args.target, args.config_dirs or DEFAULT_CONFIG_DIRS)


if __name__ == "__main__":
    sys.exit(main())
