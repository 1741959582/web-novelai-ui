# -*- coding: utf-8 -*-
"""BasePage / BaseElement 抽象基类"""


class BasePage(object):
    """页面基类，定义通用接口"""

    _type = 'BasePage'

    def __repr__(self):
        return '<{} {}>'.format(self._type, getattr(self, 'url', ''))

    def __str__(self):
        return self.__repr__()


class BaseElement(object):
    """元素基类，定义通用接口"""

    _type = 'BaseElement'

    def __repr__(self):
        tag = getattr(self, 'tag', '?')
        text = getattr(self, 'text', '')
        if text and len(text) > 30:
            text = text[:30] + '...'
        return '<{} {} "{}">'.format(self._type, tag, text)

    def __str__(self):
        return self.__repr__()

    def __bool__(self):
        return True
