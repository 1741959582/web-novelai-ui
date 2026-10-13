# -*- coding: utf-8 -*-
"""BiDi storage 模块命令"""


def _normalize_partition(partition):
    """兼容旧式 partition 简写，补全 BiDi 必需的 type 字段。"""
    if not partition:
        return None

    partition = dict(partition)
    partition_type = partition.get("type")
    if partition_type is None:
        if "context" in partition:
            partition_type = "context"
            partition["type"] = partition_type
        elif "userContext" in partition or "sourceOrigin" in partition:
            partition_type = "storageKey"
            partition["type"] = partition_type
        else:
            raise ValueError("partition must identify a context or storage key")

    if partition_type == "context":
        if not isinstance(partition.get("context"), str):
            raise ValueError("context partition requires a string context")
        extra_fields = set(partition) - {"type", "context"}
        if extra_fields:
            raise ValueError(
                "context partition does not accept fields: {}".format(
                    ", ".join(sorted(extra_fields))
                )
            )
    elif partition_type == "storageKey":
        for key in ("userContext", "sourceOrigin"):
            value = partition.get(key)
            if value is None:
                partition.pop(key, None)
            elif not isinstance(value, str):
                raise ValueError("storageKey {} must be a string".format(key))
    else:
        raise ValueError("partition type must be context or storageKey")

    return partition


def get_cookies(driver, filter_=None, partition=None):
    """获取 Cookie

    Args:
        filter_: Cookie 过滤条件 {'name': str, 'domain': str, ...}
        partition: 分区
            兼容以下两类写法：
            - 旧简写: {'context': str} 或 {'userContext': str, 'sourceOrigin': str}
            - 新协议: {'type': 'context', 'context': str}
                     {'type': 'storageKey', 'userContext': str, 'sourceOrigin': str}

    Returns:
        {'cookies': [CookieInfo...], 'partitionKey': dict}
    """
    params = {}
    if filter_:
        params["filter"] = filter_
    partition = _normalize_partition(partition)
    if partition:
        params["partition"] = partition
    return driver.run("storage.getCookies", params)


def set_cookie(driver, cookie, partition=None):
    """设置 Cookie

    Args:
        cookie: Cookie 字典
            {
                'name': str,
                'value': {'type': 'string', 'value': str},
                'domain': str,
                'path': str,  # 可选
                'httpOnly': bool,  # 可选
                'secure': bool,  # 可选
                'sameSite': 'strict'|'lax'|'none',  # 可选
                'expiry': int,  # 可选，Unix 时间戳
            }
        partition: 分区，支持旧简写并会自动补全 type 字段

    Returns:
        {'partitionKey': dict}
    """
    params = {"cookie": cookie}
    partition = _normalize_partition(partition)
    if partition:
        params["partition"] = partition
    return driver.run("storage.setCookie", params)


def delete_cookies(driver, filter_=None, partition=None):
    """删除 Cookie

    Args:
        filter_: Cookie 过滤条件
        partition: 分区，支持旧简写并会自动补全 type 字段

    Returns:
        {'partitionKey': dict}
    """
    params = {}
    if filter_:
        params["filter"] = filter_
    partition = _normalize_partition(partition)
    if partition:
        params["partition"] = partition
    return driver.run("storage.deleteCookies", params)
