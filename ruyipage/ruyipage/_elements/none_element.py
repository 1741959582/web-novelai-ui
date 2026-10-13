# -*- coding: utf-8 -*-
"""NoneElement - 元素未找到时返回的空对象

实现空对象模式，允许安全地链式调用而不抛异常。
"""


class NoneElement(object):
    """元素未找到时返回的空对象

    所有属性访问返回 None 或空值，
    所有方法调用返回 self（允许链式调用不报错）。
    """

    _type = 'NoneElement'

    def __init__(self, page=None, method=None, args=None):
        """
        Args:
            page: 所属页面
            method: 查找方法名
            args: 查找参数
        """
        self._page = page
        self._method = method
        self._args = args or {}

    def __repr__(self):
        return '<NoneElement>'

    def __str__(self):
        return 'NoneElement'

    def __bool__(self):
        """NoneElement 在布尔上下文中为 False"""
        return False

    def __eq__(self, other):
        return other is None or isinstance(other, NoneElement)

    def __hash__(self):
        return hash(None)

    # 属性访问返回空值
    @property
    def tag(self) -> str:
        return ''

    @property
    def text(self) -> str:
        return ''

    @property
    def raw_text(self) -> str:
        return ''

    @property
    def html(self) -> str:
        return ''

    @property
    def inner_html(self) -> str:
        return ''

    @property
    def outer_html(self) -> str:
        return ''

    @property
    def value(self) -> str:
        return ''

    @property
    def attrs(self) -> dict:
        return {}

    @property
    def link(self) -> str:
        return ''

    @property
    def src(self) -> str:
        return ''

    @property
    def is_displayed(self) -> bool:
        return False

    @property
    def is_enabled(self) -> bool:
        return False

    @property
    def is_checked(self) -> bool:
        return False

    @property
    def size(self) -> dict:
        return {'width': 0, 'height': 0}

    @property
    def location(self) -> dict:
        return {'x': 0, 'y': 0}

    @property
    def shadow_root(self) -> None:
        return None

    def attr(self, name) -> None:
        return None

    def property(self, name) -> None:
        return None

    def style(self, name, pseudo='') -> str:
        return ''

    # 交互方法返回 self
    def click_self(self, *args, **kwargs) -> "NoneElement":
        return self

    def input(self, *args, **kwargs) -> "NoneElement":
        return self

    def clear(self, *args, **kwargs) -> "NoneElement":
        return self

    def hover(self, *args, **kwargs) -> "NoneElement":
        return self

    def drag_to(self, *args, **kwargs) -> "NoneElement":
        return self

    def focus(self, *args, **kwargs) -> "NoneElement":
        return self

    def screenshot(self, *args, **kwargs) -> None:
        return None

    # DOM 导航返回 NoneElement
    def parent(self, *args, **kwargs) -> "NoneElement":
        return NoneElement()

    def child(self, *args, **kwargs) -> "NoneElement":
        return NoneElement()

    def children(self, *args, **kwargs) -> list:
        return []

    def next(self, *args, **kwargs) -> "NoneElement":
        return NoneElement()

    def prev(self, *args, **kwargs) -> "NoneElement":
        return NoneElement()

    def ele(self, *args, **kwargs) -> "NoneElement":
        return NoneElement()

    def eles(self, *args, **kwargs) -> list:
        return []

    def s_ele(self, *args, **kwargs) -> "NoneElement":
        return NoneElement()

    def s_eles(self, *args, **kwargs) -> list:
        return []

    def run_js(self, *args, **kwargs) -> None:
        return None
