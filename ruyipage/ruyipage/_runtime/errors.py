# -*- coding: utf-8 -*-
"""Runtime installer errors."""


class RuntimeErrorBase(Exception):
    """Base error for ruyiPage managed runtime operations."""


class UnsupportedPlatformError(RuntimeErrorBase):
    """Current platform has no bundled runtime asset."""


class RuntimeDownloadError(RuntimeErrorBase):
    """Runtime archive download failed."""


class RuntimeVerificationError(RuntimeErrorBase):
    """Runtime archive verification failed."""


class UnsafeArchiveError(RuntimeErrorBase):
    """Runtime archive contains unsafe entries."""


class RuntimeInstallError(RuntimeErrorBase):
    """Runtime install failed."""


class RuntimeNotInstalledError(RuntimeErrorBase):
    """Runtime is not installed."""
