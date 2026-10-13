# -*- coding: utf-8 -*-
"""Rect - 位置/尺寸信息"""


class TabRect(object):
    """页面视口位置/尺寸"""

    def __init__(self, owner):
        self._owner = owner

    @property
    def window_size(self):
        """窗口大小 (width, height)"""
        result = self._owner.run_js("({w: window.outerWidth, h: window.outerHeight})")
        if result:
            return result.get("w", 0), result.get("h", 0)
        return 0, 0

    @property
    def viewport_size(self):
        """视口大小 (width, height)"""
        result = self._owner.run_js("({w: window.innerWidth, h: window.innerHeight})")
        if result:
            return result.get("w", 0), result.get("h", 0)
        return 0, 0

    @property
    def page_size(self):
        """页面完整大小 (width, height)"""
        result = self._owner.run_js("""({
            w: Math.max(document.documentElement.scrollWidth, document.body ? document.body.scrollWidth : 0),
            h: Math.max(document.documentElement.scrollHeight, document.body ? document.body.scrollHeight : 0)
        })""")
        if result:
            return result.get("w", 0), result.get("h", 0)
        return 0, 0

    @property
    def scroll_position(self):
        """当前滚动位置 (x, y)"""
        result = self._owner.run_js("({x: window.scrollX, y: window.scrollY})")
        if result:
            return result.get("x", 0), result.get("y", 0)
        return 0, 0

    @property
    def window_location(self):
        """窗口位置 (x, y)"""
        result = self._owner.run_js("({x: window.screenX, y: window.screenY})")
        if result:
            return result.get("x", 0), result.get("y", 0)
        return 0, 0

    @property
    def viewport_midpoint(self):
        """视口中心点 (x, y)"""
        width, height = self.viewport_size
        return int(width / 2), int(height / 2)


class ElementRect(object):
    """元素位置/尺寸"""

    def __init__(self, element):
        self._ele = element

    @property
    def size(self):
        """元素尺寸 (width, height)"""
        s = self._ele.size
        return s.get("width", 0), s.get("height", 0)

    @property
    def location(self):
        """元素在页面中的位置 (x, y)"""
        loc = self._ele.location
        return loc.get("x", 0), loc.get("y", 0)

    @property
    def midpoint(self):
        """元素中心点 (x, y)"""
        pos = self._ele._get_center()
        if pos:
            return pos.get("x", 0), pos.get("y", 0)
        return 0, 0

    @property
    def click_point(self):
        """可点击位置（同 midpoint）"""
        return self.midpoint

    @property
    def viewport_location(self):
        """元素相对于视口的位置 (x, y)"""
        result = self._ele._read_geometry(
            """(el) => {
            const r = el.getBoundingClientRect();
            return {x: Math.round(r.x), y: Math.round(r.y)};
        }""",
            ("x", "y"),
            "视口坐标",
        )
        return result["x"], result["y"]

    @property
    def viewport_midpoint(self):
        """元素相对于视口的中心点"""
        result = self._ele._read_geometry(
            """(el) => {
            const r = el.getBoundingClientRect();
            return {x: Math.round(r.x + r.width/2), y: Math.round(r.y + r.height/2)};
        }""",
            ("x", "y"),
            "视口中心坐标",
        )
        return result["x"], result["y"]

    @property
    def corners(self):
        """元素四个角的坐标 [(x1,y1), (x2,y2), (x3,y3), (x4,y4)]"""
        result = self._ele._call_js_on_self("""(el) => {
            const r = el.getBoundingClientRect();
            return [
                {x: Math.round(r.left), y: Math.round(r.top)},
                {x: Math.round(r.right), y: Math.round(r.top)},
                {x: Math.round(r.right), y: Math.round(r.bottom)},
                {x: Math.round(r.left), y: Math.round(r.bottom)}
            ];
        }""")
        if result and isinstance(result, list):
            return [(p.get("x", 0), p.get("y", 0)) for p in result]
        return [(0, 0)] * 4
