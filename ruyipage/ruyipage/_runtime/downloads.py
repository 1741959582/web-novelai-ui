# -*- coding: utf-8 -*-
"""Download helpers for runtime archives."""

import os
import shutil
import tempfile
import urllib.request

from .errors import RuntimeDownloadError
from .paths import download_dir


def download(url, filename, root=None, timeout=120, quiet=False):
    """Download url into the managed download cache and return archive path."""
    dest_dir = download_dir(root)
    if not os.path.isdir(dest_dir):
        os.makedirs(dest_dir)
    final_path = os.path.join(dest_dir, filename)

    if os.path.isfile(final_path):
        return final_path

    fd, tmp_path = tempfile.mkstemp(prefix=filename + ".", suffix=".part", dir=dest_dir)
    os.close(fd)
    try:
        if not quiet:
            print("Downloading ruyiPage Firefox runtime:")
            print("  {}".format(url))
        with urllib.request.urlopen(url, timeout=timeout) as response, open(tmp_path, "wb") as out:
            shutil.copyfileobj(response, out, length=1024 * 1024)
        os.replace(tmp_path, final_path)
        return final_path
    except Exception as e:
        try:
            os.remove(tmp_path)
        except OSError:
            pass
        raise RuntimeDownloadError("下载 Firefox runtime 失败: {}".format(e))
