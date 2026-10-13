# -*- coding: utf-8 -*-
"""Resolve Firefox executable paths for launch()."""

import os
import shutil
import sys

from .installer import get_executable_path
from .paths import env_executable_path


def resolve_firefox_path(explicit_path=None, allow_system=True):
    """Return a Firefox executable path or None.

    Resolution order:
    1. explicit path from API
    2. RUYIPAGE_FIREFOX_EXECUTABLE_PATH / RUYIPAGE_BROWSER_PATH
    3. managed ruyiPage runtime installed by python -m ruyipage install
    4. system Firefox fallback
    """
    if explicit_path:
        return _normalize_executable(explicit_path)

    env_path = env_executable_path()
    if env_path:
        return _normalize_executable(env_path)

    runtime_path = get_executable_path(strict=False)
    if runtime_path:
        return runtime_path

    if allow_system:
        return system_firefox_path()
    return None


def system_firefox_path():
    candidates = []
    if sys.platform == "win32":
        candidates.extend([
            r"C:\Program Files\Mozilla Firefox\firefox.exe",
            r"C:\Program Files (x86)\Mozilla Firefox\firefox.exe",
        ])
        found = shutil.which("firefox.exe") or shutil.which("firefox")
    elif sys.platform == "darwin":
        candidates.append("/Applications/Firefox.app/Contents/MacOS/firefox")
        found = shutil.which("firefox")
    else:
        candidates.extend(["/usr/bin/firefox", "/usr/local/bin/firefox", "/snap/bin/firefox"])
        found = shutil.which("firefox")

    for path in candidates:
        if os.path.isfile(path):
            return path
    return found or ("firefox" if sys.platform not in ("win32", "darwin") else None)


def _normalize_executable(path):
    path = os.path.expanduser(str(path))
    if os.path.isdir(path):
        exe_name = "firefox.exe" if sys.platform == "win32" else "firefox"
        path = os.path.join(path, exe_name)
    return path


def missing_firefox_message(path=None):
    checked = path or "未设置"
    return (
        "ruyiPage 找不到可用的 Firefox 浏览器。\n\n"
        "看起来你还没有安装 ruyiPage 配套 Firefox runtime，或系统 Firefox 不可用。\n\n"
        "请先运行:\n"
        "  python -m ruyipage install\n\n"
        "或者显式指定已有 Firefox 路径:\n"
        "  page = launch(browser_path=r\"C:\\Path\\To\\firefox.exe\")\n\n"
        "当前尝试的路径: {}"
    ).format(checked)
