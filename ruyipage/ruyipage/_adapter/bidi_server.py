# -*- coding: utf-8 -*-
"""BiDi Server 内部通信封装

Firefox BiDi Server 架构：
  Firefox Remote Agent
    └── BiDi WebSocket Handler (ws://host:port/session)
          ├── 命令路由 → 各模块 Handler
          └── 事件广播 → 所有订阅者

本模块封装 BiDi Server 的连接建立、会话初始化、
事件订阅管理，作为 BrowserBiDiDriver 的上层门面。
"""

import logging
import secrets
import socket

logger = logging.getLogger('ruyipage')


def _find_random_free_port(opts):
    start, end = opts.random_port_range
    candidates = list(range(start, end + 1))
    secrets.SystemRandom().shuffle(candidates)

    for port in candidates:
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
                sock.bind(("127.0.0.1", port))
                return port
        except OSError:
            continue

    raise RuntimeError("在端口范围 {}-{} 中找不到可用端口".format(start, end))


class BiDiServer:
    """BiDi Server 连接门面

    封装从 Firefox 启动到 BiDi 会话就绪的完整流程：
    1. 等待 Remote Agent 就绪
    2. 获取 WebSocket URL
    3. 建立 WebSocket 连接
    4. 初始化 BiDi session
    5. 同步 context 树
    """

    def __init__(self, options):
        """
        Args:
            options: FirefoxOptions 实例
        """
        self._options = options
        self._driver = None      # BrowserBiDiDriver
        self._process = None     # Firefox 进程
        self._ctx_registry = None
        self._ctx_adapter = None

    @property
    def driver(self):
        return self._driver

    @property
    def process(self):
        return self._process

    def connect(self, launch=True):
        """连接到 Firefox BiDi Server

        Args:
            launch: True=自动启动 Firefox，False=仅连接已有实例

        Returns:
            BrowserBiDiDriver 实例
        """
        from .remote_agent import (find_free_port, wait_for_firefox,
                                   get_bidi_ws_url, launch_firefox)
        from .context_manager import ContextRegistry, ContextEventAdapter
        from .._base.driver import BrowserBiDiDriver

        opts = self._options
        host = opts.host
        port = opts.port

        # 自动端口
        if opts.random_port:
            port = _find_random_free_port(opts)
            opts._set_port_for_launch(port)
        elif opts.auto_port:
            port = find_free_port(port)
            opts._set_port_for_launch(port)

        # 写入 profile prefs
        if opts.profile_path:
            opts.write_prefs_to_profile()

        # 启动 Firefox（如需要）
        if launch and not opts.is_existing_only:
            cmd = opts.build_command()
            self._process = launch_firefox(cmd)

        # 等待就绪
        if not wait_for_firefox(host, port, timeout=30):
            raise RuntimeError(
                'Firefox Remote Agent 未就绪 ({}:{})'.format(host, port))

        # 获取 WS URL 并建立连接
        ws_url = get_bidi_ws_url(host, port, timeout=10)
        address = '{}:{}'.format(host, port)

        self._driver = BrowserBiDiDriver(address)
        self._driver.start(ws_url)

        # 初始化 context 注册表
        self._ctx_registry = ContextRegistry()
        self._ctx_adapter = ContextEventAdapter(self._driver, self._ctx_registry)
        self._ctx_adapter.start()

        # 同步初始 context 树
        self._sync_contexts()

        logger.info('BiDi 连接就绪: %s', ws_url)
        return self._driver

    def _sync_contexts(self):
        """从 browsingContext.getTree 同步 context 注册表"""
        try:
            result = self._driver.run('browsingContext.getTree', {})
            self._ctx_registry.sync_from_tree(result.get('contexts', []))
        except Exception as e:
            logger.debug('同步 context 树失败: %s', e)

    def get_top_context(self):
        """获取第一个顶层 context ID"""
        ids = self._ctx_registry.all_ids()
        # 找没有 parent 的 context
        for cid in ids:
            info = self._ctx_registry.get(cid)
            if info and info.get('parent') is None:
                return cid
        return ids[0] if ids else None

    def disconnect(self):
        """断开连接并可选关闭 Firefox"""
        if self._ctx_adapter:
            try:
                self._ctx_adapter.stop()
            except Exception:
                pass
        if self._driver:
            try:
                self._driver.stop()
            except Exception:
                pass
        if self._process:
            try:
                self._process.terminate()
            except Exception:
                pass
