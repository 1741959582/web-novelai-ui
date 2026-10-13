# -*- coding: utf-8 -*-
"""RuyiPage 异常层次"""


class RuyiPageError(Exception):
    """RuyiPage 基础异常"""
    diagnostics = None  # FailureSnapshot | None，由框架在 raise 前注入


class ElementNotFoundError(RuyiPageError):
    """元素未找到"""
    pass


class ElementLostError(RuyiPageError):
    """元素引用失效（页面导航后 sharedId 失效）"""
    pass


class ContextLostError(RuyiPageError):
    """浏览上下文已销毁（标签页被关闭等）"""
    pass


class BiDiError(RuyiPageError):
    """BiDi 协议错误"""

    def __init__(self, error, message='', stacktrace=''):
        self.error = error
        self.bidi_message = message
        self.stacktrace = stacktrace
        super().__init__('{}: {}'.format(error, message))


class PageDisconnectedError(RuyiPageError):
    """WebSocket 连接断开"""
    pass


class JavaScriptError(RuyiPageError):
    """JavaScript 执行异常"""

    def __init__(self, message='', exception_details=None):
        self.exception_details = exception_details
        super().__init__(message)


class BrowserConnectError(RuyiPageError):
    """无法连接到浏览器"""
    pass


class BrowserLaunchError(RuyiPageError):
    """无法启动浏览器"""
    pass


class AlertExistsError(RuyiPageError):
    """对话框阻塞操作"""
    pass


class WaitTimeoutError(RuyiPageError):
    """等待超时"""
    pass


class NoRectError(RuyiPageError):
    """元素没有可视区域"""
    pass


class ElementGeometryError(RuyiPageError):
    """无法从浏览器读取元素几何信息

    与 :class:`NoRectError` 的区别：``NoRectError`` 表示元素确实没有可视区域，
    本异常表示读取动作自身失败（BiDi 未返回结果或对象序列化被截断），
    此时元素的真实尺寸未知。
    """
    pass


class CanNotClickError(RuyiPageError):
    """元素不可点击"""
    pass


class LocatorError(RuyiPageError):
    """定位器语法错误"""
    pass


class IncorrectURLError(RuyiPageError):
    """URL 格式错误"""
    pass


class NetworkInterceptError(RuyiPageError):
    """网络拦截失败"""
    pass


class DebuggerError(RuyiPageError):
    """DevTools 调试通道（RDP）错误"""
    pass
