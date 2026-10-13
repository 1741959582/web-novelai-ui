# -*- coding: utf-8 -*-
"""Filesystem paths for the managed Firefox runtime."""

import os
import sys

ENV_BROWSERS_PATH = "RUYIPAGE_BROWSERS_PATH"
ENV_FIREFOX_EXECUTABLE = "RUYIPAGE_FIREFOX_EXECUTABLE_PATH"
ENV_BROWSER_PATH = "RUYIPAGE_BROWSER_PATH"


def _home_dir():
    return os.path.expanduser("~")


def browsers_root(path=None):
    """Return the root directory for managed browsers."""
    if path:
        return os.path.abspath(os.path.expanduser(str(path)))

    env_path = os.environ.get(ENV_BROWSERS_PATH)
    if env_path:
        return os.path.abspath(os.path.expanduser(env_path))

    if sys.platform == "win32":
        base = os.environ.get("LOCALAPPDATA") or os.path.join(_home_dir(), "AppData", "Local")
        return os.path.join(base, "ruyipage", "browsers")

    if sys.platform == "darwin":
        return os.path.join(_home_dir(), "Library", "Caches", "ruyipage", "browsers")

    base = os.environ.get("XDG_CACHE_HOME") or os.path.join(_home_dir(), ".cache")
    return os.path.join(base, "ruyipage", "browsers")


def download_dir(root=None):
    return os.path.join(browsers_root(root), ".downloads")


def tmp_dir(root=None):
    return os.path.join(browsers_root(root), ".tmp")


def env_executable_path():
    """Return a user-provided executable path from environment variables."""
    return os.environ.get(ENV_FIREFOX_EXECUTABLE) or os.environ.get(ENV_BROWSER_PATH)
