"""The game activity's launch contract, which winit's one-loop-per-process rule needs."""

from pathlib import Path
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
ANDROID = '{http://schemas.android.com/apk/res/android}'


class GameActivityLaunchTests(unittest.TestCase):
    def test_play_reuses_a_game_that_is_still_alive(self):
        manifest = ET.parse(ROOT / 'packaging/android/AndroidManifest.xml')
        activities = {
            activity.get(f'{ANDROID}name'): activity
            for activity in manifest.getroot().iter('activity')
        }
        game = activities['dev.horizon.android.HorizonActivity']
        self.assertEqual(game.get(f'{ANDROID}process'), ':game')
        self.assertEqual(
            game.get(f'{ANDROID}launchMode'),
            'singleTask',
            'a second HorizonActivity would ask winit for a second EventLoop in the cached :game process',
        )


if __name__ == '__main__':
    unittest.main()
