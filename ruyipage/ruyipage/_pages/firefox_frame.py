# -*- coding: utf-8 -*-
"""FirefoxFrame - iframe/frame 控制器"""

import logging
from typing import TYPE_CHECKING

from .firefox_base import FirefoxBase
from .._base.driver import ContextDriver

logger = logging.getLogger('ruyipage')


class FirefoxFrame(FirefoxBase):
    """Firefox iframe/frame

    BiDi 的优势：每个 iframe 有自己的 browsingContext ID，
    即使跨域也可以直接操作，不需要 CDP 那样的复杂切换。
    """

    _type = 'FirefoxFrame'

    def __init__(self, browser, context_id, parent_page):
        """
        Args:
            browser: Firefox 实例
            context_id: iframe 的 browsingContext ID
            parent_page: 父页面/Tab
        """
        super(FirefoxFrame, self).__init__()
        self._init_context(browser, context_id)
        self._parent = parent_page

    @property
    def parent(self) -> "FirefoxBase":
        """父页面"""
        return self._parent

    @property
    def is_cross_origin(self) -> bool:
        """是否跨域

        Returns:
            bool
        """
        try:
            parent_origin = self._parent.run_js('location.origin')
            my_origin = self.run_js('location.origin')
            return parent_origin != my_origin
        except Exception:
            return True

    def __repr__(self):
        try:
            url = self.url
        except Exception:
            url = '?'
        return '<FirefoxFrame {}>'.format(url[:60])
