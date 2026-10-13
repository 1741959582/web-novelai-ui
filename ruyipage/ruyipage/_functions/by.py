# -*- coding: utf-8 -*-
"""By 定位器类型枚举"""


class By(object):
    """定位器类型常量

    用法::

        page.ele((By.CSS, '#myid'))
        page.ele((By.XPATH, '//div'))
    """

    CSS = 'css'
    XPATH = 'xpath'
    TEXT = 'text'
    INNER_TEXT = 'innerText'
    ACCESSIBILITY = 'accessibility'
    ID = 'id'
    CLASS_NAME = 'class name'
    TAG_NAME = 'tag name'
    NAME = 'name'
    LINK_TEXT = 'link text'
