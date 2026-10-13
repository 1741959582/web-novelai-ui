# -*- coding: utf-8 -*-
"""Event Emitter — BiDi 事件分发层

职责：
- 维护 (event_method, context) → callback 映射
- 事件队列 + 后台消费线程（避免阻塞 recv 循环）
- immediate 模式：在短生命周期线程中立即执行（用于 alert 等关键事件）
- 通配符订阅：context=None 匹配所有 context
"""

import threading
import logging
from queue import Queue, Empty

logger = logging.getLogger('ruyipage')


class EventEmitter:
    """BiDi 事件发射器"""

    def __init__(self):
        self._handlers = {}           # {(method, ctx): callback}
        self._immediate = {}          # {(method, ctx): callback}
        self._lock = threading.RLock()
        self._queue = Queue()
        self._running = False
        self._consumer = None

    # ── 生命周期 ──────────────────────────────────────────────────────────

    def start(self):
        if self._running:
            return
        self._running = True
        self._consumer = threading.Thread(
            target=self._consume_loop,
            name='bidi-event-consumer',
            daemon=True,
        )
        self._consumer.start()

    def stop(self):
        self._running = False
        self._queue.put(None)  # 唤醒消费线程

    # ── 注册/注销 ─────────────────────────────────────────────────────────

    def on(self, event: str, callback, context=None, immediate=False):
        """注册事件回调

        Args:
            event: BiDi 事件名，如 'network.responseCompleted'
            callback: 回调函数 callback(params: dict)
            context: browsingContext ID，None 匹配所有
            immediate: True=在独立短线程立即执行，False=走事件队列
        """
        key = (event, context)
        with self._lock:
            target = self._immediate if immediate else self._handlers
            target[key] = callback
        logger.debug('EventEmitter 注册 %s ctx=%s immediate=%s',
                     event, context, immediate)

    def off(self, event: str, context=None, immediate=False):
        """注销事件回调"""
        key = (event, context)
        with self._lock:
            target = self._immediate if immediate else self._handlers
            target.pop(key, None)

    # ── 事件投递（由 Transport/Driver 调用）──────────────────────────────

    def emit(self, event: str, context, params: dict):
        """投递一个 BiDi 事件

        Args:
            event: 事件方法名
            context: 事件来源 context ID（可为 None）
            params: 事件参数字典
        """
        # immediate 回调：在短线程立即执行
        with self._lock:
            imm_items = list(self._immediate.items())

        for (evt, ctx), cb in imm_items:
            if evt == event and (ctx is None or ctx == context):
                self._run_immediate(cb, params, event)

        # 普通回调：入队
        self._queue.put((event, context, params))

    # ── 内部 ──────────────────────────────────────────────────────────────

    def _consume_loop(self):
        """后台消费线程"""
        while self._running:
            try:
                item = self._queue.get(timeout=1)
            except Empty:
                continue
            if item is None:
                break
            event, context, params = item
            with self._lock:
                items = list(self._handlers.items())
            for (evt, ctx), cb in items:
                if evt == event and (ctx is None or ctx == context):
                    try:
                        cb(params)
                    except Exception as e:
                        logger.error('事件回调异常 %s: %s', event, e)
        logger.debug('EventEmitter 消费线程退出')

    def _run_immediate(self, cb, params, event):
        def _run():
            try:
                cb(params)
            except Exception as e:
                logger.error('Immediate 回调异常 %s: %s', event, e)
        t = threading.Thread(target=_run, name='bidi-imm-evt', daemon=True)
        t.start()
