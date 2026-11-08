import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))

from tools import android_prepare


class AndroidPrepareTests(unittest.TestCase):
    def test_download_game_uses_the_pinned_version_json_not_the_catalog_entry(self):
        version = '26.3'
        pins = json.loads((android_prepare.ROOT / 'research-lock.json').read_text())['minecraft']
        downloaded = []

        def fake_download(url, path, checksum):
            downloaded.append((url, checksum))
            path.parent.mkdir(parents=True, exist_ok=True)
            if path.name == f'{version}.json':
                path.write_text(json.dumps({
                    'id': version,
                    'downloads': {
                        'client': {'sha1': pins['client_sha1'], 'url': 'https://example.invalid/client.jar'},
                        'server': {'sha1': pins['server_sha1'], 'url': 'https://example.invalid/server.jar'},
                    },
                    'assetIndex': {'id': 'test', 'sha1': '0' * 40, 'url': 'https://example.invalid/index.json'},
                    'javaVersion': {'majorVersion': pins['java_major']},
                }))
            elif path.parent.name == 'indexes':
                path.write_text('{"objects": {}}')
            else:
                path.write_bytes(b'jar')
            return path

        catalog = {'versions': [{
            'id': version,
            'url': 'https://example.invalid/republished.json',
            'sha1': 'f' * 40,
        }]}
        with tempfile.TemporaryDirectory() as temporary, \
                mock.patch.object(android_prepare, 'fetch_json', return_value=catalog), \
                mock.patch.object(android_prepare.lab, 'download', side_effect=fake_download), \
                mock.patch.object(android_prepare.lab, 'client_library_artifacts', return_value=[]):
            android_prepare.download_game(version, Path(temporary))

        self.assertEqual(downloaded[0], (
            f"https://piston-meta.mojang.com/v1/packages/{pins['manifest_sha1']}/{version}.json",
            pins['manifest_sha1'],
        ))


if __name__ == '__main__':
    unittest.main()
