# -*- coding: utf-8 -*-
"""ContextManager - browsingContext 高层管理器。"""

from .._bidi import browsing_context as bidi_context


class ContextInfo(object):
    """单个 browsingContext 信息对象。

    Args:
        data: BiDi 返回的单个 context 信息字典。

    Returns:
        ContextInfo: 支持属性访问的 context 信息对象。

    适用场景：
        - 让编辑器可以通过属性跳转和补全
        - 在示例中避免反复写 ``item.get('context')`` 这类不直观代码
    """

    def __init__(self, data):
        self.raw = dict(data or {})
        self.context = self.raw.get("context")
        self.url = self.raw.get("url")
        self.user_context = self.raw.get("userContext")
        self.parent = self.raw.get("parent")
        self.original_opener = self.raw.get("originalOpener")
        self.client_window = self.raw.get("clientWindow")
        self.children = [ContextInfo(i) for i in self.raw.get("children", [])]

    def __repr__(self):
        return "<ContextInfo {} {}>".format(self.context or "", (self.url or "")[:80])


class ContextTree(object):
    """浏览上下文树结果对象。

    Args:
        data: ``browsingContext.getTree`` 原始返回字典。

    Returns:
        ContextTree: 支持属性访问的上下文树对象。

    适用场景：
        - 让 ``tree.contexts`` 可以直接跳转和补全
        - 避免示例继续写 ``tree.get('contexts', [])``
    """

    def __init__(self, data):
        self.raw = dict(data or {})
        self.contexts = [ContextInfo(i) for i in self.raw.get("contexts", [])]

    def __repr__(self):
        return "<ContextTree contexts={}>".format(len(self.contexts))


class ContextManager(object):
    """browsingContext 高层管理器。

    这是面向使用者的上下文管理入口，用来替代直接调用
    ``_bidi.browsing_context`` 和手动传 ``page._driver`` 的写法。

    主要解决三类问题：
    1. 查询当前浏览器上下文树
    2. 创建、关闭 tab/window
    3. 操作当前页面对应 context 的 reload、viewport、CSP 绕过等能力
    """

    def __init__(self, owner):
        self._owner = owner

    def get_tree(self, max_depth=None, root=None):
        """获取浏览上下文树。

        Args:
            max_depth: 返回树的最大深度。
                单位：层级深度整数。
                常见值：``0`` 只看顶层、``1`` 看顶层及一层子节点。
            root: 可选的根 browsingContext ID。
                常见值：某个 tab 或 window 的 context ID。传 ``None`` 表示从顶层开始。

        Returns:
            ContextTree: 支持通过 ``tree.contexts`` 访问上下文列表的结果对象。

        适用场景：
            - 查看当前浏览器有哪些 context
            - 检查新建 tab/window 后是否进入上下文树
            - 调试多标签页或嵌套 frame 结构
        """
        return ContextTree(
            bidi_context.get_tree(self._owner._driver, max_depth=max_depth, root=root)
        )

    def create_tab(self, background=False, user_context=None, reference_context=None):
        """创建新标签页。

        Args:
            background: 是否后台创建。
                常见值：``False`` 前台、``True`` 后台。
            user_context: 可选的 user context ID。
                常见值：Firefox 容器标签页 ID。用于在指定 user context 中创建 tab。
            reference_context: 参考 browsingContext ID。
                常见值：当前页面的 ``page.tab_id``。

        Returns:
            str: 新建 tab 的 browsingContext ID。

        适用场景：
            - 新建 tab
            - 在指定 user context 中创建隔离 tab
        """
        result = bidi_context.create(
            self._owner._driver,
            type_="tab",
            reference_context=reference_context,
            background=background,
            user_context=user_context,
        )
        return result.get("context")

    def create_window(self, background=False, user_context=None):
        """创建新窗口。

        Args:
            background: 是否后台创建。
                常见值：``False`` 前台、``True`` 后台。
            user_context: 可选的 user context ID。
                常见值：Firefox 容器标签页 ID。

        Returns:
            str: 新建 window 的 browsingContext ID。

        适用场景：
            - 测试多窗口创建
            - 在指定 user context 中创建独立窗口
        """
        result = bidi_context.create(
            self._owner._driver,
            type_="window",
            background=background,
            user_context=user_context,
        )
        return result.get("context")

    def close(self, context=None, prompt_unload=False):
        """关闭指定或当前 browsingContext。

        Args:
            context: 要关闭的 browsingContext ID。
                常见值：``create_tab()`` / ``create_window()`` 返回值。
                传 ``None`` 表示关闭当前页面对应的 context。
            prompt_unload: 是否允许触发 ``beforeunload`` 提示流程。
                常见值：``False`` 直接关闭、``True`` 按浏览器卸载流程处理。

        Returns:
            owner: 原页面对象，便于链式调用。

        适用场景：
            - 关闭测试中临时创建的 tab/window
            - 关闭当前页面 context
        """
        bidi_context.close(
            self._owner._driver,
            context or self._owner.tab_id,
            prompt_unload=prompt_unload,
        )
        return self._owner

    def reload(self, ignore_cache=False, wait="complete", context=None):
        """重载指定或当前 context。

        Args:
            ignore_cache: 是否忽略缓存。
                常见值：``False`` 普通重载、``True`` 类似强制刷新。
            wait: 重载后的等待策略。
                常见值：``'none'``、``'interactive'``、``'complete'``。
            context: 可选的 browsingContext ID。
                传 ``None`` 表示使用当前页面的 ``page.tab_id``。

        Returns:
            dict: 通常包含 ``navigation`` 和 ``url`` 字段。

        适用场景：
            - 验证 reload 命令本身
            - 测试 ignore_cache 在当前浏览器版本是否实现
        """
        return bidi_context.reload(
            self._owner._driver,
            context=context or self._owner.tab_id,
            ignore_cache=ignore_cache,
            wait=wait,
        )

    def set_viewport(self, width, height, device_pixel_ratio=None, context=None):
        """设置指定或当前 context 的视口大小。

        Args:
            width: 视口宽度。
                单位：CSS 像素。
                常见值：``800``、``1280``、``375``。
            height: 视口高度。
                单位：CSS 像素。
                常见值：``600``、``720``、``667``。
            device_pixel_ratio: 设备像素比。
                常见值：``1``、``2``、``3``。传 ``None`` 表示不改动当前 DPR。
            context: 可选的 browsingContext ID。
                传 ``None`` 表示使用当前页面的 ``page.tab_id``。

        Returns:
            owner: 原页面对象，便于链式调用。

        适用场景：
            - 快速调整当前页面视口
            - 针对某个指定 context 设置 viewport
        """
        kwargs = {
            "context": context or self._owner.tab_id,
            "width": width,
            "height": height,
        }
        if device_pixel_ratio is not None:
            kwargs["device_pixel_ratio"] = device_pixel_ratio
        bidi_context.set_viewport(self._owner._driver, **kwargs)
        return self._owner

    def set_bypass_csp(self, enabled=True, context=None):
        """调用标准 browsingContext.setBypassCSP。

        Args:
            enabled: 是否启用绕过。
                常见值：``True`` 启用、``False`` 禁用。
            context: 可选的 browsingContext ID。
                传 ``None`` 表示使用当前页面的 ``page.tab_id``。

        Returns:
            dict: BiDi 命令返回结果，通常为空字典。

        适用场景：
            - 直接验证标准 ``browsingContext.setBypassCSP`` 是否支持
            - 不想使用兼容式 preload script，而是测试浏览器原生命令
        """
        return bidi_context.set_bypass_csp(
            self._owner._driver,
            context=context or self._owner.tab_id,
            enabled=enabled,
        )

    def start_screencast(
        self,
        context=None,
        mime_type=None,
        video=None,
        audio=None,
    ):
        """开始录制指定或当前 browsingContext。

        Args:
            context: 可选的顶层 browsingContext ID，默认使用当前页面。
            mime_type: 可选媒体类型，例如 ``"video/webm"``。
            video: 可选视频约束，支持 ``width``、``height``、``frameRate``。
            audio: 是否同时录制页面音频。

        Returns:
            dict: 包含 ``screencast`` ID 和远端 ``path``。
        """
        return bidi_context.start_screencast(
            self._owner._driver,
            context=context or self._owner.tab_id,
            mime_type=mime_type,
            video=video,
            audio=audio,
        )

    def stop_screencast(self, screencast):
        """停止录制并返回远端生成文件的路径。"""
        return bidi_context.stop_screencast(
            self._owner._driver,
            screencast=screencast,
        )
