# -*- coding: utf-8 -*-
"""BiDi permissions 模块命令（扩展模块）

Firefox 支持状态：
  ⚠️ setPermission  (扩展模块，Firefox可能不支持，提供降级方案)

注意：permissions不是W3C BiDi核心规范的一部分，是扩展模块。
Firefox可能不支持BiDi命令，但可以通过Firefox prefs实现相同效果。
"""

import logging
logger = logging.getLogger('ruyipage')


def _safe_run(driver, method, params, description='permissions command'):
    """执行BiDi permissions命令，不支持时优雅降级"""
    try:
        return driver.run(method, params)
    except Exception as e:
        err_str = str(e).lower()
        if ('unknown command' in err_str or 'not supported' in err_str
                or 'unknown method' in err_str or 'invalid method' in err_str):
            logger.warning('%s 不受当前Firefox版本支持: %s', description, e)
            return None
        raise


def set_permission(
    driver,
    descriptor,
    state,
    origin='https://example.com',
    contexts=None,
    *,
    embedded_origin=None,
    user_context=None,
):
    """设置浏览器权限（扩展模块，Firefox可能不支持）

    优先级：
    1. 尝试BiDi permissions.setPermission命令
    2. 降级到Firefox prefs设置

    Args:
        descriptor: 权限描述符，如 {'name': 'geolocation'}
        state: 'granted' / 'denied' / 'prompt'
        origin: 限定源（默认'https://example.com'）
        embedded_origin: 可选的嵌入源
        user_context: 可选的 user context ID
        contexts: 已废弃；permissions 扩展不接受 browsing contexts

    Returns:
        命令结果，或{'fallback': 'prefs'}（降级时），或None（无法设置）

    Example:
        >>> from ruyipage._bidi import permissions
        >>> # 授予地理位置权限
        >>> permissions.set_permission(driver, {'name': 'geolocation'}, 'granted')
        >>> # 拒绝通知权限
        >>> permissions.set_permission(driver, {'name': 'notifications'}, 'denied')
    """
    # 尝试BiDi命令
    if contexts is not None:
        raise ValueError("permissions.setPermission does not accept contexts")
    if state not in ('granted', 'denied', 'prompt'):
        raise ValueError("state must be granted, denied, or prompt")
    params = {'descriptor': descriptor, 'state': state, 'origin': origin}
    if embedded_origin is not None:
        params['embeddedOrigin'] = embedded_origin
    if user_context is not None:
        params['userContext'] = user_context

    result = _safe_run(driver, 'permissions.setPermission', params,
                       'permissions.setPermission')

    if result is not None:
        return result

    # 降级方案：通过Firefox prefs设置
    pref_map = {
        'geolocation': 'permissions.default.geo',
        'notifications': 'permissions.default.desktop-notification',
        'camera': 'permissions.default.camera',
        'microphone': 'permissions.default.microphone',
    }

    perm_name = descriptor.get('name', '')
    pref_name = pref_map.get(perm_name)

    if pref_name:
        # 状态映射：granted=1, denied=2, prompt=0
        state_value = {'granted': 1, 'denied': 2, 'prompt': 0}.get(state, 0)

        logger.info('通过Firefox prefs设置权限: %s = %s', pref_name, state_value)
        return {'fallback': 'prefs', 'pref': pref_name, 'value': state_value,
                'note': '需要通过PrefsManager或user.js手动设置'}

    logger.warning('无法设置权限: %s (不支持的权限类型)', perm_name)
    return None
