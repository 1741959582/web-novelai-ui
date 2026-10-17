# -*- coding: utf-8 -*-
"""Drive an already-open fingerprint Firefox and nurture one X account.

The browser stays open. This process only attaches to its debug port.
"""
from __future__ import annotations

import json
import os
import random
import sys
import time

ROOT = os.environ.get("RUYIPAGE_ROOT", "").strip()
if ROOT and ROOT not in sys.path:
    sys.path.insert(0, ROOT)


def say(text: str) -> None:
    print(text, flush=True)


def stopped(path: str) -> bool:
    return bool(path) and os.path.exists(path)


def pause(seconds: float, stop_path: str) -> bool:
    deadline = time.time() + max(0.2, seconds)
    while time.time() < deadline:
        if stopped(stop_path):
            return False
        time.sleep(0.2)
    return True


def run_js(page, script: str):
    return page.run_js(script, as_expr=True, timeout=20)


def click_feed(page, feed: str) -> None:
    if feed != "following":
        return
    run_js(
        page,
        """
        (() => {
          const tabs = Array.from(document.querySelectorAll('[role="tab"], a[role="tab"]'));
          const tab = tabs.find((el) => /Following|正在关注/.test(el.innerText || el.textContent || ''));
          if (!tab) return false;
          tab.click();
          return true;
        })()
        """,
    )


def scroll_once(page) -> None:
    run_js(
        page,
        """
        (() => {
          const step = 520 + Math.floor(Math.random() * 480);
          window.scrollBy(0, step);
          const col = document.scrollingElement || document.documentElement;
          if (col) col.scrollTop += step;
          return true;
        })()
        """,
    )


def like_once(page) -> bool:
    result = run_js(
        page,
        """
        (() => {
          const articles = Array.from(document.querySelectorAll('article'));
          for (const art of articles) {
            const button = art.querySelector('[data-testid="like"]');
            if (!button) continue;
            button.click();
            const link = art.querySelector('a[href*="/status/"]');
            return { ok: true, href: link ? (link.getAttribute('href') || '') : '' };
          }
          return { ok: false };
        })()
        """,
    )
    return bool(isinstance(result, dict) and result.get("ok"))


def repost_once(page) -> bool:
    opened = run_js(
        page,
        """
        (() => {
          const articles = Array.from(document.querySelectorAll('article'));
          for (const art of articles) {
            const button = art.querySelector('[data-testid="retweet"]');
            if (!button) continue;
            button.click();
            return true;
          }
          return false;
        })()
        """,
    )
    if not opened:
        return False
    time.sleep(0.6)
    confirmed = run_js(
        page,
        """
        (() => {
          const button = document.querySelector('[data-testid="retweetConfirm"]');
          if (!button) return false;
          button.click();
          return true;
        })()
        """,
    )
    return bool(confirmed)


def follow_once(page) -> bool:
    result = run_js(
        page,
        """
        (() => {
          const buttons = Array.from(document.querySelectorAll('[data-testid="follow"]'));
          const button = buttons.find((el) => !el.closest('article')) || buttons[0];
          if (!button) return false;
          button.click();
          return true;
        })()
        """,
    )
    return bool(result)


def open_visible_post(page) -> bool:
    result = run_js(
        page,
        """
        (() => {
          const link = document.querySelector('article a[href*="/status/"]');
          if (!link) return false;
          link.click();
          return true;
        })()
        """,
    )
    return bool(result)


def ensure_home(page, feed: str, stop_path: str) -> bool:
    page.get("https://x.com/home")
    if not pause(3, stop_path):
        return False
    click_feed(page, feed)
    return pause(2, stop_path)


def within_time(cfg: dict) -> bool:
    if stopped(cfg["stop"]):
        return False
    deadline = float(cfg.get("deadline") or 0)
    return deadline <= 0 or time.time() < deadline


def roam(page, cfg: dict) -> None:
    stop_path = cfg["stop"]
    delay = float(cfg["delay"])
    while within_time(cfg):
        roll = random.random()
        if roll < 0.45 and like_once(page):
            say("已点赞")
        elif roll < 0.7 and repost_once(page):
            say("已转发")
        elif roll < 0.82 and follow_once(page):
            say("已关注")
        scroll_once(page)
        if not pause(delay * random.uniform(0.85, 1.35), stop_path):
            return
    say("这一轮结束")


def browse(page, cfg: dict) -> None:
    stop_path = cfg["stop"]
    delay = float(cfg["delay"])
    if not ensure_home(page, cfg.get("feed") or "for_you", stop_path):
        return
    deadline = float(cfg.get("deadline") or 0)
    rounds = int(cfg["rounds"])
    say("开始浏览" if deadline > 0 else f"开始浏览，共 {rounds} 轮")
    index = 0
    while True:
        if stopped(stop_path):
            say("已停止")
            return
        if deadline > 0 and time.time() >= deadline:
            break
        if deadline <= 0 and index >= rounds:
            break
        scroll_once(page)
        index += 1
        say(f"浏览 {index}" if deadline > 0 else f"浏览 {index}/{rounds}")
        if index % 3 == 2 and open_visible_post(page):
            say("点开一条帖子看一会儿")
            if not pause(delay * 1.6, stop_path):
                return
            try:
                page.run_js("history.back()", as_expr=True, timeout=10)
            except Exception:
                page.get("https://x.com/home")
            if not pause(delay, stop_path):
                return
            click_feed(page, cfg.get("feed") or "for_you")
        elif not pause(delay * random.uniform(0.85, 1.25), stop_path):
            return
    say("浏览结束")


def act_on_timeline(page, cfg: dict, likes: int, reposts: int) -> tuple[int, int]:
    stop_path = cfg["stop"]
    delay = float(cfg["delay"])
    rounds = max(1, int(cfg["rounds"]))
    for _ in range(rounds):
        if stopped(stop_path) or (likes <= 0 and reposts <= 0):
            break
        if likes > 0 and like_once(page):
            likes -= 1
            say(f"已点赞，还剩 {likes}")
            if not pause(delay, stop_path):
                break
        if reposts > 0 and repost_once(page):
            reposts -= 1
            say(f"已转发，还剩 {reposts}")
            if not pause(delay, stop_path):
                break
        scroll_once(page)
        if not pause(delay * 0.6, stop_path):
            break
    return likes, reposts


def interact(page, cfg: dict) -> None:
    stop_path = cfg["stop"]
    delay = float(cfg["delay"])
    likes = int(cfg["likes"])
    reposts = int(cfg["reposts"])
    follows = int(cfg["follows"])
    handles = [item for item in cfg.get("handles") or [] if item]
    if likes <= 0 and reposts <= 0 and follows <= 0:
        say("开始互动，在设定的时间里随机点赞、转发和关注")
        for raw in handles:
            if not within_time(cfg):
                break
            target = str(raw).strip()
            if "/status/" in target:
                say(f"打开帖子 {target}")
                page.get(target)
            else:
                name = target[1:] if target.startswith("@") else target
                say(f"打开 @{name}")
                page.get(f"https://x.com/{name}")
            if not pause(3, stop_path):
                return
            if "/status/" not in target and follow_once(page):
                say("已关注")
                if not pause(delay, stop_path):
                    return
        if not within_time(cfg):
            say("互动结束")
            return
        if not ensure_home(page, cfg.get("feed") or "following", stop_path):
            return
        roam(page, cfg)
        return
    say(f"开始互动，点赞 {likes}，转发 {reposts}，关注 {follows}")
    if not handles:
        if follows:
            say("没有填写用户名，这次不关注别人，只处理正在关注时间线里的帖子")
        if not ensure_home(page, cfg.get("feed") or "following", stop_path):
            return
        act_on_timeline(page, cfg, likes, reposts)
        say("互动结束")
        return
    for raw in handles:
        if stopped(stop_path) or (likes <= 0 and reposts <= 0 and follows <= 0):
            break
        target = str(raw).strip()
        if "/status/" in target:
            say(f"打开帖子 {target}")
            page.get(target)
        else:
            name = target[1:] if target.startswith("@") else target
            say(f"打开 @{name}")
            page.get(f"https://x.com/{name}")
        if not pause(3, stop_path):
            return
        if follows > 0 and "/status/" not in target and follow_once(page):
            follows -= 1
            say(f"已关注，还剩 {follows}")
            if not pause(delay, stop_path):
                return
        likes, reposts = act_on_timeline(page, cfg, likes, reposts)
    say("互动结束")


def main() -> int:
    if len(sys.argv) < 2:
        say("缺少养号配置")
        return 1
    with open(sys.argv[1], "r", encoding="utf-8") as handle:
        cfg = json.load(handle)
    port = int(cfg["port"])
    try:
        from ruyipage import FirefoxOptions, FirefoxPage
    except Exception as error:
        say(f"没有可用的 ruyipage：{error}")
        return 1
    say(f"连接指纹浏览器端口 {port}")
    page = None
    try:
        options = FirefoxOptions().set_address(f"127.0.0.1:{port}").existing_only(True)
        options.set_bidi_discovery_timeout(12)
        page = FirefoxPage(options)
        if cfg.get("mode") == "interact":
            interact(page, cfg)
        elif cfg.get("mode") == "plaza":
            page.get("https://x.com/explore")
            if pause(3, cfg["stop"]):
                if int(cfg["likes"]) <= 0 and int(cfg["reposts"]) <= 0:
                    say("开始逛广场")
                    roam(page, cfg)
                else:
                    act_on_timeline(page, cfg, int(cfg["likes"]), int(cfg["reposts"]))
                    say("广场这一轮结束")
        else:
            browse(page, cfg)
        return 0
    except Exception as error:
        text = str(error)
        lowered = text.lower()
        if "session" in lowered or "active sessions" in lowered:
            say("指纹浏览器的调试端口正被预览占用，这次没能接上控制。")
        else:
            say(f"养号中断：{text}")
        return 1
    finally:
        if page is not None:
            try:
                page.quit(timeout=3, force=False)
            except Exception:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
