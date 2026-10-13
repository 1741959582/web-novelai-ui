# -*- coding: utf-8 -*-
"""Storage 管理单元（localStorage/sessionStorage）"""

from .._bidi import script as bidi_script


class StorageManager:
    """Storage 管理器基类"""

    def __init__(self, page, storage_type):
        """
        Args:
            page: FirefoxPage 实例
            storage_type: 'localStorage' 或 'sessionStorage'
        """
        self._page = page
        self._storage_type = storage_type

    def set(self, key, value):
        """设置存储项"""
        js = f'(key, value) => {self._storage_type}.setItem(key, value)'
        self._page.run_js(js, key, str(value))

    def get(self, key):
        """获取存储项"""
        js = f'(key) => {self._storage_type}.getItem(key)'
        return self._page.run_js(js, key)

    def remove(self, key):
        """删除存储项"""
        js = f'(key) => {self._storage_type}.removeItem(key)'
        self._page.run_js(js, key)

    def clear(self):
        """清空所有存储项"""
        js = f'{self._storage_type}.clear()'
        self._page.run_js(js)

    def keys(self):
        """获取所有键"""
        js = f'Object.keys({self._storage_type})'
        return self._page.run_js(js) or []

    def items(self):
        """获取所有键值对"""
        js = f'''
        (() => {{
            const items = {{}};
            for (let i = 0; i < {self._storage_type}.length; i++) {{
                const key = {self._storage_type}.key(i);
                items[key] = {self._storage_type}.getItem(key);
            }}
            return items;
        }})()
        '''
        return self._page.run_js(js) or {}

    def __len__(self):
        """获取存储项数量"""
        js = f'{self._storage_type}.length'
        return self._page.run_js(js) or 0

    def __contains__(self, key):
        """检查键是否存在"""
        return self.get(key) is not None

    def __getitem__(self, key):
        """字典式访问"""
        value = self.get(key)
        if value is None:
            raise KeyError(key)
        return value

    def __setitem__(self, key, value):
        """字典式设置"""
        self.set(key, value)

    def __delitem__(self, key):
        """字典式删除"""
        self.remove(key)
