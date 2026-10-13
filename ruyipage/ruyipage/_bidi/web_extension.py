# -*- coding: utf-8 -*-
"""BiDi webExtension 模块命令

Firefox 支持状态：
  ✅ install      (FF55+)
  ✅ uninstall    (FF55+)
"""

import logging
logger = logging.getLogger('ruyipage')


def install(driver, path=None, base64_value=None):
    """安装Web扩展

    Args:
        driver: BiDi driver
        path: 扩展文件路径（.xpi文件或解压目录）
        base64_value: base64 编码的扩展归档；与 path 二选一

    Returns:
        {'extension': str}  扩展ID 或 None（不支持时）

    Example:
        >>> from ruyipage._bidi import web_extension
        >>> result = web_extension.install(driver, '/path/to/extension.xpi')
        >>> extension_id = result['extension']
    """
    import os

    if (path is None) == (base64_value is None):
        raise ValueError("provide exactly one of path or base64_value")
    if base64_value is not None:
        params = {'extensionData': {'type': 'base64', 'value': base64_value}}
    elif os.path.isfile(path):
        params = {'extensionData': {'type': 'archivePath', 'path': path}}
    else:
        params = {'extensionData': {'type': 'path', 'path': path}}

    try:
        return driver.run('webExtension.install', params)
    except Exception as e:
        err_str = str(e).lower()
        if ('unknown command' in err_str or 'not supported' in err_str
                or 'unknown method' in err_str or 'invalid method' in err_str):
            logger.warning('webExtension.install 不受当前 Firefox 版本支持: %s', e)
            return None
        raise


def uninstall(driver, extension_id):
    """卸载Web扩展

    Args:
        driver: BiDi driver
        extension_id: 扩展ID（由install返回）

    Returns:
        空字典 {} 或 None（不支持时）

    Example:
        >>> web_extension.uninstall(driver, extension_id)
    """
    params = {'extension': extension_id}
    try:
        return driver.run('webExtension.uninstall', params)
    except Exception as e:
        err_str = str(e).lower()
        if ('unknown command' in err_str or 'not supported' in err_str
                or 'unknown method' in err_str or 'invalid method' in err_str):
            logger.warning('webExtension.uninstall 不受当前 Firefox 版本支持: %s', e)
            return None
        raise
