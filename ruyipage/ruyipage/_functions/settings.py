# -*- coding: utf-8 -*-
"""全局设置"""


class Settings(object):
    """全局配置，类属性直接使用，无需实例化"""

    # 元素未找到时是否抛出异常
    raise_when_ele_not_found = False

    # 点击失败时是否抛出异常
    raise_when_click_failed = False

    # 等待失败时是否抛出异常
    raise_when_wait_failed = False

    # 是否使用单例模式管理标签页对象
    singleton_tab_obj = True

    # BiDi 命令默认超时（秒）
    bidi_timeout = 30

    # 浏览器连接默认超时（秒）
    browser_connect_timeout = 30

    # 元素查找默认超时（秒）
    element_find_timeout = 10

    # 页面加载默认超时（秒）
    page_load_timeout = 30

    # 脚本执行默认超时（秒）
    script_timeout = 30

    # 响应体读取默认超时（秒）
    response_body_timeout = 10

    # page.listen 历史缓冲区最大数据包数，超出后丢弃最旧的包。
    # 长时间监听高流量站点时，该上限决定了内存占用的天花板。
    listen_max_packets = 1000

    # ── 诊断与追踪 ──

    # 是否启用 debug trace 记录（默认关闭，零开销）
    trace_enabled = False

    # trace 缓冲区最大条目数
    trace_max_entries = 1000

    # 是否在操作失败时自动收集诊断快照（默认关闭）
    failure_snapshot_enabled = False

    # 诊断快照中 DOM HTML 最大字节数（超出则截断）
    snapshot_dom_max_bytes = 2 * 1024 * 1024  # 2MB

    # 诊断快照中包含的最近网络请求数
    snapshot_recent_requests = 30
