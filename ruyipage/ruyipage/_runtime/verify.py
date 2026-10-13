# -*- coding: utf-8 -*-
"""Runtime archive verification helpers."""

import hashlib

from .errors import RuntimeVerificationError


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_sha256(path, expected):
    actual = sha256_file(path)
    expected = (expected or "").lower()
    if actual.lower() != expected:
        raise RuntimeVerificationError(
            "Firefox runtime SHA256 校验失败。\n"
            "文件: {}\n"
            "期望: {}\n"
            "实际: {}\n"
            "文件可能下载不完整，或被代理/网关替换。".format(
                path, expected, actual
            )
        )
    return actual
