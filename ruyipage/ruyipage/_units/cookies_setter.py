# -*- coding: utf-8 -*-
"""CookiesSetter - Cookie 操作"""


class CookiesSetter(object):
    """Cookie 操作管理器

    用法::

        page.cookies_setter.set({'name': 'token', 'value': 'abc'})
        page.cookies_setter.clear()
    """

    def __init__(self, owner):
        self._owner = owner

    def set(self, cookies):
        """设置 Cookie"""
        self._owner.set_cookies(cookies)
        return self._owner

    def remove(self, name, domain=None):
        """删除指定 Cookie"""
        self._owner.delete_cookies(name=name, domain=domain)
        return self._owner

    def clear(self):
        """清除所有 Cookie"""
        self._owner.delete_cookies()
        return self._owner
