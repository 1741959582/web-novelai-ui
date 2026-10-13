# -*- coding: utf-8 -*-
"""FirefoxTab - 标签页控制器"""

import logging

from .firefox_base import FirefoxBase
from .._base.driver import ContextDriver
from .._bidi import browsing_context as bidi_context

logger = logging.getLogger('ruyipage')


class FirefoxTab(FirefoxBase):
    """Firefox 标签页

    通过 browser.new_tab()、browser.get_tab() 等方式获取。
    不要直接实例化。
    """

    _type = 'FirefoxTab'

    def _init_from_browser(self, browser, context_id):
        """从浏览器管理器初始化（内部方法）

        Args:
            browser: Firefox 实例
            context_id: browsingContext ID
        """
        super(FirefoxTab, self).__init__()
        self._init_context(browser, context_id)

    def activate(self) -> "FirefoxTab":
        """激活（聚焦）此标签页

        Returns:
            self
        """
        bidi_context.activate(self._driver._browser_driver, self._context_id)
        return self

    def close(self, others=False) -> None:
        """关闭标签页

        Args:
            others: True 关闭其他标签页，保留当前

        Returns:
            None
        """
        if others:
            self._browser.close_tabs(self._context_id, others=True)
        else:
            try:
                bidi_context.close(self._driver._browser_driver, self._context_id)
            except Exception:
                pass

    def save(self, path=None, name=None, as_pdf=False) -> str:
        """保存页面

        Args:
            path: 保存目录
            name: 文件名
            as_pdf: 保存为 PDF

        Returns:
            文件路径
        """
        import os

        if path is None:
            path = '.'
        if name is None:
            title = self.title or 'page'
            name = ''.join(c for c in title if c not in r'\/:*?"<>|')[:50]

        if as_pdf:
            file_path = os.path.join(path, name + '.pdf')
            self.pdf(file_path)
        else:
            file_path = os.path.join(path, name + '.html')
            html = self.html
            os.makedirs(path, exist_ok=True)
            with open(file_path, 'w', encoding='utf-8') as f:
                f.write(html)

        return file_path

    def __repr__(self):
        try:
            url = self.url
        except Exception:
            url = '?'
        return '<FirefoxTab {}>'.format(url[:60])
