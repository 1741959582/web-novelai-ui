# -*- coding: utf-8 -*-
"""ScriptTools - script 模块高层结果与工具。"""


class RealmInfo(object):
    """单个 realm 信息对象。

    Args:
        data: ``script.getRealms`` 返回的单个 realm 字典。

    Returns:
        RealmInfo: 支持属性访问的 realm 对象。
    """

    def __init__(self, data):
        self.raw = dict(data or {})
        self.realm = self.raw.get("realm")
        self.type = self.raw.get("type")
        self.context = self.raw.get("context")
        self.origin = self.raw.get("origin")


class ScriptRemoteValue(object):
    """script.evaluate / callFunction 的远程值结果对象。

    常用属性：
        - ``handle``: 远程对象句柄，可用于后续 ``script.disown``
        - ``shared_id``: 共享引用 ID，常见于节点引用
        - ``value``: 已序列化后的普通值
    """

    def __init__(self, data):
        self.raw = dict(data or {})
        self.type = self.raw.get("type")
        self.handle = self.raw.get("handle")
        self.shared_id = self.raw.get("sharedId")
        self.value = self.raw.get("value")


class ScriptResult(object):
    """脚本执行结果对象。

    适用场景：
        - 用 ``success`` 判断脚本是否执行成功
        - 用 ``result.handle`` 拿到远程对象句柄
        - 用 ``result.value`` 读取普通返回值
    """

    def __init__(self, data):
        self.raw = dict(data or {})
        self.type = self.raw.get("type")
        self.realm = self.raw.get("realm")
        self.result = ScriptRemoteValue(self.raw.get("result", {}))
        self.exception_details = self.raw.get("exceptionDetails")

    @property
    def success(self):
        """本次脚本执行是否成功。

        Returns:
            bool: ``True`` 表示脚本返回 success，``False`` 表示有异常或失败。
        """
        return self.type == "success"


class PreloadScript(object):
    """预加载脚本结果对象。

    Args:
        script_id: 预加载脚本 ID。

    Returns:
        PreloadScript: 支持 ``id`` 属性访问的结果对象。

    适用场景：
        - 保存 `script.addPreloadScript` 返回值
        - 后续用 ``page.remove_preload_script(preload.id)`` 清理
    """

    def __init__(self, script_id):
        self.id = script_id or ""
