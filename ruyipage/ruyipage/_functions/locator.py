# -*- coding: utf-8 -*-
"""定位器解析 - 将定位器字符串转换为 BiDi locator 字典"""

import re

from ..errors import LocatorError


def parse_locator(locator):
    """解析定位器字符串为 BiDi locator 字典

    支持的格式:
        '#id'                   -> CSS: #id
        '.class'                -> CSS: .class
        '@attr=val'             -> CSS: [attr='val']
        '@attr'                 -> CSS: [attr]
        '@@attr1=v1@@attr2=v2'  -> CSS: [attr1='v1'][attr2='v2']
        'tag:div'               -> CSS: div
        'tag:div@class=foo'     -> CSS: div[class='foo']
        'tag:div@@a=1@@b=2'     -> CSS: div[a='1'][b='2']
        'text:hello'            -> innerText 包含匹配
        'text=exact'            -> innerText 精确匹配
        'xpath://...'           -> XPath
        'x://...'               -> XPath (简写)
        'css:selector'          -> CSS
        'c:selector'            -> CSS (简写)
        ('css', 'selector')     -> CSS (元组形式)
        ('xpath', '//...')      -> XPath (元组形式)
        纯文字 'hello world'    -> innerText 包含匹配

    Args:
        locator: 定位器字符串或元组

    Returns:
        BiDi locator 字典, 如 {'type': 'css', 'value': '#myid'}
    """
    # 元组形式 (type, value)
    if isinstance(locator, tuple):
        if len(locator) != 2:
            raise LocatorError('元组定位器必须是 (type, value) 格式: {}'.format(locator))
        loc_type, loc_value = locator
        type_map = {
            'css': 'css', 'css selector': 'css',
            'xpath': 'xpath',
            'text': 'innerText', 'inner_text': 'innerText', 'innerText': 'innerText',
            'accessibility': 'accessibility',
        }
        bidi_type = type_map.get(loc_type.lower().replace(' ', ''))
        if bidi_type is None:
            raise LocatorError('不支持的定位器类型: {}'.format(loc_type))
        if bidi_type == 'accessibility':
            if isinstance(loc_value, dict):
                return {'type': 'accessibility', 'value': loc_value}
            return {'type': 'accessibility', 'value': {'name': loc_value}}
        return {'type': bidi_type, 'value': loc_value}

    if not isinstance(locator, str):
        raise LocatorError('定位器必须是字符串或元组: {}'.format(type(locator)))

    locator = locator.strip()
    if not locator:
        raise LocatorError('定位器不能为空')

    # CSS 前缀
    if locator.startswith('css:') or locator.startswith('c:'):
        prefix = 'css:' if locator.startswith('css:') else 'c:'
        return {'type': 'css', 'value': locator[len(prefix):].strip()}

    # XPath 前缀
    if locator.startswith('xpath:') or locator.startswith('x:'):
        prefix = 'xpath:' if locator.startswith('xpath:') else 'x:'
        return {'type': 'xpath', 'value': locator[len(prefix):].strip()}

    # XPath 自动检测: 以 / 或 ./ 或 ( 开头
    if locator.startswith('/') or locator.startswith('./') or locator.startswith('('):
        return {'type': 'xpath', 'value': locator}

    # text 精确匹配
    if locator.startswith('text='):
        text = locator[5:]
        return {'type': 'innerText', 'value': text, 'matchType': 'full'}

    # text 包含匹配
    if locator.startswith('text:'):
        text = locator[5:]
        return {'type': 'innerText', 'value': text}

    # CSS ID
    if locator.startswith('#'):
        return {'type': 'css', 'value': locator}

    # CSS class
    if locator.startswith('.') and not locator.startswith('./'):
        return {'type': 'css', 'value': locator}

    # tag 标签名
    if locator.startswith('tag:'):
        tag_and_rest = locator[4:]
        return _parse_tag_locator(tag_and_rest)

    # 多属性 @@attr1=v1@@attr2=v2
    if locator.startswith('@@'):
        return _parse_multi_attr(locator, '')

    # 单属性 @attr=val 或 @attr
    if locator.startswith('@'):
        return _parse_single_attr(locator[1:], '')

    # 已经是 CSS 选择器的复杂形式
    if _looks_like_css_selector(locator):
        return {'type': 'css', 'value': locator}

    # 默认：当作文本包含匹配
    return {'type': 'innerText', 'value': locator}


def _parse_tag_locator(tag_and_rest):
    """解析 tag:tagname@attr=val 或 tag:tagname@@a=1@@b=2 格式"""
    # 查找第一个 @@ 或 @ 的位置
    double_at = tag_and_rest.find('@@')
    single_at = tag_and_rest.find('@')

    if double_at >= 0 and (single_at < 0 or double_at <= single_at):
        # tag:div@@attr1=v1@@attr2=v2
        tag = tag_and_rest[:double_at].strip()
        return _parse_multi_attr(tag_and_rest[double_at:], tag)
    elif single_at >= 0:
        # tag:div@attr=val
        tag = tag_and_rest[:single_at].strip()
        return _parse_single_attr(tag_and_rest[single_at + 1:], tag)
    else:
        # tag:div (仅标签名)
        tag = tag_and_rest.strip()
        return {'type': 'css', 'value': tag}


def _parse_single_attr(attr_str, tag=''):
    """解析 attr=val 或 attr 格式"""
    if '=' in attr_str:
        attr, val = attr_str.split('=', 1)
        attr = attr.strip()
        val = val.strip()

        # text() 特殊处理
        if attr == 'text()':
            if tag:
                # 需要 XPath 来同时匹配 tag 和 text
                return {'type': 'xpath',
                        'value': '//{}[contains(text(), "{}")]'.format(tag or '*', val)}
            return {'type': 'innerText', 'value': val}

        css = '{tag}[{attr}=\'{val}\']'.format(
            tag=tag, attr=attr, val=_css_escape_value(val))
        return {'type': 'css', 'value': css}
    else:
        attr = attr_str.strip()
        css = '{}[{}]'.format(tag, attr)
        return {'type': 'css', 'value': css}


def _parse_multi_attr(locator_str, tag=''):
    """解析 @@attr1=v1@@attr2=v2 格式"""
    # 分割多个属性
    parts = re.split(r'@@', locator_str)
    parts = [p for p in parts if p]  # 过滤空串

    css_attrs = []
    has_text = False
    text_val = ''

    for part in parts:
        if '=' in part:
            attr, val = part.split('=', 1)
            attr = attr.strip()
            val = val.strip()

            if attr == 'text()':
                has_text = True
                text_val = val
            else:
                css_attrs.append("[{attr}='{val}']".format(
                    attr=attr, val=_css_escape_value(val)))
        else:
            css_attrs.append('[{}]'.format(part.strip()))

    if has_text:
        # 有 text() 需要用 XPath
        xpath_parts = []
        for part in parts:
            if '=' in part:
                attr, val = part.split('=', 1)
                attr = attr.strip()
                val = val.strip()
                if attr == 'text()':
                    xpath_parts.append('contains(text(), "{}")'.format(val))
                else:
                    xpath_parts.append('@{}="{}"'.format(attr, val))
            else:
                xpath_parts.append('@{}'.format(part.strip()))

        xpath = '//{tag}[{conditions}]'.format(
            tag=tag or '*',
            conditions=' and '.join(xpath_parts)
        )
        return {'type': 'xpath', 'value': xpath}

    css = '{}{}'.format(tag, ''.join(css_attrs))
    return {'type': 'css', 'value': css}


def _css_escape_value(val):
    """转义 CSS 属性值中的特殊字符"""
    return val.replace("'", "\\'").replace('"', '\\"')


def _looks_like_css_selector(s):
    """判断字符串是否看起来像 CSS 选择器"""
    css_patterns = [
        r'^\[',          # [attr] 或 [attr="value"]
        r'^\w+\[',       # tag[attr]
        r'^\w+\s*>',     # tag >
        r'^\w+\s*\+',    # tag +
        r'^\w+\s*~',     # tag ~
        r'^\*',          # *
        r'^\w+:',        # tag:pseudo (但不是我们的 tag: 前缀)
    ]
    for pattern in css_patterns:
        if re.match(pattern, s):
            return True
    return False
