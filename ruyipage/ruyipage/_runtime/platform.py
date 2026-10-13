# -*- coding: utf-8 -*-
"""Platform detection for ruyiPage managed runtime."""

import platform as _platform
import sys

from .errors import UnsupportedPlatformError


def current_platform_key():
    """Return the runtime manifest platform key for the current machine."""
    machine = (_platform.machine() or "").lower()
    is_x64 = machine in ("amd64", "x86_64")

    if sys.platform == "win32" and is_x64:
        return "win64"
    if sys.platform.startswith("linux") and is_x64:
        return "linux-x86_64"

    raise UnsupportedPlatformError(
        "当前平台暂不支持 ruyiPage 配套 Firefox runtime。\n"
        "当前平台: {} / {}\n"
        "已支持平台: win64, linux-x86_64\n"
        "你仍然可以通过 launch(browser_path=...) 指定已有 Firefox。".format(
            sys.platform, machine or "unknown"
        )
    )
