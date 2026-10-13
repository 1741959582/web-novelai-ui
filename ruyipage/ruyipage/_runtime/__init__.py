# -*- coding: utf-8 -*-
"""ruyiPage managed Firefox runtime helpers."""

from .installer import install, is_installed, get_executable_path
from .resolver import resolve_firefox_path

__all__ = [
    "install",
    "is_installed",
    "get_executable_path",
    "resolve_firefox_path",
]
