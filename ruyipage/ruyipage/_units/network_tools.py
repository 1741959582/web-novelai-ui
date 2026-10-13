# -*- coding: utf-8 -*-
"""NetworkManager - network 模块高层管理器。

通过 ``page.network`` 访问。提供网络配置和数据采集能力，
不涉及请求拦截（拦截请用 ``page.intercept``）。

主要功能
--------
1. **额外请求头** — 为当前页面所有后续请求自动附加指定头::

       page.network.set_extra_headers({"X-Token": "abc123"})
       page.get("https://api.example.com")
       page.network.clear_extra_headers()

2. **缓存控制** — 绕过浏览器缓存确保请求真正发出::

       page.network.set_cache_behavior("bypass")

3. **数据采集** — 创建 DataCollector 收集请求体/响应体::

       collector = page.network.add_data_collector(
           data_types=["response"]
       )
       # ... 触发请求 ...
       data = collector.get(request_id, data_type="response")
       print(data.bytes)
       collector.remove()

.. tip::

    在多数场景下，你可以用 ``page.intercept.start_requests(collect_response=True)``
    配合 ``req.response_body`` 一步读取响应体，无需手动管理 DataCollector。
    DataCollector 适合需要精细控制或批量采集的高级场景。
"""

import warnings

from .._bidi import network as bidi_network


class NetworkData(object):
    """单次 ``network.getData`` 返回的结果对象。

    对 BiDi 原始返回字典的属性封装，提供更友好的字段访问方式。

    Attributes:
        raw (dict): BiDi 原始返回数据。
        bytes: 响应体的 bytes 值（BiDi 格式 dict 或 None）。
        base64: 响应体的 base64 值（BiDi 格式 dict 或 None）。

    Examples::

        data = collector.get(request_id, data_type="response")
        if data.has_data:
            print(data.bytes)    # {"type": "string", "value": "..."}
            print(data.base64)   # {"type": "base64", "value": "..."}
        else:
            print("未采集到数据")
            print(data.raw)      # 查看原始返回
    """

    def __init__(self, data):
        self.raw = dict(data or {})
        self.bytes = self.raw.get("bytes")
        self.base64 = self.raw.get("base64")

    @property
    def has_data(self):
        """是否成功获取到网络数据。

        Returns:
            bool: ``True`` 表示 ``bytes`` 或 ``base64`` 中至少有一个不为 None。

        Examples::

            data = collector.get(request_id)
            if data.has_data:
                # 处理数据
                ...
            else:
                print("未采集到数据，可能请求尚未完成")
        """
        return self.bytes is not None or self.base64 is not None


class DataCollector(object):
    """网络数据收集器句柄。

    由 ``page.network.add_data_collector()`` 创建，用于采集请求体和响应体。

    你可以把它理解为浏览器侧的"数据缓冲区"——浏览器把命中的网络数据
    交给 collector 保留，你随后按 ``request_id`` 取回。

    生命周期::

        # 1. 创建
        collector = page.network.add_data_collector(
            data_types=["response"]
        )

        # 2. 触发网络请求...

        # 3. 按 request_id 读取
        data = collector.get(request_id, data_type="response")

        # 4. 释放单条数据（可选，释放浏览器内存）
        collector.disown(request_id)

        # 5. 移除整个 collector
        collector.remove()

    Attributes:
        id (str): 收集器 ID，由浏览器分配。

    .. tip::

        在多数场景下，你可以用 ``page.intercept.start_requests(collect_response=True)``
        配合 ``req.response_body`` 一步读取响应体，内部自动管理 DataCollector。
    """

    def __init__(self, manager, collector_id):
        self._manager = manager
        self.id = collector_id

    def get(self, request_id, data_type="response"):
        """按 ``request_id`` 读取采集到的数据。

        Args:
            request_id: 请求唯一 ID。

                通常来源于 ``InterceptedRequest.request_id``
                或 ``DataPacket.request.get("request")``。

            data_type: 要读取的数据类型。

                - ``'response'`` — 读取响应体（默认）。
                - ``'request'`` — 读取请求体。

        Returns:
            NetworkData: 包含 ``bytes``/``base64``/``raw`` 属性的结果对象。

        Examples::

            # 典型用法：拦截 + 采集响应体
            collector = page.network.add_data_collector(
                data_types=["response"]
            )
            # ... 拦截请求获取 request_id ...
            data = collector.get(req.request_id, data_type="response")
            if data.has_data:
                print(data.bytes)
        """
        return self._manager.get_data(self.id, request_id, data_type=data_type)

    def disown(self, request_id, data_type="response"):
        """释放指定请求的采集数据（释放浏览器内存）。

        释放后再调用 ``get()`` 将拿不到数据。适用于大量请求场景，
        已处理完的数据主动释放以控制内存占用。

        Args:
            request_id: 请求唯一 ID。
            data_type: 要释放的数据类型，``'response'`` 或 ``'request'``。

        Examples::

            data = collector.get(req.request_id)
            # 处理完毕后释放
            collector.disown(req.request_id)
        """
        return self._manager.disown_data(self.id, request_id, data_type=data_type)

    def remove(self):
        """移除整个收集器。

        移除后不再采集任何数据。通常在 ``finally`` 块中调用以确保清理。

        Examples::

            collector = page.network.add_data_collector(...)
            try:
                # ... 使用 collector ...
                pass
            finally:
                collector.remove()
        """
        return self._manager.remove_data_collector(self.id)


class NetworkManager(object):
    """network 模块高层管理器。

    通过 ``page.network`` 访问。提供额外请求头、缓存控制和数据采集功能。

    Examples::

        # 设置额外请求头
        page.network.set_extra_headers({"X-Token": "abc123", "X-Lang": "zh"})

        # 绕过缓存
        page.network.set_cache_behavior("bypass")

        # 数据采集
        collector = page.network.add_data_collector(
            data_types=["response"]
        )
    """

    def __init__(self, owner):
        self._owner = owner

    def _ctx(self):
        return [self._owner.tab_id]

    def set_extra_headers(self, headers):
        """为当前页面的所有后续请求自动附加额外请求头。

        这些头会被浏览器自动注入到每个请求中，无需拦截。
        调用 ``clear_extra_headers()`` 清除。

        Args:
            headers: 请求头字典。

                示例::

                    page.network.set_extra_headers({
                        "X-Token": "abc123",
                        "X-Request-Source": "ruyipage",
                    })

        Returns:
            owner: 原页面对象，便于链式调用。

        Examples::

            # 设置额外头 → 访问页面 → 清除
            page.network.set_extra_headers({"X-Token": "abc123"})
            page.get("https://api.example.com/data")
            page.network.clear_extra_headers()

            # 链式调用
            page.network.set_extra_headers({"X-Lang": "zh"}).get("https://example.com")
        """
        bidi_headers = []
        for name, value in headers.items():
            bidi_headers.append(
                {"name": name, "value": {"type": "string", "value": str(value)}}
            )
        bidi_network.set_extra_headers(
            self._owner._driver,
            headers=bidi_headers,
            contexts=self._ctx(),
        )
        return self._owner

    def clear_extra_headers(self):
        """清空当前页面的所有额外请求头。

        Returns:
            owner: 原页面对象，便于链式调用。

        Examples::

            page.network.set_extra_headers({"X-Token": "abc123"})
            # ... 操作 ...
            page.network.clear_extra_headers()
        """
        bidi_network.set_extra_headers(
            self._owner._driver, headers=[], contexts=self._ctx()
        )
        return self._owner

    def set_cache_behavior(self, behavior="default"):
        """设置当前页面的缓存行为。

        Args:
            behavior: 缓存策略。

                - ``'default'`` — 使用浏览器默认缓存策略。
                  命中缓存时不发起真实网络请求，
                  不会触发 ``responseCompleted`` 事件。
                - ``'bypass'`` — 绕过缓存，强制向服务器请求资源。
                  确保每次都有真实网络请求和对应的网络事件。

        Returns:
            owner: 原页面对象，便于链式调用。

        Examples::

            # 绕过缓存（适合需要监听每次请求的场景）
            page.network.set_cache_behavior("bypass")
            page.get("https://example.com")

            # 恢复默认缓存
            page.network.set_cache_behavior("default")
        """
        bidi_network.set_cache_behavior(
            self._owner._driver,
            behavior=behavior,
            contexts=self._ctx(),
        )
        return self._owner

    def add_data_collector(
        self, events=None, *, data_types=None, max_encoded_data_size=10485760
    ):
        """创建网络数据收集器。

        收集器会在指定的网络事件阶段自动保存请求体/响应体数据，
        随后可通过 ``collector.get(request_id)`` 按需读取。

        Args:
            events: 旧版兼容参数；当前 W3C 协议由 ``data_types`` 决定数据类型。

            data_types: 数据类型列表。决定保留哪些数据。

                - ``['request']`` — 只保留请求体。
                - ``['response']`` — 只保留响应体。
                - ``['request', 'response']`` — 两者都保留。
                - ``None`` — 默认两者都保留。

            max_encoded_data_size: 单次采集的最大编码数据大小（字节）。
                默认 ``10485760``（10 MB）。超过此大小的数据会被截断。

        Returns:
            DataCollector: 收集器句柄，提供 ``get()``/``disown()``/``remove()`` 方法。

        Examples::

            # 采集响应体
            collector = page.network.add_data_collector(
                data_types=["response"],
            )
            # ... 触发请求，获取 request_id ...
            data = collector.get(request_id, data_type="response")
            if data.has_data:
                print(data.bytes)
            collector.remove()

            # 同时采集请求体和响应体
            collector = page.network.add_data_collector(
                data_types=["request", "response"],
            )
        """
        if events is not None:
            warnings.warn(
                "events is deprecated by the current WebDriver BiDi schema; "
                "use data_types instead",
                DeprecationWarning,
                stacklevel=2,
            )
        return self._add_data_collector(
            data_types=data_types,
            max_encoded_data_size=max_encoded_data_size,
            contexts=self._ctx(),
        )

    def _add_data_collector(
        self,
        *,
        data_types=None,
        max_encoded_data_size=10485760,
        contexts=None,
    ):
        """Create a collector with an explicit scope for internal fallbacks."""
        result = bidi_network.add_data_collector(
            self._owner._driver,
            contexts=contexts,
            max_encoded_data_size=max_encoded_data_size,
            data_types=data_types,
        )
        return DataCollector(self, result.get("collector"))

    def remove_data_collector(self, collector_id):
        """按 ID 移除数据收集器。

        通常通过 ``collector.remove()`` 调用，这里是底层方法。

        Args:
            collector_id: 收集器 ID。

        Returns:
            owner: 原页面对象。
        """
        bidi_network.remove_data_collector(self._owner._driver, collector_id)
        return self._owner

    def get_data(self, collector_id, request_id, data_type="response"):
        """从收集器读取指定请求的数据。

        通常通过 ``collector.get()`` 调用，这里是底层方法。

        Args:
            collector_id: 收集器 ID。
            request_id: 请求唯一 ID。
            data_type: 数据类型，``'request'`` 或 ``'response'``。

        Returns:
            NetworkData: 包含 ``bytes``/``base64``/``has_data`` 的结果对象。
        """
        result = bidi_network.get_data(
            self._owner._driver,
            collector_id,
            request_id,
            data_type=data_type,
        )
        return NetworkData(result)

    def disown_data(self, collector_id, request_id, data_type="response"):
        """释放收集器中指定请求的数据。

        释放后该请求对应的数据不再可读，浏览器回收相应内存。
        通常通过 ``collector.disown()`` 调用，这里是底层方法。

        Args:
            collector_id: 收集器 ID。
            request_id: 请求唯一 ID。
            data_type: 数据类型，``'request'`` 或 ``'response'``。

        Returns:
            dict: BiDi 命令返回结果，通常为空字典。
        """
        return bidi_network.disown_data(
            self._owner._driver,
            collector_id,
            request_id,
            data_type=data_type,
        )
