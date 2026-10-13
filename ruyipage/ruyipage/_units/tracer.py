# -*- coding: utf-8 -*-
"""统一 Debug Trace — 结构化操作时间线与失败诊断快照

提供两大功能：

1. **Trace 时间线** — 记录所有 BiDi 命令、事件、网络拦截、关键异常，
   形成可回溯的操作流水线。

2. **FailureSnapshot** — 自动化失败时的诊断快照数据容器，
   包含截图路径、DOM 快照路径、URL、最近网络请求和 trace 记录。

设计要点
--------
- Tracer 实例挂在 BrowserBiDiDriver 上（browser 级单例），
  所有 tab/frame 共享同一个 trace 缓冲区。
- page.trace 是代理属性，委托到 browser 级 Tracer。
- 网络事件通过 _recv_loop 被动钩子记录，不使用 set_callback，
  与 page.listen / page.intercept 零冲突。

Usage::

    from ruyipage import Settings
    Settings.trace_enabled = True

    page = FirefoxPage(opts)
    page.get('https://example.com')

    # 查看 trace
    print(page.trace.summary())
    print(page.trace.dump_json())
    entries = page.trace.latest(20)

    # 失败快照（由框架自动注入到异常的 .diagnostics 属性）
    try:
        page.ele('#missing', timeout=3)
    except ElementNotFoundError as e:
        if e.diagnostics:
            print(e.diagnostics.summary())
"""

import time
import json
import threading
from collections import deque


# ─── 敏感字段清洗 ───

_SENSITIVE_KEYS = frozenset({
    'password', 'credentials', 'cookie', 'authorization',
    'set-cookie', 'proxy-authorization', 'x-api-key',
    'secret', 'token',
})


def _scrub_dict(d, max_str_len=500):
    """递归清洗 dict 中的敏感字段，截断超长字符串。

    Args:
        d: 待清洗的值（dict/str/其他）
        max_str_len: 字符串最大长度，超出则截断

    Returns:
        清洗后的值
    """
    if not isinstance(d, dict):
        if isinstance(d, str) and len(d) > max_str_len:
            return d[:max_str_len] + '...[truncated {}]'.format(len(d))
        return d
    out = {}
    for k, v in d.items():
        if isinstance(k, str) and k.lower() in _SENSITIVE_KEYS:
            out[k] = '***'
        elif isinstance(v, dict):
            out[k] = _scrub_dict(v, max_str_len)
        elif isinstance(v, list):
            out[k] = [
                _scrub_dict(i, max_str_len) if isinstance(i, dict) else i
                for i in v[:10]
            ]
        elif isinstance(v, str) and len(v) > max_str_len:
            out[k] = v[:max_str_len] + '...[truncated {}]'.format(len(v))
        else:
            out[k] = v
    return out


def _summarize_params(params):
    """将 BiDi params 精简为安全摘要。

    不记录完整参数（可能包含大量 base64 数据或敏感信息），
    只保留关键字段的截断摘要。

    Args:
        params: BiDi 命令参数 dict

    Returns:
        dict: 清洗并截断后的摘要
    """
    if not params:
        return {}
    return _scrub_dict(params)


class TraceEntry:
    """单条 trace 记录。

    Attributes:
        timestamp (float): 记录时间 (time.time())
        elapsed_ms (float): 耗时（毫秒），仅 bidi_cmd 类型有意义
        category (str): 分类 — ``"bidi_cmd"`` / ``"bidi_event"``
            / ``"net_intercept"`` / ``"error"``
        event (str): 具体事件名，如 ``"browsingContext.navigate"``
        data (dict): 自由字段，如 url, locator, error
        context_id (str | None): 关联的 browsingContext ID
        status (str): ``"ok"`` / ``"error"`` / ``"timeout"`` / ``"warn"``
    """
    __slots__ = ('timestamp', 'elapsed_ms', 'category', 'event',
                 'data', 'context_id', 'status')

    def __init__(self, category, event, data=None, context_id=None,
                 elapsed_ms=0, status='ok'):
        self.timestamp = time.time()
        self.elapsed_ms = round(elapsed_ms, 2)
        self.category = category
        self.event = event
        self.data = data or {}
        self.context_id = context_id
        self.status = status

    def to_dict(self):
        """序列化为 dict"""
        return {
            'ts': self.timestamp,
            'ms': self.elapsed_ms,
            'cat': self.category,
            'evt': self.event,
            'ctx': self.context_id,
            'st': self.status,
            'data': self.data,
        }

    def __repr__(self):
        return '<Trace {:.0f}ms {} {} {}>'.format(
            self.elapsed_ms, self.category, self.event, self.status)


class FailureSnapshot:
    """自动化失败时的诊断快照。

    由框架在操作失败时自动收集，附加到异常的 ``.diagnostics`` 属性上。

    Attributes:
        timestamp (float): 快照采集时间
        error_type (str): 异常类名
        error_message (str): 异常消息（截断到 500 字符）
        url (str | None): 当前页面 URL
        screenshot_path (str | None): 截图文件保存路径
        dom_path (str | None): DOM HTML 文件保存路径
        recent_requests (list): 最近网络请求摘要列表
        trace_entries (list): 最近 trace 记录列表
        context_id (str | None): 当前 browsingContext ID
        saved_dir (str | None): 快照文件保存目录
        capture_errors (list): 各收集步骤的失败信息
    """

    def __init__(self):
        self.timestamp = time.time()
        self.error_type = ''
        self.error_message = ''
        self.url = None
        self.screenshot_path = None
        self.dom_path = None
        self.recent_requests = []
        self.trace_entries = []
        self.context_id = None
        self.saved_dir = None
        self.capture_errors = []

    def to_dict(self):
        """序列化为 dict（不含 bytes 数据）。"""
        return {
            'timestamp': self.timestamp,
            'error_type': self.error_type,
            'error_message': self.error_message,
            'url': self.url,
            'screenshot_path': self.screenshot_path,
            'dom_path': self.dom_path,
            'recent_requests': self.recent_requests,
            'trace_entries': [
                e.to_dict() if hasattr(e, 'to_dict') else e
                for e in self.trace_entries
            ],
            'context_id': self.context_id,
            'capture_errors': self.capture_errors,
        }

    def to_json(self):
        """序列化为 JSON 字符串。"""
        return json.dumps(self.to_dict(), ensure_ascii=False, indent=2,
                          default=str)

    def summary(self):
        """生成人类可读的诊断摘要。"""
        lines = [
            '=== Failure Snapshot ===',
            'Error: {} — {}'.format(self.error_type, self.error_message),
            'URL: {}'.format(self.url or '<unavailable>'),
            'Context: {}'.format(self.context_id or '<unknown>'),
            'Screenshot: {}'.format(self.screenshot_path or '<not captured>'),
            'DOM: {}'.format(self.dom_path or '<not captured>'),
            'Recent requests: {} entries'.format(len(self.recent_requests)),
            'Trace: {} entries'.format(len(self.trace_entries)),
        ]
        if self.capture_errors:
            lines.append('Capture warnings: {}'.format(
                '; '.join(self.capture_errors)))
        if self.saved_dir:
            lines.append('Saved to: {}'.format(self.saved_dir))
        return '\n'.join(lines)

    def __repr__(self):
        return '<FailureSnapshot {} {}>'.format(
            self.error_type, self.url or '?')


class Tracer:
    """Browser 级 trace 缓冲区（线程安全）。

    管理两个独立的环形缓冲区：
    - _buffer: 通用 trace 记录（BiDi 命令、事件、拦截、异常）
    - _net_buffer: 网络请求记录（来自 _recv_loop 被动钩子）

    所有方法线程安全，可从 recv 线程、event 线程和用户线程并发调用。
    """

    def __init__(self, max_entries=None):
        from .._functions.settings import Settings
        size = max_entries or Settings.trace_max_entries
        self._buffer = deque(maxlen=size)
        self._net_buffer = deque(maxlen=50)
        self._lock = threading.Lock()

    @property
    def enabled(self):
        """当前是否启用 trace 记录。"""
        from .._functions.settings import Settings
        return Settings.trace_enabled

    def record(self, category, event, data=None, context_id=None,
               elapsed_ms=0, status='ok'):
        """记录一条 trace 条目。

        Args:
            category: 分类 (bidi_cmd/bidi_event/net_intercept/error)
            event: 事件名
            data: 附加数据 dict
            context_id: browsingContext ID
            elapsed_ms: 耗时（毫秒）
            status: 状态 (ok/error/timeout/warn)
        """
        if not self.enabled:
            return
        entry = TraceEntry(category, event, data, context_id,
                           elapsed_ms, status)
        with self._lock:
            self._buffer.append(entry)

    def record_net(self, event_type, url, method, status_code,
                   context_id=None):
        """记录网络事件（来自 _recv_loop 被动钩子）。

        不使用 set_callback，与 page.listen / page.intercept 零冲突。

        Args:
            event_type: 事件类型 (responseCompleted/fetchError)
            url: 请求 URL
            method: HTTP 方法
            status_code: 响应状态码
            context_id: browsingContext ID
        """
        entry = {
            'timestamp': time.time(),
            'event_type': event_type,
            'url': url[:500] if url else '',
            'method': method or '',
            'status': status_code,
            'context_id': context_id,
        }
        with self._lock:
            self._net_buffer.append(entry)

    def latest(self, n=20):
        """获取最近 N 条 trace 记录。

        Args:
            n: 条数

        Returns:
            list[TraceEntry]
        """
        with self._lock:
            items = list(self._buffer)
        return items[-n:] if n < len(items) else items

    def recent_requests(self, n=20):
        """获取最近 N 条网络请求记录。

        Args:
            n: 条数

        Returns:
            list[dict]
        """
        with self._lock:
            items = list(self._net_buffer)
        return items[-n:] if n < len(items) else items

    def clear(self):
        """清空所有缓冲区。"""
        with self._lock:
            self._buffer.clear()
            self._net_buffer.clear()

    def dump_json(self):
        """导出全部 trace 为 JSON 字符串。"""
        with self._lock:
            entries = list(self._buffer)
        data = [e.to_dict() for e in entries]
        return json.dumps(data, ensure_ascii=False, indent=2, default=str)

    def summary(self, n=30):
        """生成人类可读的 trace 摘要。

        Args:
            n: 显示最近 N 条

        Returns:
            str: 格式化的摘要文本
        """
        entries = self.latest(n)
        if not entries:
            return '(no trace entries)'
        lines = ['=== Trace Summary (last {}) ==='.format(len(entries))]
        for e in entries:
            ts = time.strftime('%H:%M:%S', time.localtime(e.timestamp))
            lines.append('{} {:>7.1f}ms [{}] {} {}'.format(
                ts, e.elapsed_ms, e.category, e.event, e.status))
        return '\n'.join(lines)

    def __repr__(self):
        with self._lock:
            return '<Tracer entries={} net={}>'.format(
                len(self._buffer), len(self._net_buffer))
