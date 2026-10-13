# -*- coding: utf-8 -*-
"""BiDi log 模块事件处理"""


# log.entryAdded 事件处理
# 此模块仅定义事件数据类型，实际的回调注册在 Listener 等单元中


class LogEntry(object):
    """日志条目"""

    def __init__(self, level='', text='', timestamp=0, source=None,
                 log_type='', method='', args=None, stack_trace=None):
        self.level = level  # 'debug', 'info', 'warn', 'error'
        self.text = text
        self.timestamp = timestamp
        self.source = source or {}
        self.log_type = log_type  # 'console' or 'javascript'
        self.method = method  # console method name
        self.args = args or []
        self.stack_trace = stack_trace

    @classmethod
    def from_params(cls, params):
        """从事件参数创建"""
        return cls(
            level=params.get('level', ''),
            text=params.get('text', ''),
            timestamp=params.get('timestamp', 0),
            source=params.get('source', {}),
            log_type=params.get('type', ''),
            method=params.get('method', ''),
            args=params.get('args', []),
            stack_trace=params.get('stackTrace'),
        )

    def __repr__(self):
        return '<LogEntry [{}] {}>'.format(self.level, (self.text or '')[:60])
