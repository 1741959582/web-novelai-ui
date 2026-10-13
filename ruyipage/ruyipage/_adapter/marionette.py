# -*- coding: utf-8 -*-
"""Marionette 特权通道封装

Marionette 是 Firefox 内置的特权 JS 执行通道（端口 2828），
可访问 Services.prefs、ChromeUtils 等内容沙箱无法访问的 API。

重要限制：
- newSession 命令会断开 BiDi WebSocket（两者共用 Remote Agent 内部状态）
- 因此本模块只做单次命令执行，不维持 Marionette session
- 实际 about:config 读写改用 user.js 文件操作（见 prefs.py）

本模块保留用于：
- 读取运行时 pref（只读，不需要 newSession）
- 执行特权 JS 片段（需要 newSession，谨慎使用）
"""

import json
import socket
import logging

logger = logging.getLogger('ruyipage')

MARIONETTE_PORT = 2828
MARIONETTE_HOST = '127.0.0.1'


class MarionetteClient:
    """最小化 Marionette 客户端

    仅用于单次特权命令，不维持长连接 session。
    每次操作独立建立/关闭 TCP 连接。
    """

    def __init__(self, host=MARIONETTE_HOST, port=MARIONETTE_PORT):
        self.host = host
        self.port = port

    def _connect(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(5)
        s.connect((self.host, self.port))
        # 读取握手包（Marionette 连接后立即发送版本信息）
        self._recv(s)
        return s

    def _send(self, s, msg):
        data = json.dumps(msg)
        s.sendall('{}\n'.format(len(data)).encode() + data.encode())

    def _recv(self, s):
        # Marionette 消息格式: <length>:<json>
        buf = b''
        while b':' not in buf:
            chunk = s.recv(1)
            if not chunk:
                break
            buf += chunk
        length = int(buf.split(b':')[0])
        data = b''
        while len(data) < length:
            chunk = s.recv(length - len(data))
            if not chunk:
                break
            data += chunk
        return json.loads(data.decode())

    def is_available(self):
        """检测 Marionette 端口是否可用"""
        try:
            s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            s.settimeout(1)
            s.connect((self.host, self.port))
            s.close()
            return True
        except OSError:
            return False

    def get_pref(self, key):
        """读取单个 pref（不需要 newSession，直接通过 getPrefs 命令）

        注意：Firefox 146 的 Marionette getPrefs 命令在无 session 时
        可能返回错误，此时降级返回 None。

        Args:
            key: pref 名称，如 'dom.webdriver.enabled'

        Returns:
            pref 值或 None
        """
        if not self.is_available():
            return None
        try:
            s = self._connect()
            self._send(s, [0, 1, 'getPrefs', {'prefs': [key]}])
            resp = self._recv(s)
            s.close()
            # resp 格式: [1, id, error, result]
            if isinstance(resp, list) and len(resp) == 4:
                if resp[2] is None and isinstance(resp[3], dict):
                    return (resp[3].get('prefs') or {}).get(key)
        except Exception as e:
            logger.debug('Marionette getPrefs 失败: %s', e)
        return None
