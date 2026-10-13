# -*- coding: utf-8 -*-
"""BiDi 触摸动作链 — 基于 W3C input.performActions (pointerType: touch)。

通过 BiDi 协议实现全部触摸手势，支持多指操作。
每个手指独立一条 action 序列，通过 perform() 一次性发送。

用法::

    # 单指点击
    page.touch.tap(ele).perform()

    # 长按 + 向上滑动
    page.touch.long_press(ele).swipe_up(300).perform()

    # 双指捏合缩放
    page.touch.pinch_in(cx=400, cy=600).perform()

    # 自定义滑动
    page.touch.swipe(100, 500, 100, 100, duration=300).perform()
"""

import random
import math


class TouchActions:
    """BiDi 触摸动作链管理器。

    通过 BiDi input.performActions 使用 pointerType: "touch" 实现。
    支持多指操作（每个手指用 fid 标识），所有手指的动作序列
    在 perform() 时自动补齐到相同长度后并行执行。

    属性:
        _owner: 所属的 FirefoxBase 页面/标签页对象。
        _fingers: 手指动作字典 {fid: [actions]}。
        _x: 当前触摸 X 坐标。
        _y: 当前触摸 Y 坐标。
    """

    def __init__(self, owner):
        """初始化触摸动作链。

        Args:
            owner: 所属的 FirefoxBase 页面/标签页对象，
                   提供 _driver 和 _context_id 用于执行 BiDi 命令。
        """
        self._owner = owner
        self._fingers = {}   # 每个手指一条 action 序列, key = finger_id (int)
        self._x = 0          # 当前触摸 X 坐标 (视口像素)
        self._y = 0          # 当前触摸 Y 坐标 (视口像素)

    # ── 内部辅助 ──────────────────────────────────────────────────

    def _finger(self, fid=0):
        """获取指定手指的动作序列（不存在则创建）。

        Args:
            fid: 手指标识符 (整数)。0 为默认手指。

        Returns:
            list: 该手指的动作列表。
        """
        if fid not in self._fingers:
            self._fingers[fid] = []
        return self._fingers[fid]

    def _resolve(self, ele_or_loc):
        """解析目标位置为 (x, y) 坐标。

        Args:
            ele_or_loc: 目标位置，支持:
                        - None: 使用当前触摸位置
                        - tuple/list: (x, y) 坐标
                        - dict: {'x': int, 'y': int}
                        - FirefoxElement: 使用元素中心

        Returns:
            tuple: (x, y) 坐标。
        """
        if ele_or_loc is None:
            return self._x, self._y
        if isinstance(ele_or_loc, (list, tuple)):
            return float(ele_or_loc[0]), float(ele_or_loc[1])
        if isinstance(ele_or_loc, dict):
            return float(ele_or_loc['x']), float(ele_or_loc['y'])
        # 元素对象
        pos = getattr(ele_or_loc, '_get_center', lambda: None)()
        if pos:
            return float(pos['x']), float(pos['y'])
        return self._x, self._y

    def _pad_to(self, fid, length):
        """用 pause 补齐序列长度至指定长度。

        Args:
            fid:    手指标识符。
            length: 目标长度。
        """
        seq = self._finger(fid)
        while len(seq) < length:
            seq.append({'type': 'pause', 'duration': 0})

    # ── 单指基础操作 ──────────────────────────────────────────────

    def move_to(self, ele_or_loc, offset_x=0, offset_y=0, duration=50, fid=0):
        """移动手指到目标位置。

        Args:
            ele_or_loc: 目标位置 (元素 / 坐标 dict / 坐标 tuple / None)。
            offset_x:   X 方向偏移量 (像素)。默认 0。
            offset_y:   Y 方向偏移量 (像素)。默认 0。
            duration:   移动时长 (毫秒)。默认 50。
            fid:        手指标识符。默认 0。

        Returns:
            self: 支持链式调用。
        """
        x, y = self._resolve(ele_or_loc)
        x += offset_x
        y += offset_y
        self._x, self._y = x, y
        self._finger(fid).append({
            'type': 'pointerMove', 'x': int(x), 'y': int(y), 'duration': duration
        })
        return self

    def touch_down(self, ele_or_loc=None, fid=0):
        """手指按下（触摸屏幕）。

        Args:
            ele_or_loc: 按下位置。为 None 时在当前位置按下。默认 None。
            fid:        手指标识符。默认 0。

        Returns:
            self: 支持链式调用。
        """
        if ele_or_loc is not None:
            self.move_to(ele_or_loc, fid=fid)
        self._finger(fid).append({'type': 'pointerDown', 'button': 0})
        return self

    def touch_up(self, ele_or_loc=None, fid=0):
        """手指抬起（离开屏幕）。

        Args:
            ele_or_loc: 抬起位置。为 None 时在当前位置抬起。默认 None。
            fid:        手指标识符。默认 0。

        Returns:
            self: 支持链式调用。
        """
        if ele_or_loc is not None:
            self.move_to(ele_or_loc, fid=fid)
        self._finger(fid).append({'type': 'pointerUp', 'button': 0})
        return self

    def pause(self, ms=100, fid=0):
        """暂停指定时长。

        Args:
            ms:  暂停时长 (毫秒)。默认 100。
            fid: 手指标识符。默认 0。

        Returns:
            self: 支持链式调用。
        """
        self._finger(fid).append({'type': 'pause', 'duration': ms})
        return self

    # ── 高级手势 ──────────────────────────────────────────────────

    def tap(self, ele_or_loc=None, times=1):
        """单指点击（模拟手指轻触）。

        Args:
            ele_or_loc: 点击位置。为 None 时在当前位置点击。默认 None。
            times:      点击次数。默认 1。传 2 可模拟双击。

        Returns:
            self: 支持链式调用。
        """
        if ele_or_loc is not None:
            self.move_to(ele_or_loc)
        for _ in range(times):
            self.touch_down().pause(random.randint(60, 120)).touch_up()
            if times > 1:
                self.pause(random.randint(80, 150))
        return self

    def double_tap(self, ele_or_loc=None):
        """双击（两次快速点击）。

        两次 tap 间隔 ~100ms，符合浏览器 dblclick 识别阈值。

        Args:
            ele_or_loc: 双击位置。为 None 时在当前位置双击。默认 None。

        Returns:
            self: 支持链式调用。
        """
        if ele_or_loc is not None:
            self.move_to(ele_or_loc)
        self.touch_down().pause(60).touch_up().pause(100).touch_down().pause(60).touch_up()
        return self

    def long_press(self, ele_or_loc=None, duration=800):
        """长按。

        Args:
            ele_or_loc: 长按位置。为 None 时在当前位置长按。默认 None。
            duration:   按住时长 (毫秒)。默认 800。

        Returns:
            self: 支持链式调用。
        """
        if ele_or_loc is not None:
            self.move_to(ele_or_loc)
        self.touch_down().pause(duration).touch_up()
        return self

    def swipe(self, x1, y1, x2, y2, duration=400, steps=0):
        """自定义方向滑动。

        从 (x1, y1) 滑动到 (x2, y2)，中间生成平滑的过渡点。

        Args:
            x1:       起始 X 坐标 (视口像素)。
            y1:       起始 Y 坐标 (视口像素)。
            x2:       结束 X 坐标 (视口像素)。
            y2:       结束 Y 坐标 (视口像素)。
            duration: 滑动总时长 (毫秒)。默认 400。
            steps:    中间步数。为 0 时自动计算。默认 0。

        Returns:
            self: 支持链式调用。
        """
        steps = steps or max(10, int(duration / 16))
        self._finger(0).append({
            'type': 'pointerMove', 'x': int(x1), 'y': int(y1), 'duration': 0
        })
        self._finger(0).append({'type': 'pointerDown', 'button': 0})
        step_dur = max(1, duration // steps)
        for i in range(1, steps + 1):
            px = int(x1 + (x2 - x1) * i / steps)
            py = int(y1 + (y2 - y1) * i / steps)
            self._finger(0).append({
                'type': 'pointerMove', 'x': px, 'y': py, 'duration': step_dur
            })
        self._finger(0).append({'type': 'pointerUp', 'button': 0})
        self._x, self._y = x2, y2
        return self

    def swipe_up(self, distance=400, x=None, duration=400):
        """向上滑动。

        从屏幕下方 65% 处开始向上滑动。

        Args:
            distance: 滑动距离 (像素)。默认 400。
            x:        滑动的 X 坐标。为 None 时使用屏幕中心。默认 None。
            duration: 滑动时长 (毫秒)。默认 400。

        Returns:
            self: 支持链式调用。
        """
        w, h = self._owner.rect.viewport_size
        cx = x or w // 2
        cy = int(h * 0.65)
        return self.swipe(cx, cy, cx, cy - distance, duration)

    def swipe_down(self, distance=400, x=None, duration=400):
        """向下滑动。

        从屏幕上方 35% 处开始向下滑动。

        Args:
            distance: 滑动距离 (像素)。默认 400。
            x:        滑动的 X 坐标。为 None 时使用屏幕中心。默认 None。
            duration: 滑动时长 (毫秒)。默认 400。

        Returns:
            self: 支持链式调用。
        """
        w, h = self._owner.rect.viewport_size
        cx = x or w // 2
        cy = int(h * 0.35)
        return self.swipe(cx, cy, cx, cy + distance, duration)

    def swipe_left(self, distance=300, y=None, duration=300):
        """向左滑动。

        从屏幕右侧 65% 处开始向左滑动。

        Args:
            distance: 滑动距离 (像素)。默认 300。
            y:        滑动的 Y 坐标。为 None 时使用屏幕中心。默认 None。
            duration: 滑动时长 (毫秒)。默认 300。

        Returns:
            self: 支持链式调用。
        """
        w, h = self._owner.rect.viewport_size
        cy = y or h // 2
        cx = int(w * 0.65)
        return self.swipe(cx, cy, cx - distance, cy, duration)

    def swipe_right(self, distance=300, y=None, duration=300):
        """向右滑动。

        从屏幕左侧 35% 处开始向右滑动。

        Args:
            distance: 滑动距离 (像素)。默认 300。
            y:        滑动的 Y 坐标。为 None 时使用屏幕中心。默认 None。
            duration: 滑动时长 (毫秒)。默认 300。

        Returns:
            self: 支持链式调用。
        """
        w, h = self._owner.rect.viewport_size
        cy = y or h // 2
        cx = int(w * 0.35)
        return self.swipe(cx, cy, cx + distance, cy, duration)

    def pinch_in(self, cx=None, cy=None, start_gap=200, end_gap=60, duration=400):
        """双指捏合（缩小/zoom out）。

        两个手指从外向内移动。

        Args:
            cx:        捏合中心 X 坐标。为 None 时使用屏幕中心。默认 None。
            cy:        捏合中心 Y 坐标。为 None 时使用屏幕中心。默认 None。
            start_gap: 起始时两指间距 (像素)。默认 200。
            end_gap:   结束时两指间距 (像素)。默认 60。
            duration:  动作时长 (毫秒)。默认 400。

        Returns:
            self: 支持链式调用。
        """
        return self._two_finger_zoom(cx, cy, start_gap, end_gap, duration)

    def pinch_out(self, cx=None, cy=None, start_gap=60, end_gap=200, duration=400):
        """双指张开（放大/zoom in）。

        两个手指从内向外移动。

        Args:
            cx:        张开中心 X 坐标。为 None 时使用屏幕中心。默认 None。
            cy:        张开中心 Y 坐标。为 None 时使用屏幕中心。默认 None。
            start_gap: 起始时两指间距 (像素)。默认 60。
            end_gap:   结束时两指间距 (像素)。默认 200。
            duration:  动作时长 (毫秒)。默认 400。

        Returns:
            self: 支持链式调用。
        """
        return self._two_finger_zoom(cx, cy, start_gap, end_gap, duration)

    def _two_finger_zoom(self, cx, cy, start_gap, end_gap, duration):
        """双指缩放内部实现。

        Args:
            cx:        中心 X 坐标。
            cy:        中心 Y 坐标。
            start_gap: 起始间距。
            end_gap:   结束间距。
            duration:  动作时长。

        Returns:
            self: 支持链式调用。
        """
        w, h = self._owner.rect.viewport_size
        cx = cx or w // 2
        cy = cy or h // 2
        steps = max(10, int(duration / 16))
        step_dur = max(1, duration // steps)

        for fid in (0, 1):
            half_start = start_gap / 2
            sign = -1 if fid == 0 else 1
            sx = cx + sign * half_start
            self._finger(fid).append({
                'type': 'pointerMove', 'x': int(sx), 'y': int(cy), 'duration': 0
            })
            self._finger(fid).append({'type': 'pointerDown', 'button': 0})

        for i in range(1, steps + 1):
            gap = start_gap + (end_gap - start_gap) * i / steps
            half = gap / 2
            for fid in (0, 1):
                sign = -1 if fid == 0 else 1
                px = cx + sign * half
                self._finger(fid).append({
                    'type': 'pointerMove', 'x': int(px), 'y': int(cy), 'duration': step_dur
                })

        for fid in (0, 1):
            self._finger(fid).append({'type': 'pointerUp', 'button': 0})

        return self

    def rotate(self, cx=None, cy=None, radius=100, start_angle=0, end_angle=90, duration=500):
        """双指旋转。

        两个手指以中心点为圆心，沿圆弧同步旋转。

        Args:
            cx:          旋转中心 X 坐标。为 None 时使用屏幕中心。默认 None。
            cy:          旋转中心 Y 坐标。为 None 时使用屏幕中心。默认 None。
            radius:      旋转半径 (像素)。默认 100。
            start_angle: 起始角度 (度)。默认 0。
            end_angle:   结束角度 (度)。默认 90。
            duration:    动作时长 (毫秒)。默认 500。

        Returns:
            self: 支持链式调用。
        """
        w, h = self._owner.rect.viewport_size
        cx = cx or w // 2
        cy = cy or h // 2
        steps = max(10, int(duration / 16))
        step_dur = max(1, duration // steps)

        for fid in (0, 1):
            angle_offset = 0 if fid == 0 else 180
            a = math.radians(start_angle + angle_offset)
            sx = cx + radius * math.cos(a)
            sy = cy + radius * math.sin(a)
            self._finger(fid).append({
                'type': 'pointerMove', 'x': int(sx), 'y': int(sy), 'duration': 0
            })
            self._finger(fid).append({'type': 'pointerDown', 'button': 0})

        for i in range(1, steps + 1):
            angle = start_angle + (end_angle - start_angle) * i / steps
            for fid in (0, 1):
                angle_offset = 0 if fid == 0 else 180
                a = math.radians(angle + angle_offset)
                px = cx + radius * math.cos(a)
                py = cy + radius * math.sin(a)
                self._finger(fid).append({
                    'type': 'pointerMove', 'x': int(px), 'y': int(py), 'duration': step_dur
                })

        for fid in (0, 1):
            self._finger(fid).append({'type': 'pointerUp', 'button': 0})

        return self

    def flick(self, ele_or_loc=None, vx=0, vy=-1000, duration=150):
        """快速轻弹手势。

        以指定速度快速滑动一小段距离。

        Args:
            ele_or_loc: 起始位置。为 None 时使用当前位置。默认 None。
            vx:         水平速度 (像素/秒)。正值向右。默认 0。
            vy:         垂直速度 (像素/秒)。负值向上。默认 -1000。
            duration:   轻弹时长 (毫秒)。默认 150。

        Returns:
            self: 支持链式调用。
        """
        if ele_or_loc is not None:
            self.move_to(ele_or_loc)
        dx = int(vx * duration / 1000)
        dy = int(vy * duration / 1000)
        x2 = self._x + dx
        y2 = self._y + dy
        return self.swipe(self._x, self._y, x2, y2, duration)

    # ── 执行与释放 ────────────────────────────────────────────────

    def perform(self):
        """执行所有积累的触摸动作。

        通过 BiDi input.performActions 命令发送，所有手指的动作
        序列自动补齐到相同长度后并行执行。执行后清空动作队列。

        Returns:
            self: 支持链式调用。
        """
        if not self._fingers:
            return self

        # 补齐所有手指序列到相同长度
        max_len = max(len(v) for v in self._fingers.values())
        for fid in self._fingers:
            self._pad_to(fid, max_len)

        actions = []
        for fid in sorted(self._fingers.keys()):
            actions.append({
                'type': 'pointer',
                'id': 'touch{}'.format(fid),
                'parameters': {'pointerType': 'touch'},
                'actions': self._fingers[fid][:]
            })

        self._owner._driver._browser_driver.run('input.performActions', {
            'context': self._owner._context_id,
            'actions': actions
        })

        self._fingers.clear()
        self._x = 0
        self._y = 0
        return self

    def release_all(self):
        """释放所有触摸点。

        通过 BiDi input.releaseActions 重置触摸状态。

        Returns:
            self: 支持链式调用。
        """
        self._owner._driver._browser_driver.run('input.releaseActions', {
            'context': self._owner._context_id
        })
        self._fingers.clear()
        return self
