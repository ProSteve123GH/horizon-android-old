"""The APK workflow's versions.json/base announcement, without network access."""

import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    'mod_builds_announce', ROOT / 'packaging/android/mod_builds_announce.py'
)
announce = importlib.util.module_from_spec(spec)
spec.loader.exec_module(announce)

RELEASE = '152710c0bc674a46f5167f3d7ecd1dd3011bfa63'
DEV = '681538bf247ee6449c816b57bf784d43b1b1f0cd'


class VersionsTests(unittest.TestCase):
    def test_a_dev_build_is_added_without_losing_the_release_mapping(self):
        original = {'0.7.7': RELEASE}
        updated = announce.merged_versions(original, '0.7.7-dev.29.681538bf247e', DEV)
        self.assertEqual(updated['0.7.7'], RELEASE)
        self.assertEqual(updated['0.7.7-dev.29.681538bf247e'], DEV)
        self.assertEqual(original, {'0.7.7': RELEASE})

        text = announce.versions_text(updated)
        self.assertEqual(json.loads(text), updated)
        self.assertTrue(text.endswith('\n'))
        self.assertEqual(text, json.dumps(updated, indent=2, sort_keys=True) + '\n')


class BaseCacheTests(unittest.TestCase):
    def test_a_cached_base_is_not_dispatched_twice(self):
        payload = {'actions_caches': [{'key': f'base-{DEV}'}]}
        with patch.object(announce, 'api', return_value=(200, payload)) as request:
            self.assertTrue(announce.base_cached('owner/repo', DEV, 'token'))
        path = request.call_args.args[1]
        self.assertIn(f'key=base-{DEV}', path)

    def test_another_cached_key_does_not_count(self):
        payload = {'actions_caches': [{'key': f'base-{"0" * 40}'}]}
        with patch.object(announce, 'api', return_value=(200, payload)):
            self.assertFalse(announce.base_cached('owner/repo', DEV, 'token'))


if __name__ == '__main__':
    unittest.main()
