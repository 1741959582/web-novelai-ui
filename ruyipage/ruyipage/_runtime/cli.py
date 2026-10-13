# -*- coding: utf-8 -*-
"""Command line interface for ruyiPage runtime management."""

import argparse
import json
import sys

from .errors import RuntimeErrorBase
from .installer import install, get_executable_path, install_status, uninstall


def main(argv=None):
    parser = argparse.ArgumentParser(prog="python -m ruyipage")
    sub = parser.add_subparsers(dest="command")

    install_parser = sub.add_parser("install", help="install the ruyiPage Firefox runtime")
    install_parser.add_argument("browser", nargs="?", default="firefox")
    install_parser.add_argument("--force", action="store_true", help="reinstall even if already installed")
    install_parser.add_argument("--dry-run", action="store_true", help="show install plan without downloading")
    install_parser.add_argument("--from-file", help="install from a local release archive")
    install_parser.add_argument("--install-dir", help="override browser install root")
    install_parser.add_argument("--base-url", help="override download base URL")
    install_parser.add_argument("--json", action="store_true", help="print JSON output")
    install_parser.add_argument("--quiet", action="store_true", help="reduce output")

    path_parser = sub.add_parser("path", help="print installed Firefox executable path")
    path_parser.add_argument("--install-dir", help="override browser install root")

    doctor_parser = sub.add_parser("doctor", help="show ruyiPage Firefox runtime status")
    doctor_parser.add_argument("--install-dir", help="override browser install root")
    doctor_parser.add_argument("--json", action="store_true")

    uninstall_parser = sub.add_parser("uninstall", help="remove the managed Firefox runtime")
    uninstall_parser.add_argument("--install-dir", help="override browser install root")
    uninstall_parser.add_argument("--yes", action="store_true", help="skip confirmation")

    args = parser.parse_args(argv)
    if not args.command:
        parser.print_help()
        return 0

    try:
        if args.command == "install":
            if args.browser != "firefox":
                raise RuntimeErrorBase("当前只支持安装 firefox runtime。")
            result = install(
                root=args.install_dir,
                force=args.force,
                from_file=args.from_file,
                base_url=args.base_url,
                dry_run=args.dry_run,
                quiet=args.quiet or args.json,
            )
            if args.json:
                print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
            else:
                _print_install_result(result)
            return 0

        if args.command == "path":
            print(get_executable_path(root=args.install_dir, strict=True))
            return 0

        if args.command == "doctor":
            result = install_status(root=args.install_dir)
            if args.json:
                print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
            else:
                _print_doctor(result)
            return 0 if result.get("installed") else 1

        if args.command == "uninstall":
            if not args.yes:
                print("This will remove the ruyiPage managed Firefox runtime. Use --yes to confirm.")
                return 1
            removed = uninstall(root=args.install_dir)
            print("Removed." if removed else "No runtime was installed.")
            return 0
    except RuntimeErrorBase as e:
        print(str(e), file=sys.stderr)
        return 1

    parser.print_help()
    return 1


def _print_install_result(result):
    if result.get("dry_run"):
        print("ruyiPage Firefox runtime install plan:")
    elif result.get("cached"):
        print("ruyiPage Firefox runtime is already installed:")
    else:
        print("ruyiPage Firefox runtime installed:")
    print("  version: {}".format(result["version"]))
    print("  release: {}".format(result["release"]))
    print("  platform: {}".format(result["platform"]))
    print("  path: {}".format(result["executable_path"]))
    if result.get("dry_run"):
        print("  url: {}".format(result["url"]))
        print("No files were downloaded because --dry-run was used.")


def _print_doctor(result):
    print("ruyiPage Firefox runtime doctor")
    print("  installed: {}".format("yes" if result.get("installed") else "no"))
    print("  version: {}".format(result["version"]))
    print("  release: {}".format(result["release"]))
    print("  platform: {}".format(result["platform"]))
    print("  path: {}".format(result["executable_path"]))
    if not result.get("installed"):
        print("\nRun:")
        print("  python -m ruyipage install")
