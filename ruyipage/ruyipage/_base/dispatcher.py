# -*- coding: utf-8 -*-
"""Command Dispatcher — BiDi 命令分发层

职责：
- 生成唯一命令 ID
- 维护 pending 命令的响应队列
- 将 Transport 收到的响应路由到对应等待者
- 同步阻塞等待响应（Queue.get with timeout）
"""

import json
import threading
import logging
from queue import Queue, Empty

from ..errors import BiDiError, PageDisconnectedError

logger = logging.getLogger('ruyipage')


class CommandDispatcher:
    """BiDi 命令分发器

    与 BiDiTransport 配合：
      dispatcher.dispatch(transport, method, params) → result dict
    """

    def __init__(self):
        self._cur_id = 0
        self._id_lock = threading.Lock()
        self._pending = {}          # {cmd_id: Queue}
        self._pending_lock = threading.Lock()

    # ── 核心接口 ──────────────────────────────────────────────────────────

    def dispatch(self, transport, method: str, params: dict = None,
                 timeout: float = 30) -> dict:
        """发送命令并同步等待响应

        Args:
            transport: BiDiTransport 实例
            method: BiDi 方法名，如 'browsingContext.navigate'
            params: 参数字典
            timeout: 超时（秒）

        Returns:
            响应的 result 字典

        Raises:
            BiDiError: 协议错误
            PageDisconnectedError: 连接断开
        """
        cmd_id = self._next_id()
        q = Queue()
        with self._pending_lock:
            self._pending[cmd_id] = q

        msg = {'id': cmd_id, 'method': method, 'params': params or {}}
        try:
            transport.send(msg)
            logger.debug('Dispatcher 发送 id=%d %s', cmd_id, method)
        except Exception as e:
            with self._pending_lock:
                self._pending.pop(cmd_id, None)
            raise PageDisconnectedError('命令发送失败: {}'.format(e))

        # 阻塞等待
        try:
            result = q.get(timeout=timeout)
        except Empty:
            with self._pending_lock:
                self._pending.pop(cmd_id, None)
            raise BiDiError('timeout',
                            '命令超时: {} ({}s)'.format(method, timeout))

        with self._pending_lock:
            self._pending.pop(cmd_id, None)

        if result is None:
            raise PageDisconnectedError('连接已断开（命令 {} 未收到响应）'.format(method))

        if result.get('type') == 'error':
            raise BiDiError(
                result.get('error', 'unknown'),
                result.get('message', ''),
                result.get('stacktrace', ''),
            )

        return result.get('result', {})

    def on_response(self, msg: dict):
        """Transport 收到命令响应时调用

        Args:
            msg: 已解析的响应字典（含 'id' 字段）
        """
        cmd_id = msg.get('id')
        if cmd_id is None:
            return
        with self._pending_lock:
            q = self._pending.get(cmd_id)
        if q:
            q.put(msg)
            logger.debug('Dispatcher 路由响应 id=%d', cmd_id)
        else:
            logger.debug('Dispatcher 收到未知 id=%d 的响应', cmd_id)

    def wake_all(self):
        """连接断开时唤醒所有等待者（放入 None 触发 PageDisconnectedError）"""
        with self._pending_lock:
            for q in self._pending.values():
                try:
                    q.put_nowait(None)
                except Exception:
                    pass
            self._pending.clear()

    # ── 内部 ──────────────────────────────────────────────────────────────

    def _next_id(self) -> int:
        with self._id_lock:
            self._cur_id += 1
            return self._cur_id
