# -*- coding: utf-8 -*-
"""URL 验证和 Web 工具"""

import re


def is_valid_url(url):
    """检查是否是合法 URL

    Args:
        url: URL 字符串

    Returns:
        bool
    """
    pattern = re.compile(
        r'^https?://'
        r'(?:(?:[A-Z0-9](?:[A-Z0-9-]{0,61}[A-Z0-9])?\.)+[A-Z]{2,6}\.?|'
        r'localhost|'
        r'\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})'
        r'(?::\d+)?'
        r'(?:/?|[/?]\S+)$',
        re.IGNORECASE
    )
    return bool(pattern.match(url))


def ensure_url(url):
    """确保 URL 有协议前缀

    Args:
        url: URL 字符串

    Returns:
        带协议的 URL
    """
    if not url:
        return url
    if not re.match(r'^https?://', url, re.IGNORECASE):
        if url.startswith('//'):
            return 'https:' + url
        return 'https://' + url
    return url
