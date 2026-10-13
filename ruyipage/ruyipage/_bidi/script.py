# -*- coding: utf-8 -*-
"""BiDi script 模块命令"""

from .._functions.bidi_values import serialize_value


_LOCAL_VALUE_TYPES = {
    "array",
    "bigint",
    "boolean",
    "channel",
    "date",
    "map",
    "null",
    "number",
    "object",
    "regexp",
    "set",
    "string",
    "undefined",
}


def _is_serialized_local_value(value):
    if not isinstance(value, dict):
        return False
    if "sharedId" in value or "handle" in value:
        return True
    return value.get("type") in _LOCAL_VALUE_TYPES


def _normalize_local_value(value):
    return value if _is_serialized_local_value(value) else serialize_value(value)


def _normalize_channel_value(value):
    if not isinstance(value, dict) or value.get("type") != "channel":
        raise ValueError("preload script arguments must be BiDi ChannelValue objects")
    channel = value.get("value")
    if not isinstance(channel, dict) or not isinstance(channel.get("channel"), str):
        raise ValueError("ChannelValue requires value.channel")
    return value


def evaluate(driver, context, expression, await_promise=True,
             result_ownership='root', serialization_options=None,
             user_activation=False, sandbox=None, timeout=None):
    """执行 JavaScript 表达式

    Args:
        context: browsingContext ID
        expression: JS 表达式字符串
        await_promise: 是否等待 Promise resolve
        result_ownership: 'root' 或 'none'
        serialization_options: 序列化选项
        user_activation: 是否模拟用户激活
        sandbox: 沙箱名称
        timeout: 超时时间（秒），None 使用默认值

    Returns:
        包含必需 ``realm`` 字段的成功结果或异常结果。
    """
    target = {'context': context}
    if sandbox:
        target['sandbox'] = sandbox

    params = {
        'expression': expression,
        'target': target,
        'awaitPromise': await_promise,
        'resultOwnership': result_ownership,
    }
    if serialization_options:
        params['serializationOptions'] = serialization_options
    if user_activation:
        params['userActivation'] = True

    return driver.run('script.evaluate', params, timeout=timeout)


def call_function(driver, context, function_declaration, arguments=None,
                  this=None, await_promise=True, result_ownership='root',
                  serialization_options=None, user_activation=False, sandbox=None,
                  timeout=None):
    """调用 JavaScript 函数

    Args:
        context: browsingContext ID
        function_declaration: 函数声明字符串，如 '(a, b) => a + b'
        arguments: 参数列表，每项为 LocalValue 或 SharedReference
        this: this 绑定的对象
        await_promise: 是否等待 Promise resolve
        result_ownership: 'root' 或 'none'
        serialization_options: 序列化选项
        user_activation: 是否模拟用户激活
        sandbox: 沙箱名称

    Returns:
        包含必需 ``realm`` 字段的成功结果或异常结果。
    """
    target = {'context': context}
    if sandbox:
        target['sandbox'] = sandbox

    params = {
        'functionDeclaration': function_declaration,
        'target': target,
        'awaitPromise': await_promise,
        'resultOwnership': result_ownership,
    }

    if arguments is not None:
        params['arguments'] = [_normalize_local_value(arg) for arg in arguments]

    if this is not None:
        params['this'] = _normalize_local_value(this)

    if serialization_options:
        params['serializationOptions'] = serialization_options
    if user_activation:
        params['userActivation'] = True

    return driver.run('script.callFunction', params, timeout=timeout)


def add_preload_script(driver, function_declaration, arguments=None,
                       contexts=None, sandbox=None, timeout=None, user_contexts=None):
    """注册预加载脚本（每次导航前执行）

    Args:
        function_declaration: 函数声明字符串
        arguments: 参数列表
        contexts: 限定的 context 列表
        sandbox: 沙箱名称

    Returns:
        {'script': str}  预加载脚本 ID
    """
    params = {'functionDeclaration': function_declaration}
    if arguments is not None:
        params['arguments'] = [_normalize_channel_value(a) for a in arguments]
    if contexts and user_contexts:
        raise ValueError('contexts and user_contexts cannot both be provided')
    if contexts:
        params['contexts'] = contexts if isinstance(contexts, list) else [contexts]
    if user_contexts:
        params['userContexts'] = user_contexts if isinstance(user_contexts, list) else [user_contexts]
    if sandbox:
        params['sandbox'] = sandbox
    return driver.run('script.addPreloadScript', params, timeout=timeout)


def remove_preload_script(driver, script_id):
    """移除预加载脚本

    Args:
        script_id: 预加载脚本 ID
    """
    return driver.run('script.removePreloadScript', {'script': script_id})


def get_realms(driver, context=None, type_=None):
    """获取所有 Realm（执行上下文）

    Args:
        context: 可选，限定 context
        type_: 可选，限定类型 ('window', 'dedicated-worker', 等)

    Returns:
        {'realms': [RealmInfo...]}
    """
    params = {}
    if context:
        params['context'] = context
    if type_:
        params['type'] = type_
    return driver.run('script.getRealms', params)


def disown(driver, handles, target):
    """释放远程对象句柄

    Args:
        handles: 句柄列表
        target: 目标 {'context': str} 或 {'realm': str}
    """
    return driver.run('script.disown', {
        'handles': handles,
        'target': target
    })
