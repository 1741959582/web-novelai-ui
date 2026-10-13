# -*- coding: utf-8 -*-
"""e10s/Fission 多进程 context 管理

Firefox e10s（多进程）/ Fission（站点隔离）架构下，
每个 browsingContext 运行在独立的内容进程中。

BiDi 协议通过 context ID 透明处理跨进程通信，
本模块负责：
1. context 树的维护（parent/child 关系）
2. 跨进程 context 查找
3. context 生命周期事件订阅
"""

import threading
import logging

logger = logging.getLogger('ruyipage')


class ContextRegistry:
    """browsingContext 注册表

    维护所有已知 context 的树形结构，
    支持按 URL、parent、类型快速查找。
    """

    def __init__(self):
        self._contexts = {}   # {context_id: ContextInfo}
        self._lock = threading.Lock()

    def register(self, context_id, url='', parent=None, children=None):
        with self._lock:
            self._contexts[context_id] = {
                'id': context_id,
                'url': url,
                'parent': parent,
                'children': list(children or []),
            }

    def unregister(self, context_id):
        with self._lock:
            self._contexts.pop(context_id, None)
            # 从父节点的 children 中移除
            for info in self._contexts.values():
                if context_id in info['children']:
                    info['children'].remove(context_id)

    def update_url(self, context_id, url):
        with self._lock:
            if context_id in self._contexts:
                self._contexts[context_id]['url'] = url

    def get(self, context_id):
        with self._lock:
            return self._contexts.get(context_id)

    def children(self, context_id):
        """获取直接子 context 列表"""
        with self._lock:
            info = self._contexts.get(context_id)
            return list(info['children']) if info else []

    def find_by_url(self, pattern):
        """按 URL 子串查找 context ID 列表"""
        with self._lock:
            return [cid for cid, info in self._contexts.items()
                    if pattern in info.get('url', '')]

    def all_ids(self):
        with self._lock:
            return list(self._contexts.keys())

    def sync_from_tree(self, tree_contexts):
        """从 browsingContext.getTree 结果同步注册表

        Args:
            tree_contexts: getTree 返回的 contexts 列表
        """
        def _walk(ctx, parent=None):
            cid = ctx.get('context', '')
            url = ctx.get('url', '')
            children = [c.get('context', '') for c in ctx.get('children', [])]
            self.register(cid, url=url, parent=parent, children=children)
            for child in ctx.get('children', []):
                _walk(child, parent=cid)

        with self._lock:
            self._contexts.clear()
        for ctx in tree_contexts:
            _walk(ctx)


class ContextEventAdapter:
    """context 生命周期事件适配器

    订阅 BiDi browsingContext 事件，自动维护 ContextRegistry。
    """

    def __init__(self, driver, registry):
        """
        Args:
            driver: BrowserBiDiDriver 实例
            registry: ContextRegistry 实例
        """
        self._driver = driver
        self._registry = registry
        self._subscribed = False

    def start(self):
        """订阅 context 生命周期事件"""
        if self._subscribed:
            return
        from .._bidi import session as bidi_session
        bidi_session.subscribe(self._driver, [
            'browsingContext.contextCreated',
            'browsingContext.contextDestroyed',
            'browsingContext.navigationStarted',
            'browsingContext.load',
        ])
        self._driver.set_callback(
            'browsingContext.contextCreated', self._on_created)
        self._driver.set_callback(
            'browsingContext.contextDestroyed', self._on_destroyed)
        self._driver.set_callback(
            'browsingContext.navigationStarted', self._on_nav)
        self._subscribed = True

    def stop(self):
        if not self._subscribed:
            return
        for evt in ('browsingContext.contextCreated',
                    'browsingContext.contextDestroyed',
                    'browsingContext.navigationStarted'):
            self._driver.remove_callback(evt)
        self._subscribed = False

    def _on_created(self, params):
        cid = params.get('context', '')
        url = params.get('url', '')
        parent = params.get('parent')
        self._registry.register(cid, url=url, parent=parent)
        if parent:
            info = self._registry.get(parent)
            if info and cid not in info['children']:
                with self._registry._lock:
                    info['children'].append(cid)
        logger.debug('context 创建: %s parent=%s', cid[:16], parent)

    def _on_destroyed(self, params):
        cid = params.get('context', '')
        self._registry.unregister(cid)
        logger.debug('context 销毁: %s', cid[:16])

    def _on_nav(self, params):
        cid = params.get('context', '')
        url = params.get('url', '')
        self._registry.update_url(cid, url)
