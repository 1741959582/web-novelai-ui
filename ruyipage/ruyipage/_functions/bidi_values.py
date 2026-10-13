# -*- coding: utf-8 -*-
"""BiDi 协议值序列化/反序列化工具"""


class _JsFailure(object):
    """表示 JS 调用本身失败，区别于 JS 合法返回 null/undefined。

    求值为假，因此历史上的 ``_run_safe(...) or default`` 写法行为不变；
    需要区分“失败”和“返回空”的调用方显式用 ``is JS_FAILED`` 判断。
    """

    __slots__ = ()

    def __bool__(self):
        return False

    def __repr__(self):
        return "<JS_FAILED>"


JS_FAILED = _JsFailure()


def parse_value(node):
    """将 BiDi RemoteValue 转换为 Python 原生对象

    Args:
        node: BiDi 返回的序列化值字典

    Returns:
        Python 原生对象
    """
    if not isinstance(node, dict):
        return node

    t = node.get('type', '')

    if t in ('null', 'undefined'):
        return None

    if t == 'string':
        return node.get('value', '')

    if t == 'number':
        val = node.get('value')
        if isinstance(val, str):
            if val == 'NaN':
                return float('nan')
            elif val == 'Infinity':
                return float('inf')
            elif val == '-Infinity':
                return float('-inf')
            elif val == '-0':
                return -0.0
        return val

    if t == 'boolean':
        return node.get('value', False)

    if t == 'bigint':
        return int(node.get('value', '0'))

    if t == 'array':
        return [parse_value(item) for item in node.get('value', [])]

    if t == 'object':
        obj = {}
        for pair in node.get('value', []):
            if isinstance(pair, list) and len(pair) == 2:
                k = pair[0] if isinstance(pair[0], str) else parse_value(pair[0])
                obj[k] = parse_value(pair[1])
        return obj

    if t == 'map':
        result = {}
        for pair in node.get('value', []):
            if isinstance(pair, list) and len(pair) == 2:
                k = parse_value(pair[0])
                result[k] = parse_value(pair[1])
        return result

    if t == 'set':
        return set(parse_value(item) for item in node.get('value', []))

    if t == 'date':
        return node.get('value', '')

    if t == 'regexp':
        return node.get('value', {})

    if t == 'node':
        # DOM 节点，返回原始字典以便后续创建 FirefoxElement
        return node

    if t == 'window':
        return node

    if t == 'error':
        return node

    # 未知类型，返回 value 或原字典
    return node.get('value', node)


def serialize_value(value):
    """将 Python 对象转换为 BiDi LocalValue

    Args:
        value: Python 原生对象

    Returns:
        BiDi 协议格式的字典
    """
    if value is None:
        return {'type': 'null'}

    if isinstance(value, bool):
        return {'type': 'boolean', 'value': value}

    if isinstance(value, int):
        # 检查是否超出安全整数范围
        if abs(value) > 9007199254740991:
            return {'type': 'bigint', 'value': str(value)}
        return {'type': 'number', 'value': value}

    if isinstance(value, float):
        import math
        if math.isnan(value):
            return {'type': 'number', 'value': 'NaN'}
        if math.isinf(value):
            return {'type': 'number', 'value': 'Infinity' if value > 0 else '-Infinity'}
        if value == 0.0 and math.copysign(1.0, value) < 0:
            return {'type': 'number', 'value': '-0'}
        return {'type': 'number', 'value': value}

    if isinstance(value, str):
        return {'type': 'string', 'value': value}

    if isinstance(value, (list, tuple)):
        return {'type': 'array', 'value': [serialize_value(v) for v in value]}

    if isinstance(value, dict):
        # 检查是否是 SharedReference（FirefoxElement 传入）
        if 'sharedId' in value:
            return {'type': 'sharedReference', 'sharedId': value['sharedId']}
        pairs = []
        for k, v in value.items():
            pairs.append([serialize_value(k) if not isinstance(k, str) else k,
                          serialize_value(v)])
        return {'type': 'object', 'value': pairs}

    if isinstance(value, set):
        return {'type': 'set', 'value': [serialize_value(v) for v in value]}

    # 对于带有 _shared_id 属性的对象（FirefoxElement）
    shared_id = getattr(value, '_shared_id', None)
    if shared_id:
        return {'type': 'sharedReference', 'sharedId': shared_id}

    # 其他类型尝试转为字符串
    return {'type': 'string', 'value': str(value)}


def make_shared_ref(shared_id, handle=None):
    """创建 BiDi SharedReference

    Args:
        shared_id: 元素的 sharedId
        handle: 可选的 handle

    Returns:
        SharedReference 字典
    """
    ref = {'type': 'sharedReference', 'sharedId': shared_id}
    if handle:
        ref['handle'] = handle
    return ref
