# -*- coding: utf-8 -*-
"""Web扩展管理单元"""

from .._bidi import web_extension as bidi_ext


class ExtensionManager:
    """Web扩展管理器

    用法::

        page = FirefoxPage()
        ext_id = page.extensions.install('/path/to/extension.xpi')
        page.extensions.uninstall(ext_id)
    """

    def __init__(self, driver):
        self._driver = driver
        self._installed = {}  # {extension_id: path}

    def install(self, path):
        """安装扩展

        Args:
            path: 扩展路径（.xpi文件或解压目录）

        Returns:
            str: 扩展ID
        """
        result = bidi_ext.install(self._driver, path)
        if result is None:
            return ""
        ext_id = result.get("extension", "")
        self._installed[ext_id] = path
        return ext_id

    def install_dir(self, path):
        """安装解压目录形式的扩展（新手友好别名）。"""
        return self.install(path)

    def install_archive(self, path):
        """安装 .xpi / 压缩包形式的扩展（新手友好别名）。"""
        return self.install(path)

    def install_base64(self, value):
        """安装以 W3C base64 extensionData 表示的扩展归档。"""
        result = bidi_ext.install(self._driver, base64_value=value)
        if result is None:
            return ""
        ext_id = result.get("extension", "")
        self._installed[ext_id] = "<base64>"
        return ext_id

    def uninstall(self, extension_id):
        """卸载扩展

        Args:
            extension_id: 扩展ID
        """
        bidi_ext.uninstall(self._driver, extension_id)
        self._installed.pop(extension_id, None)

    def uninstall_all(self):
        """卸载所有已安装的扩展"""
        for ext_id in list(self._installed.keys()):
            self.uninstall(ext_id)

    @property
    def installed_extensions(self):
        """获取已安装扩展列表

        Returns:
            dict: {extension_id: path}
        """
        return dict(self._installed)
