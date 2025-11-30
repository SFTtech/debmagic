import argparse
from pathlib import Path

from debmagic._build_driver.build import PackageDescription, get_shell_in_build
from debmagic._build_driver.build import build as build_driver_build
from debmagic._build_driver.common import SUPPORTED_BUILD_DRIVERS
from debmagic._version import VERSION


def _create_parser() -> argparse.ArgumentParser:
    cli = argparse.ArgumentParser(description="Debmagic")
    sp = cli.add_subparsers(dest="operation")

    cli.add_argument("--version", action="version", version=f"%(prog)s {VERSION}")
    sp.add_parser("help", help="Show this help page and exit")
    sp.add_parser("version", help="Print the version information and exit")

    common_cli = argparse.ArgumentParser(add_help=False)
    common_cli.add_argument(
        "--dry-run", action="store_true", help="don't actually run anything that changes the system/package state"
    )

    build_cli = sp.add_parser(
        "build", parents=[common_cli], help="Build a debian package with the selected containerization driver"
    )
    build_cli.add_argument("--driver", choices=SUPPORTED_BUILD_DRIVERS, required=True)
    build_cli.add_argument("-s", "--source-dir", type=Path, default=Path.cwd())
    build_cli.add_argument("-o", "--output-dir", type=Path, default=Path.cwd())

    sp.add_parser("check", parents=[common_cli], help="Run linters (e.g. lintian)")

    shell_cli = sp.add_parser("shell", parents=[common_cli], help="Attach a shell to a running debmagic build")
    shell_cli.add_argument("-s", "--source-dir", type=Path, default=Path.cwd())

    sp.add_parser("test", parents=[common_cli], help="Run package tests")
    return cli


def main():
    cli = _create_parser()
    args = cli.parse_args()

    match args.operation:
        case "help":
            cli.print_help()
            cli.exit(0)
        case "version":
            print(f"{cli.prog} {VERSION}")
            cli.exit(0)
        case "shell":
            get_shell_in_build(package=PackageDescription(name="debmagic", version="0.1.0", source_dir=args.source_dir))
            cli.exit(0)
        case "build":
            build_driver_build(
                package=PackageDescription(name="debmagic", version="0.1.0", source_dir=args.source_dir),
                build_driver=args.driver,
                output_dir=args.output_dir,
                dry_run=args.dry_run,
            )
            cli.exit(0)
