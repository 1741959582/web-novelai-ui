# -*- coding: utf-8 -*-
"""Firefox Remote Agent 连接管理

Firefox Remote Agent 通过 --remote-debugging-port 暴露两个端点：
  - HTTP  http://host:port/json          → 获取 WebSocket URL
  - WS    ws://host:port/session         → BiDi over WebSocket

本模块负责：
1. 探测 Firefox 是否已就绪（HTTP /json 轮询）
2. 获取 BiDi WebSocket URL
3. 启动 Firefox 进程（如需要）
4. 自动端口分配
"""

import json
import subprocess
import time
import logging
from urllib.parse import urlsplit

logger = logging.getLogger("ruyipage")


def _probe_ws_url(ws_url, timeout=0.5):
    """尝试建立一次 WebSocket 握手，确认给定 BiDi URL 可用。"""
    import websocket

    ws = None
    try:
        ws = websocket.create_connection(
            ws_url, timeout=timeout, suppress_origin=True, enable_multithread=True
        )
        return True
    except Exception:
        return False
    finally:
        if ws:
            try:
                ws.close()
            except Exception:
                pass


from .._functions.tools import find_free_port, is_port_open  # noqa: F401


def _is_bidi_ws_url(ws_url):
    if not ws_url:
        return False
    try:
        path = urlsplit(ws_url).path.lower()
    except Exception:
        return False
    return "/devtools/" not in path


def _remaining_timeout(deadline, cap):
    remaining = deadline - time.time()
    if remaining <= 0:
        return 0
    return max(0.01, min(cap, remaining))


def _read_json_ws_url(url, request_timeout):
    import urllib.request
    import urllib.error

    try:
        with urllib.request.urlopen(url, timeout=request_timeout) as resp:
            data = json.loads(resp.read().decode())
    except (urllib.error.URLError, OSError, json.JSONDecodeError):
        return ""

    if isinstance(data, dict):
        ws = data.get("webSocketDebuggerUrl", "")
        return ws if _is_bidi_ws_url(ws) else ""

    if isinstance(data, list):
        for item in data:
            if not isinstance(item, dict):
                continue
            ws = item.get("webSocketDebuggerUrl", "")
            if _is_bidi_ws_url(ws):
                return ws
    return ""


def get_bidi_ws_url(host, port, timeout=30):
    """从 Firefox Remote Agent HTTP 端点获取 BiDi WebSocket URL

    Firefox 146+ 在 /json 返回：
      {"webSocketDebuggerUrl": "ws://host:port/session", ...}

    Args:
        host: 主机地址
        port: 远程调试端口
        timeout: 等待超时（秒）

    Returns:
        str: WebSocket URL，如 'ws://127.0.0.1:9222/session'
    """
    direct_ws = "ws://{}:{}".format(host, port)
    session_ws = "ws://{}:{}/session".format(host, port)

    deadline = time.time() + timeout
    url = "http://{}:{}/json".format(host, port)
    request_cap = 0.5 if host in ("127.0.0.1", "localhost", "::1") else 1.0
    root_probe_done = False

    while time.time() < deadline:
        request_timeout = _remaining_timeout(deadline, request_cap)
        if request_timeout <= 0:
            break
        ws = _read_json_ws_url(url, request_timeout)
        if ws:
            return ws

        # AdsPower / FlowerBrowser may expose BiDi at ws://host:port.
        if not root_probe_done:
            root_probe_done = True
            if _probe_ws_url(direct_ws, timeout=_remaining_timeout(deadline, 0.3)):
                return direct_ws
            if _probe_ws_url(session_ws, timeout=_remaining_timeout(deadline, 0.3)):
                return session_ws

        time.sleep(min(0.1, max(0, deadline - time.time())))

    if _probe_ws_url(session_ws, timeout=_remaining_timeout(deadline, 0.3)):
        return session_ws

    if not root_probe_done and _probe_ws_url(
        direct_ws, timeout=_remaining_timeout(deadline, 0.3)
    ):
        return direct_ws

    return direct_ws


def wait_for_firefox(host, port, timeout=30):
    """等待 Firefox Remote Agent 就绪

    Args:
        host: 主机
        port: 端口
        timeout: 超时（秒）

    Returns:
        bool: 是否就绪
    """
    deadline = time.time() + timeout
    while time.time() < deadline:
        if is_port_open(host, port, timeout=1.0):
            return True
        time.sleep(0.3)
    return False


def launch_firefox(cmd, env=None):
    """启动 Firefox 进程

    Args:
        cmd: 命令行列表
        env: 环境变量字典（None 继承当前环境）

    Returns:
        subprocess.Popen 实例
    """
    import os

    kwargs = {
        "stdout": subprocess.DEVNULL,
        "stderr": subprocess.DEVNULL,
    }
    if env:
        kwargs["env"] = env

    # Windows: 隐藏控制台窗口 + 脱离 Job Object（避免调试器停止时连带杀死浏览器）
    if hasattr(subprocess, "CREATE_NO_WINDOW"):
        CREATE_BREAKAWAY_FROM_JOB = 0x01000000
        kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB
    else:
        # Unix/macOS: 脱离进程组
        kwargs["start_new_session"] = True

    logger.debug("启动 Firefox: %s", " ".join(cmd))
    try:
        return subprocess.Popen(cmd, **kwargs)
    except OSError:
        # 某些受限环境不允许 BREAKAWAY_FROM_JOB，回退
        if "creationflags" in kwargs:
            kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW
            return subprocess.Popen(cmd, **kwargs)
        raise
