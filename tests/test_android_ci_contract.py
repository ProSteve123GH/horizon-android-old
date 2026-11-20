"""Machine-consumed CI/config contracts for the Android fork.

The two Android workflows under .github/workflows/ are exported from
packaging/android/ and compared with ``cmp`` in CI, so a byte-for-byte match is
a real contract rather than a formatting preference. The pinned action SHAs and
the manifest attributes are likewise security- and crash-handler-relevant
config values. These tests read actual YAML/XML config (step names, ``uses:``
pins, element attributes), never source comments.
"""

import re
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Active workflow -> the packaged export it must stay byte-identical to.
ACTIVE_WORKFLOWS = {
    "android-dev.yml": (
        ROOT / ".github/workflows/android-dev.yml",
        ROOT / "packaging/android/dev-builds.yml",
    ),
    "android-source.yml": (
        ROOT / ".github/workflows/android-source.yml",
        ROOT / "packaging/android/source-checks.yml",
    ),
}
MANIFEST = ROOT / "packaging/android/AndroidManifest.xml"
ANDROID_NS = "http://schemas.android.com/apk/res/android"

# Full SHAs of the Node-24 pinned actions used by these workflows.
NODE24_SHAS = {
    "checkout": "08c6903cd8c0fde910a37f88322edcfb5dd907a8",
    "setup-java": "b6effb05e454b25005698d916606bdc6ffcbf961",
    "cache": "caa296126883cff596d87d8935842f9db880ef25",
    "setup-gradle": "0723195856401067f7a2779048b490ace7a47d7c",
}
# Deprecated Node-20 action SHAs that must never come back.
NODE20_PREFIXES = ("11bd719", "cf277c6", "0057852", "ed40850")

USES_RE = re.compile(r"uses:\s*[^\s#]+@([0-9a-f]{40})\b")


class WorkflowMirrorTests(unittest.TestCase):
    def test_active_workflows_match_packaging_exports_byte_for_byte(self):
        for name, (active, packaged) in ACTIVE_WORKFLOWS.items():
            with self.subTest(workflow=name):
                self.assertEqual(active.read_bytes(), packaged.read_bytes())


class ActionPinTests(unittest.TestCase):
    def test_node24_action_shas_are_pinned(self):
        combined = "\n".join(active.read_text() for active, _ in ACTIVE_WORKFLOWS.values())
        for action, sha in NODE24_SHAS.items():
            with self.subTest(action=action):
                self.assertIn("@" + sha, combined)

    def test_every_action_reference_uses_a_node24_pin(self):
        for name, (active, _) in ACTIVE_WORKFLOWS.items():
            for sha in USES_RE.findall(active.read_text()):
                with self.subTest(workflow=name, sha=sha):
                    self.assertIn(sha, NODE24_SHAS.values())

    def test_no_node20_action_shas_anywhere(self):
        for name, (active, packaged) in ACTIVE_WORKFLOWS.items():
            for source, path in (("active", active), ("packaged", packaged)):
                text = path.read_text()
                for prefix in NODE20_PREFIXES:
                    with self.subTest(workflow=name, source=source, prefix=prefix):
                        self.assertNotIn(prefix, text)


class ManifestTests(unittest.TestCase):
    def test_manifest_does_not_set_extract_native_libs(self):
        for element in ET.parse(MANIFEST).getroot().iter():
            for attribute in element.attrib:
                with self.subTest(attribute=attribute):
                    self.assertNotIn("extractNativeLibs", attribute)

    def test_application_declares_the_crash_handler(self):
        app = ET.parse(MANIFEST).getroot().find("application")
        self.assertIsNotNone(app)
        self.assertEqual(
            app.get("{%s}name" % ANDROID_NS),
            "dev.horizon.android.launcher.HorizonApplication",
        )


class PipelineStepTests(unittest.TestCase):
    def test_dev_workflow_keeps_audit_build_and_release_upload_steps(self):
        text = ACTIVE_WORKFLOWS["android-dev.yml"][0].read_text()
        for marker in (
            "python3 tools/check_public_tree.py",
            "python3 tools/android_build.py",
            "gh release upload dev-latest --clobber",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, text)

    def test_source_workflow_still_enforces_android_tests_and_workflow_cmp(self):
        text = ACTIVE_WORKFLOWS["android-source.yml"][0].read_text()
        for marker in (
            "python3 -m unittest discover -s tests -p 'test_android*.py'",
            "cmp .github/workflows/android-source.yml packaging/android/source-checks.yml",
            "cmp .github/workflows/android-dev.yml packaging/android/dev-builds.yml",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, text)


if __name__ == "__main__":
    unittest.main()
