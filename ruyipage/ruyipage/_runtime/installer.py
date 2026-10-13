# -*- coding: utf-8 -*-
"""Install and locate the managed ruyiPage Firefox runtime."""

import json
import os
import shutil
import stat
import tempfile
from datetime import datetime, timezone

from .archive import extract_archive
from .downloads import download
from .errors import RuntimeInstallError, RuntimeNotInstalledError
from .manifest import RUNTIMES, runtime_url
from .paths import browsers_root, tmp_dir
from .platform import current_platform_key


def runtime_info(platform_key=None):
    key = platform_key or current_platform_key()
    info = dict(RUNTIMES[key])
    info["platform"] = key
    return info


def install(root=None, force=False, from_file=None, base_url=None, dry_run=False, quiet=False):
    """Install the managed Firefox runtime and return install metadata."""
    info = runtime_info()
    root_dir = browsers_root(root)
    install_dir = os.path.join(root_dir, info["install_subdir"])
    exe_path = os.path.join(install_dir, *info["executable"].split("/"))
    url = runtime_url(info, base_url=base_url)

    result = _metadata(info, install_dir, exe_path, url)
    if dry_run:
        result["dry_run"] = True
        return result

    if is_installed(root=root) and not force:
        result["cached"] = True
        return result

    if not os.path.isdir(root_dir):
        os.makedirs(root_dir)
    tmp_root = tmp_dir(root)
    if not os.path.isdir(tmp_root):
        os.makedirs(tmp_root)

    archive_path = from_file
    if archive_path:
        archive_path = os.path.abspath(os.path.expanduser(str(archive_path)))
        if not os.path.isfile(archive_path):
            raise RuntimeInstallError("离线安装文件不存在: {}".format(archive_path))
    else:
        archive_path = download(url, info["asset"], root=root, quiet=quiet)

    temp_install_dir = tempfile.mkdtemp(prefix=info["install_subdir"] + ".", dir=tmp_root)
    final_backup = None
    try:
        extract_archive(
            archive_path,
            temp_install_dir,
            info["archive_type"],
            max_files=info.get("max_files", 20000),
            max_total_size=info.get("max_total_size", 900 * 1024 * 1024),
        )
        temp_exe = os.path.join(temp_install_dir, *info["executable"].split("/"))
        if not os.path.isfile(temp_exe):
            raise RuntimeInstallError(
                "Firefox runtime 解压后未找到可执行文件: {}".format(info["executable"])
            )
        if os.name != "nt":
            try:
                mode = os.stat(temp_exe).st_mode
                os.chmod(temp_exe, mode | stat.S_IXUSR)
            except OSError:
                pass

        _write_install_json(temp_install_dir, info, archive_path, url)

        if os.path.exists(install_dir):
            final_backup = install_dir + ".old"
            if os.path.exists(final_backup):
                shutil.rmtree(final_backup, ignore_errors=True)
            os.replace(install_dir, final_backup)
        os.replace(temp_install_dir, install_dir)
        if final_backup:
            shutil.rmtree(final_backup, ignore_errors=True)
    except Exception:
        shutil.rmtree(temp_install_dir, ignore_errors=True)
        if final_backup and os.path.exists(final_backup) and not os.path.exists(install_dir):
            try:
                os.replace(final_backup, install_dir)
            except OSError:
                pass
        raise

    return _metadata(info, install_dir, exe_path, url)


def is_installed(root=None):
    try:
        return bool(get_executable_path(root=root, strict=False))
    except Exception:
        return False


def get_executable_path(root=None, strict=True):
    info = runtime_info()
    install_dir = os.path.join(browsers_root(root), info["install_subdir"])
    exe_path = os.path.join(install_dir, *info["executable"].split("/"))
    if os.path.isfile(exe_path):
        return exe_path
    if strict:
        raise RuntimeNotInstalledError(
            "ruyiPage Firefox runtime 尚未安装。\n"
            "请运行:\n"
            "  python -m ruyipage install"
        )
    return None


def install_status(root=None):
    info = runtime_info()
    root_dir = browsers_root(root)
    install_dir = os.path.join(root_dir, info["install_subdir"])
    exe_path = os.path.join(install_dir, *info["executable"].split("/"))
    return _metadata(info, install_dir, exe_path, runtime_url(info), installed=os.path.isfile(exe_path))


def uninstall(root=None):
    info = runtime_info()
    install_dir = os.path.join(browsers_root(root), info["install_subdir"])
    if os.path.isdir(install_dir):
        shutil.rmtree(install_dir)
        return True
    return False


def _metadata(info, install_dir, exe_path, url, installed=None):
    return {
        "name": info["name"],
        "version": info["version"],
        "release": info["release"],
        "platform": info["platform"],
        "asset": info["asset"],
        "url": url,
        "install_dir": install_dir,
        "executable_path": exe_path,
        "installed": os.path.isfile(exe_path) if installed is None else installed,
        "cached": False,
    }


def _write_install_json(install_dir, info, archive_path, url):
    data = {
        "name": info["name"],
        "version": info["version"],
        "release": info["release"],
        "platform": info["platform"],
        "asset": info["asset"],
        "url": url,
        "archive_path": os.path.abspath(archive_path),
        "executable": info["executable"],
        "installed_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    }
    with open(os.path.join(install_dir, "install.json"), "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2, sort_keys=True)
