# -*- coding: utf-8 -*-
"""Cookie 结果对象。"""


class CookieInfo(object):
    """单个 Cookie 信息对象。

    Args:
        data: 原始 Cookie 字典。

    Returns:
        CookieInfo: 支持属性访问的 Cookie 对象。

    适用场景：
        - 避免示例里继续写 ``cookie.get('name')``
        - 提升编辑器对 Cookie 字段的跳转和补全体验
    """

    def __init__(self, data):
        self.raw = dict(data or {})
        self.name = self.raw.get("name", "")
        raw_value = self.raw.get("value", "")
        if isinstance(raw_value, dict):
            self.value = raw_value.get("value", "")
        else:
            self.value = str(raw_value)
        self.domain = self.raw.get("domain")
        self.path = self.raw.get("path")
        self.http_only = self.raw.get("httpOnly")
        self.secure = self.raw.get("secure")
        self.same_site = self.raw.get("sameSite")
        self.expiry = self.raw.get("expiry")
