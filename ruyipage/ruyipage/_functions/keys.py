# -*- coding: utf-8 -*-
"""键盘特殊键值常量。

定义 W3C WebDriver 规范中的所有特殊键值 Unicode 码点。
这些常量用于 Actions 动作链中的键盘操作。

用法::

    from ruyipage import Keys

    # 组合键 (推荐使用 combo 方法)
    page.actions.combo(Keys.CTRL, 'a').perform()       # Ctrl+A 全选
    page.actions.combo(Keys.CTRL, 'c').perform()       # Ctrl+C 复制

    # 单独按键
    page.actions.press(Keys.ENTER).perform()           # 按 Enter
    page.actions.press(Keys.TAB).perform()             # 按 Tab

    # 手动按下/释放
    page.actions.key_down(Keys.SHIFT).type('hello').key_up(Keys.SHIFT).perform()
"""


class Keys(object):
    """W3C WebDriver 特殊键值常量。

    每个常量对应一个 Unicode 私有区域码点 (U+E000-U+E03D)，
    浏览器会将这些码点识别为特殊按键操作。

    常用修饰键::

        Keys.CTRL      # Ctrl 键 (Windows/Linux)
        Keys.SHIFT     # Shift 键
        Keys.ALT       # Alt 键
        Keys.META      # Meta/Win/Cmd 键

    常用功能键::

        Keys.ENTER     # 回车键
        Keys.TAB       # Tab 键
        Keys.ESCAPE    # Esc 键
        Keys.BACKSPACE # 退格键
        Keys.DELETE    # Delete 键
        Keys.SPACE     # 空格键

    方向键::

        Keys.UP / Keys.ARROW_UP       # 上箭头
        Keys.DOWN / Keys.ARROW_DOWN   # 下箭头
        Keys.LEFT / Keys.ARROW_LEFT   # 左箭头
        Keys.RIGHT / Keys.ARROW_RIGHT # 右箭头
    """

    # ── 特殊控制键 ──────────────────────────────────────────────────

    NULL = '\ue000'       # Null 键
    CANCEL = '\ue001'     # Cancel 键
    HELP = '\ue002'       # Help 键
    BACKSPACE = '\ue003'  # 退格键
    BACK_SPACE = '\ue003' # 退格键（别名）
    TAB = '\ue004'        # Tab 键
    CLEAR = '\ue005'      # Clear 键
    RETURN = '\ue006'     # Return 键
    ENTER = '\ue007'      # Enter / 回车键

    # ── 修饰键 ──────────────────────────────────────────────────────

    SHIFT = '\ue008'      # Shift 键
    CONTROL = '\ue009'    # Control 键
    CTRL = '\ue009'       # Control 键（简写别名）
    ALT = '\ue00a'        # Alt 键
    META = '\ue03d'       # Meta / Windows / Command 键
    COMMAND = '\ue03d'    # Command 键（macOS 别名）

    # ── 功能键 ──────────────────────────────────────────────────────

    PAUSE = '\ue00b'      # Pause 键
    ESCAPE = '\ue00c'     # Escape 键
    ESC = '\ue00c'        # Escape 键（简写别名）
    SPACE = '\ue00d'      # 空格键

    # ── 导航键 ──────────────────────────────────────────────────────

    PAGE_UP = '\ue00e'    # Page Up 键
    PAGE_DOWN = '\ue00f'  # Page Down 键
    END = '\ue010'        # End 键
    HOME = '\ue011'       # Home 键

    # ── 方向键 ──────────────────────────────────────────────────────

    LEFT = '\ue012'        # 左箭头
    ARROW_LEFT = '\ue012'  # 左箭头（完整名称）
    UP = '\ue013'          # 上箭头
    ARROW_UP = '\ue013'    # 上箭头（完整名称）
    RIGHT = '\ue014'       # 右箭头
    ARROW_RIGHT = '\ue014' # 右箭头（完整名称）
    DOWN = '\ue015'        # 下箭头
    ARROW_DOWN = '\ue015'  # 下箭头（完整名称）

    # ── 编辑键 ──────────────────────────────────────────────────────

    INSERT = '\ue016'     # Insert 键
    DELETE = '\ue017'     # Delete 键

    # ── 符号键 ──────────────────────────────────────────────────────

    SEMICOLON = '\ue018'  # 分号键
    EQUALS = '\ue019'     # 等号键

    # ── 数字键盘 ─────────────────────────────────────────────────────

    NUMPAD0 = '\ue01a'    # 数字键盘 0
    NUMPAD1 = '\ue01b'    # 数字键盘 1
    NUMPAD2 = '\ue01c'    # 数字键盘 2
    NUMPAD3 = '\ue01d'    # 数字键盘 3
    NUMPAD4 = '\ue01e'    # 数字键盘 4
    NUMPAD5 = '\ue01f'    # 数字键盘 5
    NUMPAD6 = '\ue020'    # 数字键盘 6
    NUMPAD7 = '\ue021'    # 数字键盘 7
    NUMPAD8 = '\ue022'    # 数字键盘 8
    NUMPAD9 = '\ue023'    # 数字键盘 9
    MULTIPLY = '\ue024'   # 数字键盘 * (乘号)
    ADD = '\ue025'        # 数字键盘 + (加号)
    SEPARATOR = '\ue026'  # 数字键盘分隔符
    SUBTRACT = '\ue027'   # 数字键盘 - (减号)
    DECIMAL = '\ue028'    # 数字键盘 . (小数点)
    DIVIDE = '\ue029'     # 数字键盘 / (除号)

    # ── 功能键 F1-F12 ────────────────────────────────────────────────

    F1 = '\ue031'         # F1 功能键
    F2 = '\ue032'         # F2 功能键
    F3 = '\ue033'         # F3 功能键
    F4 = '\ue034'         # F4 功能键
    F5 = '\ue035'         # F5 功能键
    F6 = '\ue036'         # F6 功能键
    F7 = '\ue037'         # F7 功能键
    F8 = '\ue038'         # F8 功能键
    F9 = '\ue039'         # F9 功能键
    F10 = '\ue03a'        # F10 功能键
    F11 = '\ue03b'        # F11 功能键
    F12 = '\ue03c'        # F12 功能键
