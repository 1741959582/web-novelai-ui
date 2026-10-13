# -*- coding: utf-8 -*-
"""Cookie 格式转换工具"""


def cookies_to_dict(cookies):
    """将 Cookie 列表转为字典

    Args:
        cookies: [{'name': 'a', 'value': '1'}, ...]

    Returns:
        {'a': '1', ...}
    """
    return {c.get('name', ''): c.get('value', '') for c in cookies if c.get('name')}


def dict_to_cookies(cookie_dict, domain=''):
    """将字典转为 Cookie 列表

    Args:
        cookie_dict: {'name': 'value', ...}
        domain: 所属域名

    Returns:
        [{'name': 'name', 'value': 'value', 'domain': domain}, ...]
    """
    return [{'name': k, 'value': str(v), 'domain': domain}
            for k, v in cookie_dict.items()]


def cookie_str_to_list(cookie_str):
    """将 Cookie 字符串解析为列表

    Args:
        cookie_str: 'name1=value1; name2=value2'

    Returns:
        [{'name': 'name1', 'value': 'value1'}, ...]
    """
    cookies = []
    for pair in cookie_str.split(';'):
        pair = pair.strip()
        if '=' in pair:
            name, value = pair.split('=', 1)
            cookies.append({'name': name.strip(), 'value': value.strip()})
    return cookies
