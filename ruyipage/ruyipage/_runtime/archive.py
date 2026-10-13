# -*- coding: utf-8 -*-
"""Safe archive extraction for runtime packages."""

import os
import re
import shutil
import stat
import tarfile
import zipfile

from .errors import UnsafeArchiveError, RuntimeInstallError

_WINDOWS_DRIVE_RE = re.compile(r"^[a-zA-Z]:[\\/]")


def _is_unsafe_name(name):
    if not name or name.startswith(("/", "\\")):
        return True
    if _WINDOWS_DRIVE_RE.match(name) or name.startswith("\\\\"):
        return True
    parts = name.replace("\\", "/").split("/")
    return any(part == ".." for part in parts)


def _safe_target(root, name):
    if _is_unsafe_name(name):
        raise UnsafeArchiveError("压缩包包含不安全路径: {}".format(name))
    root_real = os.path.realpath(root)
    target = os.path.realpath(os.path.join(root_real, name))
    if os.path.commonpath([root_real, target]) != root_real:
        raise UnsafeArchiveError("压缩包路径逃逸: {}".format(name))
    return target


def _check_limits(count, total_size, max_files, max_total_size):
    if count > max_files:
        raise UnsafeArchiveError("压缩包文件数量超过限制: {}".format(count))
    if total_size > max_total_size:
        raise UnsafeArchiveError("压缩包展开大小超过限制: {}".format(total_size))


def extract_archive(archive_path, dest_dir, archive_type, max_files=20000, max_total_size=900 * 1024 * 1024):
    if os.path.isdir(dest_dir):
        shutil.rmtree(dest_dir)
    os.makedirs(dest_dir)

    if archive_type == "zip":
        _extract_zip(archive_path, dest_dir, max_files, max_total_size)
    elif archive_type == "tar.xz":
        _extract_tar(archive_path, dest_dir, max_files, max_total_size)
    else:
        raise RuntimeInstallError("不支持的 Firefox runtime 压缩格式: {}".format(archive_type))


def _extract_zip(archive_path, dest_dir, max_files, max_total_size):
    total = 0
    with zipfile.ZipFile(archive_path) as zf:
        infos = zf.infolist()
        _check_limits(len(infos), 0, max_files, max_total_size)
        for info in infos:
            total += max(0, info.file_size)
            _check_limits(len(infos), total, max_files, max_total_size)
            target = _safe_target(dest_dir, info.filename)
            if info.is_dir():
                os.makedirs(target, exist_ok=True)
                continue
            os.makedirs(os.path.dirname(target), exist_ok=True)
            with zf.open(info) as src, open(target, "wb") as dst:
                shutil.copyfileobj(src, dst, length=1024 * 1024)


def _extract_tar(archive_path, dest_dir, max_files, max_total_size):
    total = 0
    count = 0
    with tarfile.open(archive_path, mode="r:*") as tf:
        for member in tf:
            count += 1
            if not (member.isfile() or member.isdir()):
                raise UnsafeArchiveError("tar 包包含不支持的特殊成员: {}".format(member.name))
            if member.issym() or member.islnk():
                raise UnsafeArchiveError("tar 包包含链接成员，已拒绝: {}".format(member.name))
            total += max(0, member.size)
            _check_limits(count, total, max_files, max_total_size)
            target = _safe_target(dest_dir, member.name)
            if member.isdir():
                os.makedirs(target, exist_ok=True)
                continue
            source = tf.extractfile(member)
            if source is None:
                raise UnsafeArchiveError("tar 包成员无法读取: {}".format(member.name))
            os.makedirs(os.path.dirname(target), exist_ok=True)
            with source, open(target, "wb") as dst:
                shutil.copyfileobj(source, dst, length=1024 * 1024)
            mode = member.mode & 0o777
            mode &= ~(stat.S_ISUID | stat.S_ISGID | stat.S_ISVTX)
            try:
                os.chmod(target, mode or 0o644)
            except OSError:
                pass
