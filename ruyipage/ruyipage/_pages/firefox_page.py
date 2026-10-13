# -*- coding: utf-8 -*-
"""FirefoxPage - 顶层页面控制器

提供简洁易用的页面控制 API。
"""

import logging
import time
from typing import TYPE_CHECKING

from .firefox_base import FirefoxBase

if TYPE_CHECKING:
    from .firefox_tab import FirefoxTab

from .._base.browser import Firefox
from .._configs.firefox_options import FirefoxOptions
from .._bidi import browsing_context as bidi_context
from ..errors import BrowserConnectError

logger = logging.getLogger("ruyipage")

_INITIAL_CONTEXT_WAIT_TIMEOUT = 3.0
_INITIAL_CONTEXT_POLL_INTERVAL = 0.1


def _is_valid_context_id(context_id):
    return isinstance(context_id, str) and bool(context_id)


class FirefoxPage(FirefoxBase):
    """Firefox 页面控制器（顶层入口）

    用法::

        # 默认连接 127.0.0.1:9222
        page = FirefoxPage()

        # 自定义配置
        opts = FirefoxOptions()
        opts.set_port(9333).headless()
        page = FirefoxPage(opts)

        # 连接已有浏览器
        page = FirefoxPage('127.0.0.1:9222')
    """

    _type = "FirefoxPage"
    _PAGES = {}  # 单例缓存

    @classmethod
    def _cache_key_for(cls, addr_or_opts=None):
        """仅对显式 attach 场景启用地址级单例缓存。"""
        if isinstance(addr_or_opts, FirefoxOptions):
            if not addr_or_opts.is_existing_only:
                return None
            return addr_or_opts.address

        if isinstance(addr_or_opts, str):
            return addr_or_opts

        return None

    def __new__(cls, addr_or_opts=None):
        cache_key = cls._cache_key_for(addr_or_opts)

        if cache_key is not None:
            if cache_key in cls._PAGES:
                return cls._PAGES[cache_key]

        instance = super(FirefoxPage, cls).__new__(cls)
        if cache_key is not None:
            cls._PAGES[cache_key] = instance
        return instance

    def __init__(self, addr_or_opts=None):
        if hasattr(self, "_page_initialized") and self._page_initialized:
            return
        self._page_initialized = True

        super(FirefoxPage, self).__init__()

        # 创建/连接浏览器
        self._firefox = Firefox(addr_or_opts)

        # 获取第一个标签页的 context
        ctx_id = self._get_initial_context_id()

        self._init_context(self._firefox, ctx_id)
        self._apply_startup_window_size()

    def _get_initial_context_id(self):
        deadline = time.time() + _INITIAL_CONTEXT_WAIT_TIMEOUT
        while time.time() < deadline:
            tab_ids = [
                ctx_id for ctx_id in self._firefox.tab_ids if _is_valid_context_id(ctx_id)
            ]
            if tab_ids:
                return tab_ids[0]
            time.sleep(_INITIAL_CONTEXT_POLL_INTERVAL)

        for _ in range(3):
            result = bidi_context.create(self._firefox.driver, "tab")
            ctx_id = result.get("context", "")
            if _is_valid_context_id(ctx_id):
                return ctx_id
            time.sleep(_INITIAL_CONTEXT_POLL_INTERVAL)

        raise BrowserConnectError("无法获取可用的 Firefox browsingContext")

    def _apply_startup_window_size(self):
        options = getattr(self._firefox, "options", None)
        size = getattr(options, "startup_window_size", None)
        if not size or getattr(options, "is_existing_only", False):
            return

        try:
            width, height = int(size[0]), int(size[1])
        except (TypeError, ValueError, IndexError):
            return

        try:
            self.window.normal()
        except Exception as e:
            logger.debug("Startup window normal failed: %s", e)

        try:
            set_size_only = getattr(self.window, "_set_size_only", None)
            if callable(set_size_only):
                set_size_only(width, height)
            else:
                self.window.set_size(width, height)
        except Exception as e:
            logger.debug("Startup window size apply failed: %s", e)

    @property
    def browser(self) -> "Firefox":
        """Firefox 浏览器实例"""
        return self._firefox

    @property
    def tabs_count(self) -> int:
        """标签页数量"""
        return self._firefox.tabs_count

    @property
    def tab_ids(self) -> list[str]:
        """所有标签页 ID"""
        return self._firefox.tab_ids

    @property
    def latest_tab(self) -> "FirefoxTab":
        """最新的标签页"""
        return self._firefox.latest_tab

    def new_tab(self, url=None, background=False, user_context=None) -> "FirefoxTab":
        """新建标签页

        Args:
            url: 初始 URL
            background: 后台创建
            user_context: 可选的 Firefox user context ID

        Returns:
            FirefoxTab
        """
        return self._firefox.new_tab(url, background, user_context=user_context)

    def new_container_tab(self, url=None, background=False) -> "FirefoxTab":
        """新建一个 Firefox container tab。"""
        return self._firefox.new_container_tab(url=url, background=background)

    def new_container_tabs(self, count, url=None, background=False) -> "list[FirefoxTab]":
        """新建多个 Firefox container tabs。"""
        return self._firefox.new_container_tabs(count=count, url=url, background=background)

    def get_tab(self, id_or_num=None, title=None, url=None) -> "FirefoxTab":
        """获取标签页

        Args:
            id_or_num: context ID 或序号
            title: 按标题匹配
            url: 按 URL 匹配

        Returns:
            FirefoxTab
        """
        return self._firefox.get_tab(id_or_num, title, url)

    def get_tabs(self, title=None, url=None) -> "list[FirefoxTab]":
        """获取匹配的标签页列表"""
        return self._firefox.get_tabs(title, url)

    def close(self) -> None:
        """关闭当前标签页"""
        try:
            bidi_context.close(self._driver._browser_driver, self._context_id)
        except Exception:
            pass

        # 切换到其他标签页
        tab_ids = self._firefox.tab_ids
        if tab_ids:
            self._context_id = tab_ids[-1]
            self._driver = type(self._driver)(self._firefox.driver, self._context_id)

    def close_other_tabs(self, tab_or_ids=None) -> None:
        """关闭其他标签页

        Args:
            tab_or_ids: 要保留的标签页（默认保留当前标签页）
        """
        if tab_or_ids is None:
            tab_or_ids = self._context_id
        self._firefox.close_tabs(tab_or_ids, others=True)

    def quit(self, timeout=5, force=False) -> None:
        """关闭浏览器

        Args:
            timeout: 等待超时
            force: 强制关闭
        """
        address = self._firefox.address
        self._firefox.quit(timeout, force)
        self._PAGES.pop(address, None)

    def save(self, path=None, name=None, as_pdf=False) -> str:
        """保存页面

        Args:
            path: 保存目录
            name: 文件名（不含后缀）
            as_pdf: True 保存为 PDF，False 保存为 HTML

        Returns:
            保存的文件路径
        """
        import os

        if path is None:
            path = "."
        if name is None:
            title = self.title or "page"
            # 清理文件名中的非法字符
            name = "".join(c for c in title if c not in r'\/:*?"<>|')[:50]

        if as_pdf:
            file_path = os.path.join(path, name + ".pdf")
            self.pdf(file_path)
        else:
            file_path = os.path.join(path, name + ".html")
            html = self.html
            os.makedirs(path, exist_ok=True)
            with open(file_path, "w", encoding="utf-8") as f:
                f.write(html)

        return file_path
