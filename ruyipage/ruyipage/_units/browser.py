# -*- coding: utf-8 -*-
"""BrowserManager - 浏览器级能力管理器。"""

from .._bidi import browser_module as bidi_browser
from .._bidi import browsing_context as bidi_context


class BrowserManager(object):
    """浏览器级高层管理器。

    它负责的是 browser 模块能力，而不是当前页面 DOM 操作。

    主要解决三类问题：
    1. user context 管理
       例如：创建一个新的隔离上下文，再在里面打开 tab。
    2. client window 管理
       例如：读取当前所有窗口，再切换其中一个窗口为最大化或全屏。
    3. 新手屏蔽底层 BiDi 细节
       例如：不必直接碰 ``_bidi.browser_module`` 或自己拼参数。
    """

    def __init__(self, owner):
        self._owner = owner

    def create_user_context(self):
        """创建一个新的 browser user context。

        你可以把它理解为“新建一个隔离的浏览器容器”。
        后续可以把 tab 放到这个容器里运行，使 Cookie、存储、下载策略等与默认上下文分离。

        Returns:
            str: 新建 user context ID。

        适用场景：
            - 模拟 Firefox 容器标签页隔离环境
            - 针对 user context 验证 Cookie / 存储隔离
        """
        return bidi_browser.create_user_context(self._owner._driver).get("userContext")

    def get_user_contexts(self):
        """获取所有 browser user context。

        返回的是浏览器当前已知的所有 user context 信息，而不只是你本轮创建的那个。

        Returns:
            list[dict]: user context 信息列表。

        适用场景：
            - 查看当前已有的 user context
            - 对比创建前后的数量变化
        """
        return bidi_browser.get_user_contexts(self._owner._driver).get(
            "userContexts", []
        )

    def remove_user_context(self, user_context):
        """删除指定 user context。

        删除后，该 user context 不应再出现在 ``get_user_contexts()`` 结果中。

        Args:
            user_context: 目标 user context ID。
                常见值：``create_user_context()`` 返回值。

        Returns:
            owner: 原页面对象，便于链式调用。

        适用场景：
            - 清理测试中临时创建的 user context
            - 验证 user context 生命周期管理
        """
        bidi_browser.remove_user_context(self._owner._driver, user_context=user_context)
        return self._owner

    def create_tab(self, user_context=None, background=False):
        """创建新标签页。

        这是对 ``browsingContext.create(type='tab')`` 的更易懂封装。

        Args:
            user_context: 可选的 user context ID。
                常见值：某个容器标签页 ID。传 ``None`` 表示默认上下文。
            background: 是否后台创建。
                常见值：``False`` 前台、``True`` 后台。

        Returns:
            str: 新建 tab 的 browsingContext ID。

        适用场景：
            - 在指定 user context 中创建 tab
            - 验证 tab 与 user context 的归属关系
        """
        result = bidi_context.create(
            self._owner._driver,
            type_="tab",
            background=background,
            user_context=user_context,
        )
        return result.get("context")

    def get_client_windows(self):
        """获取所有客户端窗口信息。

        返回结果一般包含：窗口 ID、状态、尺寸、位置等信息。

        Returns:
            list[dict]: client window 信息列表。

        适用场景：
            - 读取当前窗口状态、尺寸、位置
            - 多窗口环境中枚举全部 client window
        """
        return bidi_browser.get_client_windows(self._owner._driver).get(
            "clientWindows", []
        )

    def set_window_state(
        self, client_window, state="normal", width=None, height=None, x=None, y=None
    ):
        """设置浏览器窗口状态或几何信息。

        当你只想操作“当前窗口”时，更简单的入口通常是 ``page.window``。
        当你已经拿到了某个 ``clientWindow`` ID，并想精确控制某个窗口时，用这个方法更合适。

        Args:
            client_window: client window ID。
                常见值：``get_client_windows()[0]['clientWindow']``。
            state: 目标窗口状态。
                常见值：``'normal'``、``'minimized'``、``'maximized'``、``'fullscreen'``。
            width: 目标窗口宽度。
                单位：像素。常见值：``1280``、``1440``。
            height: 目标窗口高度。
                单位：像素。常见值：``720``、``900``。
            x: 目标窗口横坐标。
                单位：屏幕像素。常见值：``0``、``100``。
            y: 目标窗口纵坐标。
                单位：屏幕像素。常见值：``0``、``80``。

        Returns:
            dict: 更新后的 ``ClientWindowInfo``。

        适用场景：
            - 验证窗口状态切换
            - 设置窗口大小与位置
            - 在多窗口场景里精准操作指定窗口
        """
        return bidi_browser.set_client_window_state(
            self._owner._driver,
            client_window=client_window,
            state=state,
            width=width,
            height=height,
            x=x,
            y=y,
        )
