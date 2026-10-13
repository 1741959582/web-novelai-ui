# -*- coding: utf-8 -*-
"""W3C WebDriver BiDi emulation 模块命令。

本模块跟随当前 Editor's Draft 提供完整的标准命令封装。Firefox 对新命令的
实现进度取决于具体版本，因此使用 ``_safe_run`` 的命令在浏览器尚未实现时
返回 ``None``。``inject_ua_override()`` 等函数只作为旧版 Firefox 的兼容回退。
"""

import logging

logger = logging.getLogger("ruyipage")
_UNSET = object()


def _scope(params, contexts=None, user_contexts=None, required=False):
    if contexts is not None and user_contexts is not None:
        raise ValueError("contexts and user_contexts cannot both be provided")
    if contexts is not None:
        values = contexts if isinstance(contexts, list) else [contexts]
        if not values:
            raise ValueError("contexts must not be empty")
        params["contexts"] = values
    if user_contexts is not None:
        values = user_contexts if isinstance(user_contexts, list) else [user_contexts]
        if not values:
            raise ValueError("user_contexts must not be empty")
        params["userContexts"] = values
    if required and contexts is None and user_contexts is None:
        raise ValueError("contexts or user_contexts is required")
    return params


def _is_unsupported_error(error):
    err_type = str(getattr(error, "error", "")).lower()
    err_text = str(error).lower()
    return (
        err_type == "unknown command"
        or "unknown command" in err_text
        or "not supported" in err_text
        or "unknown method" in err_text
        or "invalid method" in err_text
    )


def _safe_run(driver, method, params, description="emulation command"):
    """执行 BiDi emulation 命令，不支持时优雅降级。

    Args:
        driver: BiDi driver
        method: BiDi 方法名
        params: 参数字典
        description: 日志描述

    Returns:
        命令结果字典，不支持时返回 None
    """
    try:
        return driver.run(method, params)
    except Exception as e:
        if _is_unsupported_error(e):
            logger.warning("%s 不受当前 Firefox 版本支持: %s", description, e)
            return None
        raise


# ---------------------------------------------------------------------------
# Firefox 149+ Stable 支持的命令
# ---------------------------------------------------------------------------


def set_user_agent_override(driver, user_agent, platform=None, contexts=None, user_contexts=None):
    if platform is not None:
        raise ValueError("platform is not part of emulation.setUserAgentOverride")
    params = {"userAgent": user_agent}
    _scope(params, contexts, user_contexts)
    return _safe_run(driver, "emulation.setUserAgentOverride", params, "emulation.setUserAgentOverride")

def set_geolocation_override(
    driver, latitude=None, longitude=None, accuracy=None, contexts=None,
    user_contexts=None, error=None, altitude=None, altitude_accuracy=None,
    heading=None, speed=None
):
    coordinate_fields = {
        "accuracy": accuracy,
        "altitude": altitude,
        "altitude_accuracy": altitude_accuracy,
        "heading": heading,
        "speed": speed,
    }
    if error is not None:
        if (
            latitude is not None
            or longitude is not None
            or any(value is not None for value in coordinate_fields.values())
        ):
            raise ValueError("error cannot be combined with coordinates")
        if error != {"type": "positionUnavailable"}:
            raise ValueError('error must be {"type": "positionUnavailable"}')
        params = {"error": dict(error)}
    elif latitude is None and longitude is None:
        if any(value is not None for value in coordinate_fields.values()):
            raise ValueError(
                "coordinate fields require latitude and longitude"
            )
        params = {"coordinates": None}
    elif latitude is None or longitude is None:
        raise ValueError("latitude and longitude must be provided together")
    else:
        coordinates = {"latitude": latitude, "longitude": longitude}
        if accuracy is not None:
            coordinates["accuracy"] = accuracy
        if altitude is not None:
            coordinates["altitude"] = altitude
        if altitude_accuracy is not None:
            coordinates["altitudeAccuracy"] = altitude_accuracy
        if heading is not None:
            coordinates["heading"] = heading
        if speed is not None:
            coordinates["speed"] = speed
        params = {"coordinates": coordinates}
    _scope(params, contexts, user_contexts)
    return _safe_run(driver, "emulation.setGeolocationOverride", params, "emulation.setGeolocationOverride")

def set_timezone_override(driver, timezone_id, contexts=None, user_contexts=None):
    params = {"timezone": timezone_id}
    _scope(params, contexts, user_contexts, required=True)
    return _safe_run(driver, "emulation.setTimezoneOverride", params, "emulation.setTimezoneOverride")

def set_locale_override(driver, locales, contexts=None, user_contexts=None):
    locale = locales[0] if isinstance(locales, list) else locales
    params = {"locale": locale}
    _scope(params, contexts, user_contexts, required=True)
    return _safe_run(driver, "emulation.setLocaleOverride", params, "emulation.setLocaleOverride")

def set_screen_orientation_override(
    driver,
    orientation_type=None,
    angle=None,
    contexts=None,
    user_contexts=None,
    *,
    natural=None,
):
    if orientation_type is None:
        if angle is not None or natural is not None:
            raise ValueError("angle/natural require orientation_type")
        screen_orientation = None
    else:
        orientation_types = {
            "portrait-primary",
            "portrait-secondary",
            "landscape-primary",
            "landscape-secondary",
        }
        if orientation_type not in orientation_types:
            raise ValueError("invalid screen orientation type")
        if natural is None and angle is None:
            natural = "portrait" if "portrait" in orientation_type else "landscape"
        elif natural is None:
            angle_map = {
                "portrait": {
                    "portrait-primary": 0,
                    "landscape-primary": 90,
                    "portrait-secondary": 180,
                    "landscape-secondary": 270,
                },
                "landscape": {
                    "landscape-primary": 0,
                    "portrait-primary": 90,
                    "landscape-secondary": 180,
                    "portrait-secondary": 270,
                },
            }
            matches = [
                candidate
                for candidate, values in angle_map.items()
                if values.get(orientation_type) == angle
            ]
            if len(matches) != 1:
                raise ValueError("angle does not identify a natural orientation")
            natural = matches[0]
        elif natural not in ("portrait", "landscape"):
            raise ValueError("natural must be portrait or landscape")
        if angle is not None:
            expected_angles = {
                "portrait": {
                    "portrait-primary": 0,
                    "landscape-primary": 90,
                    "portrait-secondary": 180,
                    "landscape-secondary": 270,
                },
                "landscape": {
                    "landscape-primary": 0,
                    "portrait-primary": 90,
                    "landscape-secondary": 180,
                    "portrait-secondary": 270,
                },
            }
            if expected_angles[natural].get(orientation_type) != angle:
                raise ValueError("angle is inconsistent with natural and type")
        screen_orientation = {"type": orientation_type, "natural": natural}
    params = {"screenOrientation": screen_orientation}
    _scope(params, contexts, user_contexts, required=True)
    return _safe_run(driver, "emulation.setScreenOrientationOverride", params, "emulation.setScreenOrientationOverride")

def set_screen_settings_override(
    driver, width=None, height=None, device_pixel_ratio=None,
    contexts=None, user_contexts=None
):
    if device_pixel_ratio is not None:
        raise ValueError(
            "device_pixel_ratio is not part of emulation.setScreenSettingsOverride; "
            "use browsingContext.setViewport"
        )
    if (width is None) != (height is None):
        raise ValueError("width and height must be provided together")
    if width is None or height is None:
        screen_area = None
    else:
        screen_area = {"width": width, "height": height}
    params = {"screenArea": screen_area}
    _scope(params, contexts, user_contexts, required=True)
    return _safe_run(driver, "emulation.setScreenSettingsOverride", params, "emulation.setScreenSettingsOverride")

def inject_screen_settings_override(driver, context, width, height, device_pixel_ratio=None):
    """通过 preload script 回退覆盖 screen / DPR。

    用于不支持 ``emulation.setScreenSettingsOverride`` 的旧版 Firefox。
    """
    from . import script as bidi_script

    width_value = "null" if width is None else str(int(width))
    height_value = "null" if height is None else str(int(height))
    dpr_value = (
        "null" if device_pixel_ratio is None else str(float(device_pixel_ratio))
    )
    inject_js = """() => {
  const width = %s;
  const height = %s;
  const dpr = %s;
  function define(target, name, value) {
    if (value === null || value === undefined) return;
    try {
      Object.defineProperty(target, name, {
        get: () => value,
        configurable: true
      });
    } catch (e) {}
  }
  if (window.screen) {
    // Overrides screen.width / screen.height / screen.availWidth / screen.availHeight.
    define(screen, 'width', width);
    define(screen, 'height', height);
    define(screen, 'availWidth', width);
    define(screen, 'availHeight', height);
  }
  define(window, 'devicePixelRatio', dpr);
}""" % (width_value, height_value, dpr_value)

    result = bidi_script.add_preload_script(
        driver, inject_js, contexts=[context], timeout=3
    )
    script_id = result.get("script", "")

    try:
        bidi_script.call_function(driver, context, inject_js, timeout=3)
    except Exception as e:
        logger.debug("当前页面 screen 覆盖执行失败（preload 仍然生效）: %s", e)

    return script_id


# ---------------------------------------------------------------------------
# 较新的 W3C 命令（旧版 Firefox 使用安全降级）
# ---------------------------------------------------------------------------


def set_network_conditions(driver, offline=False, contexts=None, user_contexts=None):
    params = {"networkConditions": {"type": "offline"} if offline else None}
    _scope(params, contexts, user_contexts)
    return _safe_run(driver, "emulation.setNetworkConditions", params, "emulation.setNetworkConditions")

def set_touch_override(driver, max_touch_points=1, contexts=None, user_contexts=None):
    if max_touch_points is not None:
        if isinstance(max_touch_points, bool) or not isinstance(max_touch_points, int):
            raise TypeError("max_touch_points must be a positive integer or None")
        if max_touch_points < 1 or max_touch_points > 9007199254740991:
            raise ValueError(
                "max_touch_points must be in range 1..9007199254740991"
            )
    params = {"maxTouchPoints": max_touch_points}
    _scope(params, contexts, user_contexts)
    return _safe_run(driver, "emulation.setTouchOverride", params, "emulation.setTouchOverride")

def inject_ua_override(driver, context, user_agent):
    """通过 script.addPreloadScript 注入 UA 覆盖

    用于 Firefox < 145 版本。FF145+ 请直接使用 set_user_agent_override()。

    Args:
        driver: BiDi driver (browser-level)
        context: browsingContext ID
        user_agent: 目标 UA 字符串

    Returns:
        str: preload script ID
    """
    from . import script as bidi_script

    escaped_ua = user_agent.replace("\\", "\\\\").replace("'", "\\'")
    inject_js = (
        "() => {"
        "  Object.defineProperty(navigator, 'userAgent', "
        "{get: () => '" + escaped_ua + "'});"
        "}"
    )

    result = bidi_script.add_preload_script(driver, inject_js, contexts=[context])
    script_id = result.get("script", "")

    try:
        bidi_script.call_function(driver, context, inject_js)
    except Exception as e:
        logger.debug("当前页面 UA 覆盖执行失败（preload 仍然生效）: %s", e)

    return script_id


# ---------------------------------------------------------------------------
# Editor's Draft 命令（可能需要较新的 Firefox）
# ---------------------------------------------------------------------------


def set_media_features_override(driver, features, contexts=None, user_contexts=None):
    if features is not None and not isinstance(features, dict):
        raise TypeError("features must be a W3C MediaFeatures dictionary or None")
    params = {"features": features}
    _scope(params, contexts, user_contexts)
    return _safe_run(driver, "emulation.setMediaFeaturesOverride", params, "emulation.setMediaFeaturesOverride")

def set_viewport_meta_override(driver, viewport_meta, contexts=None, user_contexts=None):
    if viewport_meta is not True and viewport_meta is not None:
        raise ValueError("viewport_meta must be True or None")
    params = {"viewportMeta": viewport_meta}
    _scope(params, contexts, user_contexts)
    return _safe_run(driver, "emulation.setViewportMetaOverride", params, "emulation.setViewportMetaOverride")


def set_document_cookie_disabled(driver, disabled=True, contexts=None):
    """禁用/启用Cookie (Firefox可能不支持)

    Args:
        disabled: True禁用，False启用
        contexts: 限定context列表
    """
    params = {"disabled": disabled}
    if contexts:
        params["contexts"] = contexts if isinstance(contexts, list) else [contexts]
    return _safe_run(
        driver,
        "emulation.setDocumentCookieDisabled",
        params,
        "emulation.setDocumentCookieDisabled",
    )


def set_bypass_csp(driver, enabled=True, contexts=None):
    """绕过内容安全策略 (Firefox可能不支持)

    Args:
        enabled: True启用绕过，False禁用
        contexts: 限定context列表
    """
    params = {"enabled": enabled}
    if contexts:
        params["contexts"] = contexts if isinstance(contexts, list) else [contexts]
    return _safe_run(driver, "emulation.setBypassCSP", params, "emulation.setBypassCSP")


def set_focus_emulation(driver, enabled=True, contexts=None):
    """模拟焦点状态 (Firefox可能不支持)

    Args:
        enabled: True启用焦点模拟
        contexts: 限定context列表
    """
    params = {"enabled": enabled}
    if contexts:
        params["contexts"] = contexts if isinstance(contexts, list) else [contexts]
    return _safe_run(
        driver, "emulation.setFocusEmulation", params, "emulation.setFocusEmulation"
    )


def set_hardware_concurrency(driver, concurrency, contexts=None):
    """覆盖navigator.hardwareConcurrency (Firefox可能不支持)

    Args:
        concurrency: CPU核心数
        contexts: 限定context列表
    """
    params = {"hardwareConcurrency": concurrency}
    if contexts:
        params["contexts"] = contexts if isinstance(contexts, list) else [contexts]
    return _safe_run(
        driver,
        "emulation.setHardwareConcurrency",
        params,
        "emulation.setHardwareConcurrency",
    )


def set_scripting_enabled(driver, enabled=True, contexts=None, user_contexts=None):
    params = {"enabled": None if enabled else False}
    _scope(params, contexts, user_contexts, required=True)
    return _safe_run(driver, "emulation.setScriptingEnabled", params, "emulation.setScriptingEnabled")

def set_scrollbar_type_override(driver, scrollbar_type="overlay", contexts=None, user_contexts=None):
    value = None if scrollbar_type in (None, "default") else scrollbar_type
    if value not in (None, "classic", "overlay"):
        raise ValueError("scrollbar_type must be classic, overlay, or None")
    params = {"scrollbarType": value}
    _scope(params, contexts, user_contexts)
    return _safe_run(driver, "emulation.setScrollbarTypeOverride", params, "emulation.setScrollbarTypeOverride")

def set_forced_colors_mode_theme_override(driver, mode="none", contexts=None, user_contexts=None):
    theme = None if mode in (None, "none") else mode
    if theme not in (None, "light", "dark"):
        raise ValueError("mode must be light, dark, or none")
    params = {"theme": theme}
    _scope(params, contexts, user_contexts)
    return _safe_run(driver, "emulation.setForcedColorsModeThemeOverride", params, "emulation.setForcedColorsModeThemeOverride")
