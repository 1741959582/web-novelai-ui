# -*- coding: utf-8 -*-
"""Simple passive packet capture API."""

import base64
import logging
import re
import time
from queue import Empty, Queue
from threading import Lock

from .._bidi import session as bidi_session
from ..errors import BiDiError
from .._functions.queue_utils import queue_get as _queue_get
from .._functions.settings import Settings
from .._functions.sleep import sleep as _sleep

logger = logging.getLogger("ruyipage")


def _needs_global_scope_fallback(exc):
    if exc.error != "unsupported operation":
        return False
    message = (exc.bidi_message or "").lower()
    return (
        "privileged scope" in message
        or "system access is required" in message
    )


def _headers_from_bidi(headers):
    """Convert BiDi header list to a lower-case-key dictionary."""
    result = {}
    for item in headers or []:
        name = item.get("name", "")
        if not name:
            continue
        value_obj = item.get("value", {})
        value = (
            value_obj.get("value", "")
            if isinstance(value_obj, dict)
            else str(value_obj)
        )
        result[name.lower()] = value
    return result


class CapturePacket(object):
    """A completed captured network packet.

    Attributes:
        url (str): Request URL.
        method (str): HTTP method.
        request_id (str): Browser-assigned request id.
        request_headers (dict): Request headers with lower-case keys.
        request_body (str | None): Decoded request body.
        response_status (int): Response status code. Failed requests use ``0``.
        response_headers (dict): Response headers with lower-case keys.
        response_body (str | None): Decoded response body.
        is_failed (bool): Whether the request ended with ``network.fetchError``.
        request (dict): Raw BiDi request object.
        response (dict): Raw BiDi response object.
    """

    def __init__(
        self,
        request=None,
        response=None,
        event_type="",
        timestamp=0,
        request_collector=None,
        response_collector=None,
        owner=None,
    ):
        self.request = request or {}
        self.response = response or {}
        self.event_type = event_type
        self.timestamp = timestamp
        self._request_collector = request_collector
        self._response_collector = response_collector
        self._owner = owner
        self._request_body = None
        self._request_body_loaded = False
        self._response_body = None
        self._response_body_loaded = False

        self.url = self.request.get("url", "")
        self.method = (self.request.get("method", "") or "").upper()
        self.request_headers = _headers_from_bidi(self.request.get("headers", []))
        self.response_status = 0
        self.response_headers = {}
        self._apply_response(response or {}, event_type, timestamp)

    @property
    def request_id(self):
        """Unique request id used by BiDi DataCollector."""
        return self.request.get("request", "") or ""

    @property
    def status(self):
        """Alias for ``response_status``."""
        return self.response_status

    @property
    def headers(self):
        """Alias for ``response_headers``."""
        return self.response_headers

    @property
    def body(self):
        """Alias for ``response_body``."""
        return self.response_body

    @property
    def text(self):
        """Alias for ``response_body``."""
        return self.response_body

    @property
    def is_failed(self):
        """Whether the request ended with ``network.fetchError``."""
        return self.event_type == "fetchError"

    @property
    def request_body(self):
        """Decoded request body, or ``None`` if unavailable."""
        return self.get_request_body()

    @property
    def response_body(self):
        """Decoded response body, or ``None`` if unavailable."""
        return self.get_response_body()

    def _apply_response(self, response, event_type=None, timestamp=None):
        if response is None:
            response = {}
        self.response = response
        if event_type:
            self.event_type = event_type
        if timestamp is not None:
            self.timestamp = timestamp
        self.response_status = response.get("status", 0) if response else 0
        self.response_headers = _headers_from_bidi(response.get("headers", []))

    def _decode_body_value(self, body):
        if body is None:
            return None
        if isinstance(body, str):
            return body
        if not isinstance(body, dict):
            return str(body)

        body_type = body.get("type")
        value = body.get("value")
        if value is None:
            return None
        if body_type == "string":
            return str(value)
        if body_type == "base64":
            try:
                raw_bytes = base64.b64decode(value)
            except Exception:
                logger.debug("Failed to decode captured base64 body")
                return None
            try:
                return raw_bytes.decode("utf-8")
            except UnicodeDecodeError:
                return raw_bytes.decode("utf-8", errors="replace")
        return str(value)

    def _body_from_collector(self, collector, data_type, timeout=0):
        if not collector or not self.request_id:
            return None

        deadline = time.monotonic() + max(timeout or 0, 0)
        interval = 0.05

        while True:
            try:
                data = collector.get(self.request_id, data_type=data_type)
                decoded = self._decode_body_value(getattr(data, "base64", None))
                if decoded is not None:
                    return decoded
                decoded = self._decode_body_value(getattr(data, "bytes", None))
                if decoded is not None:
                    return decoded
                raw = getattr(data, "raw", None)
                if isinstance(raw, dict):
                    for key in ("data", "body", "value"):
                        decoded = self._decode_body_value(raw.get(key))
                        if decoded is not None:
                            return decoded
            except Exception as exc:
                logger.debug("Failed to read captured %s body: %s", data_type, exc)

            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return None
            _sleep(min(interval, remaining))
            interval = min(interval * 1.5, 0.5)

    def get_request_body(self, timeout=0):
        """Read the decoded request body."""
        if self._request_body_loaded:
            return self._request_body

        body = self._decode_body_value(self.request.get("body"))
        if body is None:
            body = self._body_from_collector(
                self._request_collector,
                "request",
                timeout=timeout,
            )
        self._request_body = body
        self._request_body_loaded = True
        return body

    def get_response_body(self, timeout=None):
        """Read the decoded response body."""
        if self._response_body_loaded:
            return self._response_body
        if timeout is None:
            timeout = Settings.response_body_timeout
        body = self._body_from_collector(
            self._response_collector,
            "response",
            timeout=timeout,
        )
        if body is None:
            body = self._fallback_fetch_body()
        self._response_body = body
        self._response_body_loaded = True
        return body

    def _fallback_fetch_body(self):
        """Replay a GET request with page fetch when BiDi body data is empty."""
        if self.method != "GET" or not self.url or not self._owner:
            return None
        try:
            # Firefox BiDi DataCollector can occasionally return no body for
            # completed Brotli responses, such as Bing search pages with
            # ``Content-Encoding: br``. Keep this GET-only replay as a
            # compatibility fallback until Firefox exposes those bodies
            # reliably through network.getData.
            result = self._owner.run_js(
                'return fetch(arguments[0], {credentials: "include"})'
                '.then(r => r.text())',
                self.url,
                timeout=15,
            )
            if result and isinstance(result, str):
                return result
        except Exception as exc:
            logger.debug("Failed to fallback-fetch captured GET body: %s", exc)
        return None

    def to_dict(self, include_bodies=True):
        """Return a plain dictionary snapshot of the packet."""
        result = {
            "url": self.url,
            "method": self.method,
            "request_id": self.request_id,
            "request_headers": dict(self.request_headers),
            "response_status": self.response_status,
            "response_headers": dict(self.response_headers),
            "is_failed": self.is_failed,
        }
        if include_bodies:
            result["request_body"] = self.request_body
            result["response_body"] = self.response_body
        return result

    def __repr__(self):
        return "<CapturePacket {} {} {}>".format(
            self.method,
            self.response_status,
            self.url[:60],
        )


class CaptureManager(object):
    """Passive request/response packet capture manager.

    Access it through ``page.capture``. It is passive and does not block,
    modify, mock, or fail requests.
    """

    def __init__(self, owner):
        self._owner = owner
        self._active = False
        self._targets = True
        self._is_regex = False
        self._method_filter = None
        self._caught = Queue()
        self._packets = []
        self._pending = {}
        self._lock = Lock()
        self._subscription_id = None
        self._request_collector = None
        self._response_collector = None
        self._collect_bodies = True
        self._capture_driver = None
        self._capture_browser_driver = None

    @property
    def active(self):
        """Whether packet capture is currently active."""
        return self._active

    @property
    def listening(self):
        """Alias for ``active`` for naming consistency with ``page.listen``."""
        return self._active

    @property
    def steps(self):
        """All captured packets since the last ``start()`` or ``clear()``."""
        with self._lock:
            return self._packets[:]

    def start(
        self,
        targets=True,
        *,
        method=None,
        is_regex=False,
        collect_bodies=True,
        max_body_size=10485760,
    ):
        """Start passive packet capture.

        Args:
            targets: URL match rule. ``True`` captures all requests; ``str``
                uses substring matching; ``list[str]`` matches any item.
            method: Optional HTTP method filter, such as ``"GET"`` or
                ``"POST"``. Matching is case-insensitive.
            is_regex: Treat string targets as regular expressions.
            collect_bodies: Whether to collect request and response bodies.
            max_body_size: Max encoded body size for the internal collector.

        Returns:
            CaptureManager: ``self`` for chained use.
        """
        if self._active:
            self.stop()

        self._is_regex = bool(is_regex)
        self._method_filter = method.upper() if method else None
        self._collect_bodies = bool(collect_bodies)

        if targets is True:
            self._targets = True
        elif isinstance(targets, str):
            self._targets = {targets}
        elif isinstance(targets, (list, tuple, set)):
            self._targets = set(targets)
        else:
            self._targets = True

        self._caught = Queue()
        with self._lock:
            self._packets = []
            self._pending = {}

        self._request_collector = None
        self._response_collector = None
        if collect_bodies:
            try:
                try:
                    collector = self._owner.network.add_data_collector(
                        data_types=["request", "response"],
                        max_encoded_data_size=max_body_size,
                    )
                except BiDiError as exc:
                    if not _needs_global_scope_fallback(exc):
                        raise
                    logger.warning(
                        "Context-scoped capture data collector failed; "
                        "retrying globally: %s",
                        exc,
                    )
                    collector = self._owner.network._add_data_collector(
                        data_types=["request", "response"],
                        max_encoded_data_size=max_body_size,
                        contexts=None,
                    )
                self._request_collector = collector
                self._response_collector = collector
            except Exception as exc:
                logger.debug("Failed to start capture data collector: %s", exc)

        events = [
            "network.beforeRequestSent",
            "network.responseCompleted",
            "network.fetchError",
        ]
        browser_driver = self._owner._driver._browser_driver
        driver = self._owner._driver
        self._capture_driver = driver
        self._capture_browser_driver = browser_driver
        self._subscription_id = None
        registered_events = []
        try:
            try:
                result = bidi_session.subscribe(
                    browser_driver,
                    events,
                    contexts=[self._owner._context_id],
                )
            except BiDiError as exc:
                if not _needs_global_scope_fallback(exc):
                    raise
                logger.warning(
                    "Context-scoped capture subscription failed; "
                    "retrying globally: %s",
                    exc,
                )
                result = bidi_session.subscribe(browser_driver, events)
            self._subscription_id = result.get("subscription")

            driver.set_callback("network.beforeRequestSent", self._on_request)
            registered_events.append("network.beforeRequestSent")
            driver.set_callback("network.responseCompleted", self._on_response)
            registered_events.append("network.responseCompleted")
            driver.set_callback("network.fetchError", self._on_fetch_error)
            registered_events.append("network.fetchError")
        except Exception:
            for event in registered_events:
                try:
                    driver.remove_callback(event)
                except Exception:
                    pass
            if self._subscription_id:
                try:
                    bidi_session.unsubscribe(
                        browser_driver,
                        subscription=self._subscription_id,
                    )
                except Exception:
                    pass
            self._subscription_id = None
            collector = self._response_collector or self._request_collector
            if collector:
                try:
                    collector.remove()
                except Exception:
                    pass
            self._request_collector = None
            self._response_collector = None
            self._capture_driver = None
            self._capture_browser_driver = None
            raise

        self._active = True
        return self

    def start_capture(self, *args, **kwargs):
        """Alias for ``start()``."""
        return self.start(*args, **kwargs)

    def stop(self):
        """Stop packet capture and release internal resources."""
        if not self._active:
            return self
        self._active = False

        for packet in self.steps:
            try:
                packet.get_request_body(timeout=0.5)
            except Exception:
                pass
            try:
                packet.get_response_body(timeout=0.5)
            except Exception:
                pass

        if self._subscription_id:
            try:
                bidi_session.unsubscribe(
                    self._capture_browser_driver,
                    subscription=self._subscription_id,
                )
            except Exception:
                pass
            self._subscription_id = None

        driver = self._capture_driver
        if driver:
            driver.remove_callback("network.beforeRequestSent")
            driver.remove_callback("network.responseCompleted")
            driver.remove_callback("network.fetchError")

        collector = self._response_collector or self._request_collector
        if collector:
            try:
                collector.remove()
            except Exception:
                pass
        self._request_collector = None
        self._response_collector = None
        self._capture_driver = None
        self._capture_browser_driver = None
        with self._lock:
            self._pending = {}
        return self

    def wait(self, timeout=None, count=1):
        """Wait for captured packet(s).

        ``count=1`` returns one ``CapturePacket`` or ``None``. ``count>1``
        returns a list and may contain fewer than ``count`` packets on timeout.
        """
        if timeout is None:
            timeout = Settings.bidi_timeout

        end_time = time.time() + timeout
        results = []
        while len(results) < count:
            remaining = end_time - time.time()
            if remaining <= 0:
                break
            try:
                packet = _queue_get(self._caught, timeout=min(remaining, 0.5))
                results.append(packet)
            except Empty:
                continue

        if count == 1:
            return results[0] if results else None
        return results

    def wait_capture(self, *args, **kwargs):
        """Alias for ``wait()``."""
        return self.wait(*args, **kwargs)

    def clear(self):
        """Clear queued, captured, and pending packets."""
        while not self._caught.empty():
            try:
                self._caught.get_nowait()
            except Empty:
                break
        with self._lock:
            self._packets = []
            self._pending = {}
        return self

    def _match(self, url, method):
        if self._method_filter and (method or "").upper() != self._method_filter:
            return False

        if self._targets is True:
            return True

        for pattern in self._targets:
            if self._is_regex:
                if re.search(pattern, url or ""):
                    return True
            elif pattern in (url or ""):
                return True
        return False

    def _store_pending(self, packet):
        if not packet.request_id:
            return
        with self._lock:
            self._pending[packet.request_id] = packet

    def _pop_pending(self, request_id):
        if not request_id:
            return None
        with self._lock:
            return self._pending.pop(request_id, None)

    def _emit(self, packet):
        with self._lock:
            self._packets.append(packet)
        self._caught.put(packet)

    def _on_request(self, params):
        if not self._active:
            return
        request = params.get("request", {}) or {}
        if not self._match(request.get("url", ""), request.get("method", "")):
            return

        packet = CapturePacket(
            request=request,
            event_type="beforeRequestSent",
            timestamp=params.get("timestamp", 0),
            request_collector=self._request_collector,
            response_collector=self._response_collector,
            owner=self._owner,
        )
        self._store_pending(packet)

    def _on_response(self, params):
        if not self._active:
            return
        request = params.get("request", {}) or {}
        response = params.get("response", {}) or {}
        if not self._match(request.get("url", ""), request.get("method", "")):
            return

        request_id = request.get("request", "")
        packet = self._pop_pending(request_id)
        if packet is None:
            packet = CapturePacket(
                request=request,
                request_collector=self._request_collector,
                response_collector=self._response_collector,
                owner=self._owner,
            )
        elif request:
            packet.request = request
            packet.url = request.get("url", packet.url)
            packet.method = (request.get("method", packet.method) or "").upper()
            packet.request_headers = _headers_from_bidi(request.get("headers", []))

        packet._apply_response(
            response,
            event_type="responseCompleted",
            timestamp=params.get("timestamp", 0),
        )
        self._emit(packet)

    def _on_fetch_error(self, params):
        if not self._active:
            return
        request = params.get("request", {}) or {}
        if not self._match(request.get("url", ""), request.get("method", "")):
            return

        request_id = request.get("request", "")
        packet = self._pop_pending(request_id)
        if packet is None:
            packet = CapturePacket(
                request=request,
                request_collector=self._request_collector,
                response_collector=self._response_collector,
                owner=self._owner,
            )
        packet._apply_response(
            {},
            event_type="fetchError",
            timestamp=params.get("timestamp", 0),
        )
        self._emit(packet)
