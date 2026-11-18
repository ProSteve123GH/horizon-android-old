"""Behaviour tests for the Kotlin launcher crash-report format.

The reference implementation below mirrors the format/redaction contract of
CrashHandler.kt in pure Python; these tests never read the Kotlin source as an
assertion. Expected values are written by hand and independent of the helpers.
"""

import re
import tempfile
import unittest
from pathlib import Path

SECRET_RE = re.compile(r"token|password|authorization|session", re.I)
PATH_RE = re.compile(r"/(?:data|storage|sdcard)/[^\s)]*")
URL_RE = re.compile(r"[a-z][a-z0-9+.-]*://[^\s)]*")
IP_RE = re.compile(r"\b\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?\b")

MAX_STACK_LINES = 64
MAX_STACK_BYTES = 8192
MAX_FIELD_CHARS = 2048
MAX_REPORTS = 4
NOTICE = "A previous launch crashed. Diagnostics are available in Settings."


def field(value, limit=MAX_FIELD_CHARS):
    """Bound a single metadata field and keep it on one line."""
    if not value:
        return ""
    flat = value.replace("\n", " ").replace("\r", " ")
    if len(flat) > limit:
        return flat[:limit] + "…(truncated)"
    return flat


def redact_line(line):
    """None means the whole line is dropped; otherwise the scrubbed line."""
    if SECRET_RE.search(line.lower()):
        return None
    out = PATH_RE.sub("[redacted]", line)
    out = URL_RE.sub("[redacted]", out)
    out = IP_RE.sub("[redacted]", out)
    return out


def format_report(*, thread, error_class, message, version, sdk, abi, utc_ms, stack):
    """Reference formatter: metadata + bounded, redacted stack."""
    lines = []

    def add(raw):
        safe = redact_line(raw)
        if safe is not None:
            lines.append(safe)

    add("Horizon Android Kotlin launcher crash report")
    add("utc_ms: {}".format(utc_ms))
    add("thread: {}".format(field(thread)))
    add("throwable: {}".format(field(error_class)))
    add("message: {}".format(field(message)))
    add("version: {}".format(field(version)))
    add("sdk: {}".format(sdk))
    add("abi: {}".format(field(abi)))
    add("stack:")
    used = 0
    count = 0
    for raw in stack.split("\n"):
        if count >= MAX_STACK_LINES:
            break
        safe = redact_line(raw)
        if safe is None:
            continue
        if used + len(safe) + 1 > MAX_STACK_BYTES:
            break
        lines.append(safe)
        used += len(safe) + 1
        count += 1
    return "\n".join(lines) + "\n"


def rotate(existing_names, new_name, max_files=MAX_REPORTS):
    """Return the report names kept after adding new_name (oldest dropped)."""
    names = sorted(existing_names + [new_name])
    keep = set(names[-max_files:])
    return [name for name in names if name in keep]


class MarkerStore:
    """Fixtures for the pending-notice marker (pendingReport/clear contract)."""

    def __init__(self, root):
        self.marker = Path(root) / "kotlin-crash-pending.marker"

    def pending_report(self):
        return NOTICE if self.marker.is_file() else None

    def clear(self):
        if self.marker.exists():
            self.marker.unlink()

    def record_crash(self):
        self.marker.write_text("1")


class RedactionTests(unittest.TestCase):
    def test_secret_bearing_lines_are_dropped(self):
        samples = [
            "auth token=abc123",
            "PASSWORD=hunter2",
            "Authorization: Bearer xyz",
            "expired session for user",
        ]
        for line in samples:
            with self.subTest(line=line):
                self.assertIsNone(redact_line(line))

    def test_paths_urls_and_ips_are_scrubbed(self):
        cases = {
            "open /data/data/dev.horizon.android/files/blob.bin failed":
                "open [redacted] failed",
            "root /storage/emulated/0/Download/x":
                "root [redacted]",
            "cache /sdcard/horizon/cache":
                "cache [redacted]",
            "see https://example.com/path?q=1 for details":
                "see [redacted] for details",
            "opened content://com.android.providers/downloads/123":
                "opened [redacted]",
            "peer 192.168.1.50:8080 closed":
                "peer [redacted] closed",
        }
        for raw, expected in cases.items():
            with self.subTest(raw=raw):
                self.assertEqual(redact_line(raw), expected)

    def test_report_never_retains_sensitive_content(self):
        report = format_report(
            thread="main",
            error_class="java.lang.IllegalStateException",
            message="bad auth token for /data/data/dev.horizon.android/files/blob.bin",
            version="0.7.7",
            sdk=36,
            abi="arm64-v8a",
            utc_ms=1763558400000,
            stack=(
                "java.lang.IllegalStateException: boom\n"
                "  at dev.horizon.App.start(App.kt:42)\n"
                "Caused by: java.net.ConnectException: 10.0.0.7:9000\n"
            ),
        )
        self.assertIn("utc_ms: 1763558400000", report)
        self.assertIn("  at dev.horizon.App.start(App.kt:42)", report)
        for leaked in ("token", "/data/", "10.0.0.7", "Authorization"):
            with self.subTest(leaked=leaked):
                self.assertNotIn(leaked, report)


class BoundTests(unittest.TestCase):
    def test_stack_is_bounded(self):
        stack = "\n".join("  at pkg.Frame{}".format(i) for i in range(200))
        report = format_report(
            thread="main", error_class="E", message="m", version="v",
            sdk=28, abi="arm64-v8a", utc_ms=1, stack=stack,
        )
        frames = [l for l in report.split("\n") if l.startswith("  at pkg.Frame")]
        self.assertEqual(len(frames), MAX_STACK_LINES)
        self.assertIn("  at pkg.Frame0", frames)
        self.assertNotIn("  at pkg.Frame199", report)
        self.assertLessEqual(len(report.encode("utf-8")), MAX_STACK_BYTES + 512)

    def test_long_single_line_is_bounded(self):
        report = format_report(
            thread="main", error_class="E", message="x" * 20000, version="v",
            sdk=28, abi="x", utc_ms=1, stack="  at A\n" + "z" * 20000,
        )
        for line in report.split("\n"):
            self.assertLessEqual(len(line), MAX_STACK_BYTES)


class RotationTests(unittest.TestCase):
    def test_keeps_four_newest_reports(self):
        existing = [
            "kotlin-crash-1000.txt",
            "kotlin-crash-2000.txt",
            "kotlin-crash-3000.txt",
            "kotlin-crash-4000.txt",
        ]
        self.assertEqual(
            rotate(existing, "kotlin-crash-5000.txt"),
            [
                "kotlin-crash-2000.txt",
                "kotlin-crash-3000.txt",
                "kotlin-crash-4000.txt",
                "kotlin-crash-5000.txt",
            ],
        )

    def test_fewer_than_four_reports_are_all_kept(self):
        self.assertEqual(
            rotate(["kotlin-crash-1000.txt"], "kotlin-crash-2000.txt"),
            ["kotlin-crash-1000.txt", "kotlin-crash-2000.txt"],
        )


class NoticeTests(unittest.TestCase):
    def test_notice_is_consumed_once(self):
        with tempfile.TemporaryDirectory() as root:
            store = MarkerStore(root)
            self.assertIsNone(store.pending_report())
            store.record_crash()
            self.assertEqual(store.pending_report(), NOTICE)
            store.clear()
            self.assertIsNone(store.pending_report())
            store.clear()
            self.assertIsNone(store.pending_report())


if __name__ == "__main__":
    unittest.main()
