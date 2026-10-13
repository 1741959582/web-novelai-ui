# -*- coding: utf-8 -*-
"""StaticElement —— 从 HTML 字符串解析出来的静态元素。

和 FirefoxElement 的区别：
    FirefoxElement 每取一个值都要走一次 BiDi 问浏览器；StaticElement 是把
    HTML 一次性抓下来在本地解析，之后所有查找都是纯内存操作，没有网络往返。
    适合「列表页批量抽字段」这种读多、不需要交互的场景。

代价：
    静态快照在抓取的那一刻就和页面脱钩了。页面后续的 DOM 变化不会反映到
    StaticElement 上，它也没有 click / input 这类交互能力。需要交互就用
    FirefoxElement。

本模块提供与 FirefoxElement 同名、同语义的一整套查找与导航方法
（ele / eles / s_ele / s_eles / child / children / parent / next / prev），
因此 ``page.ele(...)`` 写惯的链式代码可以原样搬到 ``page.s_ele(...)`` 上。

底层依赖 lxml + cssselect（两者都在 ruyiPage 的必装依赖里），
所以 CSS、XPath、文本三种定位器在任何环境下行为一致。
"""

import re
from functools import lru_cache

from ..errors import LocatorError
from .._functions.locator import parse_locator

# 判断一段 HTML 是不是「整篇文档」而不是「元素片段」。
# page.html 是整篇文档，element.html 是片段，两者的根节点处理方式不同。
_DOCUMENT_RE = re.compile(r'<\s*(?:!doctype\b|html[\s>])', re.I)

# lxml 解析片段时会自动补出来的包装层，取片段根节点时要剥掉
_WRAPPER_TAGS = ('html', 'body')


# ──────────────────────────── 节点层工具 ────────────────────────────
# 这一层直接操作 lxml 元素，StaticElement 只是它的面向对象封装。

def _is_element(node):
    """是否为真正的元素节点。

    lxml 会把注释、处理指令也挂在树上，它们的 ``tag`` 是一个函数而不是
    字符串。不过滤掉的话，children() / next() 会莫名其妙返回注释。
    """
    return hasattr(node, 'tag') and isinstance(node.tag, str)


def _own_text(node):
    """元素自身直接文本节点拼起来的文本（不含后代元素里的文字）。

    ``<a>he<b>l</b>llo</a>`` 的 own_text 是 ``'hello'``，而 ``<b>`` 的是 ``'l'``。
    文本定位器用它来匹配，这样 ``text:登录`` 命中的是最里层那个标签，
    而不是连 ``<html>`` 带 ``<body>`` 一路全中。
    """
    parts = [node.text or '']
    parts.extend((child.tail or '') for child in node)
    return ''.join(parts)


def _parse_document(html):
    """把 HTML 解析成 lxml 树，解析不出来返回 None。"""
    if not html or not html.strip():
        return None
    from lxml import etree

    try:
        return etree.HTML(html)
    except Exception:
        return None


def _fragment_root(doc, html):
    """取片段 HTML 真正的根节点。

    ``etree.HTML('<div>x</div>')`` 会补出 ``<html><body><div>``，直接把 doc
    当根的话，``element.s_ele()`` 返回的就不是那个元素本身，而是凭空多出来的
    ``<html>``。这里把解析器补出来的包装层剥掉。

    整篇文档（含 ``<!doctype>`` 或 ``<html>``）不剥，根就是 ``<html>``。
    """
    if _DOCUMENT_RE.search(html or ''):
        return doc

    node = doc
    while _is_element(node) and node.tag in _WRAPPER_TAGS:
        # <head> 是解析器为片段补的，不算片段内容
        kids = [k for k in node if _is_element(k) and k.tag != 'head']
        if len(kids) != 1:
            # 片段里有多个顶层元素（比如两个并列的 <li>），保留容器当根，
            # 这样 eles() 仍然能在容器里把它们全找出来
            break
        node = kids[0]
    return node


@lru_cache(maxsize=256)
def _compile_css(selector):
    """编译并缓存 CSS 选择器。

    同一个选择器在循环里会被反复使用（比如逐条解析搜索结果），
    缓存掉编译开销。语法错误直接抛 LocatorError，不静默吞掉。
    """
    from lxml.cssselect import CSSSelector

    try:
        return CSSSelector(selector)
    except Exception as e:
        raise LocatorError('CSS 选择器语法错误：{}（{}）'.format(selector, e))


def _css_select(node, selector):
    """CSS 选择器查找。"""
    return [e for e in _compile_css(selector)(node) if _is_element(e)]


def _xpath_select(node, expr, relative):
    """XPath 查找。

    XPath 里 ``//xxx`` 永远从文档根开始算，哪怕是在某个元素上调用的。
    在元素内部查找时把开头的 ``/`` 补成 ``.``，否则
    ``card.ele('x://a')`` 会把整页的 ``<a>`` 都捞回来——
    这跟 ``FirefoxElement.ele()`` 的直觉完全相反。
    """
    if relative and expr.startswith('/'):
        expr = '.' + expr

    try:
        result = node.xpath(expr)
    except Exception as e:
        raise LocatorError('XPath 语法错误：{}（{}）'.format(expr, e))

    if not isinstance(result, list):
        # count() / string() 这类返回标量的表达式，静态元素查找里没有意义
        return []
    return [e for e in result if _is_element(e)]


def _text_select(node, text, full_match):
    """按文本查找，返回文档序结果。

    Args:
        full_match: True 为精确相等（``text=xxx``），False 为包含（``text:xxx``）。
    """
    result = []
    for e in node.iter():
        if not _is_element(e):
            continue
        own = _own_text(e)
        if (own.strip() == text) if full_match else (text in own):
            result.append(e)
    return result


def _find_nodes(node, locator, relative):
    """按定位器在 node 子树里查找，返回 lxml 元素列表（文档序）。

    Args:
        node: 起始节点。
        locator: ruyiPage 定位器，写法与 ``page.ele()`` 完全一致。
        relative: True 表示「在元素内部找」，此时会排除 node 自身，
            并把 XPath 改写成相对路径；False 表示在整棵文档里找。

    Raises:
        LocatorError: 定位器语法错误或类型不支持。
    """
    bidi = parse_locator(locator)
    loc_type = bidi.get('type', '')
    value = bidi.get('value', '')

    if loc_type == 'css':
        nodes = _css_select(node, value)
    elif loc_type == 'xpath':
        nodes = _xpath_select(node, value, relative)
    elif loc_type == 'innerText':
        nodes = _text_select(node, value, bidi.get('matchType') == 'full')
    else:
        # accessibility 定位器要靠浏览器的无障碍树，静态 HTML 里没有这些信息
        raise LocatorError(
            '静态元素不支持的定位器类型：{}（请改用 CSS / XPath / 文本定位）'.format(loc_type))

    if relative:
        # cssselect 生成的是 descendant-or-self::，自身也会被匹配上；
        # 而 element.ele() 的语义是「找后代」，和 querySelectorAll 一致，
        # 所以统一把自身剔除。
        nodes = [e for e in nodes if e is not node]
    return nodes


def _pick(nodes, index):
    """按 index 取一个节点，取不到返回 None。

    index 语义与 ``page.ele()`` 一致：1 开始计数，负数从后往前，0 当 1 用。
    """
    if not nodes:
        return None
    if index > 0:
        i = index - 1
    elif index < 0:
        i = index
    else:
        i = 0
    try:
        return nodes[i]
    except IndexError:
        return None


def _make_matcher(locator):
    """把定位器编译成「单个节点是否匹配」的判断函数。

    parent() / next() / prev() 的过滤参数用它：这些场景是沿着一条线逐个
    节点试，而不是在子树里搜索。
    """
    bidi = parse_locator(locator)
    loc_type = bidi.get('type', '')
    value = bidi.get('value', '')

    if loc_type == 'css':
        selector = _compile_css(value)

        def match(node):
            # cssselect 生成的是 descendant-or-self::，拿父节点当作用域跑一遍，
            # 结果里有自己就说明自己匹配；没有父节点（根）时就在自己身上跑。
            parent = node.getparent()
            scope = parent if parent is not None else node
            return any(e is node for e in selector(scope))

        return match

    if loc_type == 'innerText':
        full = bidi.get('matchType') == 'full'

        def match(node):
            own = _own_text(node)
            return own.strip() == value if full else value in own

        return match

    raise LocatorError(
        'parent() / next() / prev() 的过滤条件只支持 CSS 和文本定位器，'
        '不支持 {}（XPath 请改用 ele()）'.format(loc_type))


def _none():
    """返回 NoneElement（元素未找到时的空对象）。"""
    from .none_element import NoneElement
    return NoneElement()


# ──────────────────────────── StaticElement ────────────────────────────

class StaticElement(object):
    """静态 HTML 元素（不连浏览器，纯本地解析）。

    一般不用自己构造，由 ``page.s_ele()`` / ``element.s_ele()`` /
    ``make_static_ele()`` 返回。
    """

    _type = 'StaticElement'

    def __init__(self, tag=None, attrs=None, text=None, inner_html=None,
                 outer_html=None, node=None):
        """
        Args:
            node: 对应的 lxml 元素。有它才能继续 ele() / parent() 等导航，
                各属性也改为按需从树上取，不必在构造时全算一遍。
            tag / attrs / text / inner_html / outer_html:
                不带 node 构造时的静态取值，保留给老调用方。
        """
        self._node = node
        self._tag = tag
        self._attrs = attrs
        self._text = text
        self._inner_html = inner_html
        self._outer_html = outer_html

    # ===== 基本属性 =====
    # 带 node 时一律惰性求值：eles() 可能一次返回上千个元素，
    # 在构造时就把每个元素的 outerHTML 序列化出来纯属浪费。

    @property
    def tag(self) -> str:
        """标签名，如 ``'div'``。"""
        if self._tag is None:
            self._tag = self._node.tag if self._node is not None else ''
        return self._tag

    @property
    def text(self) -> str:
        """元素及其所有后代的文本。"""
        if self._text is None:
            self._text = ''.join(self._node.itertext()) if self._node is not None else ''
        return self._text

    @property
    def raw_text(self) -> str:
        """仅元素自身直接文本节点的内容，不含后代元素里的文字。"""
        if self._node is not None:
            return _own_text(self._node)
        return self._text or ''

    @property
    def html(self) -> str:
        """外部 HTML（outerHTML），含元素自身的标签。"""
        return self.outer_html

    @property
    def outer_html(self) -> str:
        """外部 HTML（outerHTML）。"""
        if self._outer_html is None:
            self._outer_html = self._serialize_outer()
        return self._outer_html

    @property
    def inner_html(self) -> str:
        """内部 HTML（innerHTML），不含元素自身的标签。"""
        if self._inner_html is None:
            self._inner_html = self._serialize_inner()
        return self._inner_html

    @property
    def attrs(self) -> dict:
        """所有属性组成的字典（副本，改它不会影响元素）。"""
        if self._attrs is None:
            self._attrs = dict(self._node.attrib) if self._node is not None else {}
        return dict(self._attrs)

    @property
    def link(self) -> str:
        """``href`` 属性。"""
        return self.attr('href') or ''

    @property
    def src(self) -> str:
        """``src`` 属性。"""
        return self.attr('src') or ''

    @property
    def value(self) -> str:
        """``value`` 属性。"""
        return self.attr('value') or ''

    def attr(self, name):
        """取单个属性值，没有该属性返回 None。"""
        if self._attrs is not None:
            return self._attrs.get(name)
        if self._node is None:
            return None
        return self._node.get(name)

    def _serialize_outer(self):
        """序列化 outerHTML。"""
        if self._node is None:
            return ''
        from lxml import etree

        html = etree.tostring(self._node, encoding='unicode', method='html')
        # lxml 会把 tail（元素结束标签之后的那段文本）一起序列化进来，
        # 但那段文本属于父元素，不属于本元素，必须去掉
        tail = self._node.tail
        if tail and html.endswith(tail):
            html = html[:-len(tail)]
        return html

    def _serialize_inner(self):
        """序列化 innerHTML。"""
        if self._node is None:
            return ''
        from lxml import etree

        parts = [self._node.text or '']
        parts.extend(
            etree.tostring(child, encoding='unicode', method='html')
            for child in self._node)
        return ''.join(parts)

    def _wrap(self, node):
        """把 lxml 节点包成 StaticElement，None 则返回 NoneElement。"""
        return StaticElement(node=node) if node is not None else _none()

    # ===== 相对查找 =====

    def ele(self, locator, index=1) -> "StaticElement | NoneElement":
        """在当前元素内部查找单个后代元素。

        Args:
            locator: 定位器，写法与 ``page.ele()`` 完全一致，例如
                ``card.ele('css:h2 a')``、``card.ele('#title')``、
                ``card.ele('text:下一页')``、``card.ele('x://a[@rel]')``。
                XPath 开头的 ``//`` 会被自动改写成 ``.//``，即限定在当前子树内。
            index: 第几个匹配结果，从 1 开始，负数从后往前数。

        Returns:
            StaticElement 或 NoneElement（未找到）。

        适用场景：
            - 先 ``s_ele()`` 抓下一张卡片，再在卡片内部取标题、链接、摘要
        """
        if self._node is None:
            return _none()
        return self._wrap(_pick(_find_nodes(self._node, locator, True), index))

    def eles(self, locator) -> "list[StaticElement]":
        """在当前元素内部查找所有匹配的后代元素。

        Args:
            locator: 定位器，写法同 ``ele()``。

        Returns:
            list[StaticElement]: 按文档序排列，没找到返回空列表。
        """
        if self._node is None:
            return []
        return [self._wrap(n) for n in _find_nodes(self._node, locator, True)]

    def s_ele(self, locator=None, index=1) -> "StaticElement | NoneElement":
        """等价于 ``ele()``，为了和 FirefoxElement 的方法名对齐而保留。

        StaticElement 本身已经是静态快照，继续 ``s_ele()`` 不会再抓一次页面。
        ``locator`` 为 None 时返回自身。
        """
        if locator is None:
            return self
        return self.ele(locator, index=index)

    def s_eles(self, locator) -> "list[StaticElement]":
        """等价于 ``eles()``，为了和 FirefoxElement 的方法名对齐而保留。"""
        return self.eles(locator)

    # ===== DOM 树导航 =====

    def child(self, locator=None, index=1) -> "StaticElement | NoneElement":
        """取子元素。

        Args:
            locator: 为 None 时取第 index 个**直接**子元素；
                传了定位器则等价于 ``ele(locator, index)``，会搜索整棵子树
                （与 ``FirefoxElement.child()`` 行为保持一致）。
            index: 第几个，从 1 开始，负数从后往前数。

        Returns:
            StaticElement 或 NoneElement。
        """
        if locator is not None:
            return self.ele(locator, index=index)
        if self._node is None:
            return _none()
        return self._wrap(_pick([c for c in self._node if _is_element(c)], index))

    def children(self, locator=None) -> "list[StaticElement]":
        """取所有子元素。

        Args:
            locator: 为 None 时取全部**直接**子元素；
                传了定位器则等价于 ``eles(locator)``，会搜索整棵子树
                （与 ``FirefoxElement.children()`` 行为保持一致）。
        """
        if locator is not None:
            return self.eles(locator)
        if self._node is None:
            return []
        return [self._wrap(c) for c in self._node if _is_element(c)]

    def parent(self, locator=None, index=1) -> "StaticElement | NoneElement":
        """取父元素 / 祖先元素。

        Args:
            locator: 为 None 时向上数 index 层；传了定位器则向上找第 index 个
                匹配该条件的祖先。只支持 CSS 和文本定位器。
            index: 第几层 / 第几个匹配的祖先，从 1 开始。

        Returns:
            StaticElement 或 NoneElement（已经到根了）。
        """
        if self._node is None:
            return _none()
        step = index if index > 0 else 1

        if locator is None:
            node = self._node
            for _ in range(step):
                node = node.getparent()
                if node is None or not _is_element(node):
                    return _none()
            return self._wrap(node)

        match = _make_matcher(locator)
        node = self._node.getparent()
        hit = 0
        while node is not None and _is_element(node):
            if match(node):
                hit += 1
                if hit >= step:
                    return self._wrap(node)
            node = node.getparent()
        return _none()

    def next(self, locator=None, index=1) -> "StaticElement | NoneElement":
        """取后面的兄弟元素。

        Args:
            locator: 为 None 时取后面第 index 个兄弟；传了定位器则取后面
                第 index 个匹配该条件的兄弟。只支持 CSS 和文本定位器。
            index: 第几个，从 1 开始。
        """
        return self._sibling(locator, index, forward=True)

    def prev(self, locator=None, index=1) -> "StaticElement | NoneElement":
        """取前面的兄弟元素。

        Args:
            locator: 为 None 时取前面第 index 个兄弟；传了定位器则取前面
                第 index 个匹配该条件的兄弟。只支持 CSS 和文本定位器。
            index: 第几个，从 1 开始。
        """
        return self._sibling(locator, index, forward=False)

    def _sibling(self, locator, index, forward):
        """next() / prev() 的公共实现，只有走向不同。"""
        if self._node is None:
            return _none()
        step = index if index > 0 else 1
        advance = (lambda n: n.getnext()) if forward else (lambda n: n.getprevious())
        match = _make_matcher(locator) if locator is not None else None

        node = advance(self._node)
        hit = 0
        while node is not None:
            # 跳过注释、处理指令这些非元素节点
            if _is_element(node) and (match is None or match(node)):
                hit += 1
                if hit >= step:
                    return self._wrap(node)
            node = advance(node)
        return _none()

    # ===== 杂项 =====

    def __repr__(self):
        text = self.text
        return '<StaticElement {} "{}">'.format(
            self.tag, text[:30] + '...' if len(text) > 30 else text)

    def __str__(self):
        return self.text

    def __bool__(self):
        return True


# ──────────────────────────── 工厂函数 ────────────────────────────

def make_static_ele(html, locator=None, index=1):
    """从 HTML 字符串解析出单个静态元素。

    Args:
        html: HTML 字符串，可以是整篇文档，也可以是元素片段。
        locator: 定位器。为 None 时返回这段 HTML 的根元素
            （片段会自动剥掉 lxml 补出来的 ``<html>/<body>``）。
        index: 第几个匹配结果，从 1 开始，负数从后往前数。

    Returns:
        StaticElement 或 NoneElement。
    """
    doc = _parse_document(html)
    if doc is None:
        return _none()

    if locator is None:
        root = _fragment_root(doc, html)
        return StaticElement(node=root) if root is not None else _none()

    node = _pick(_find_nodes(doc, locator, False), index)
    return StaticElement(node=node) if node is not None else _none()


def make_static_eles(html, locator):
    """从 HTML 字符串解析出所有匹配的静态元素。

    Args:
        html: HTML 字符串。
        locator: 定位器。

    Returns:
        list[StaticElement]: 按文档序排列，没找到返回空列表。
    """
    doc = _parse_document(html)
    if doc is None:
        return []
    return [StaticElement(node=n) for n in _find_nodes(doc, locator, False)]
