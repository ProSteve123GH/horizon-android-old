#!/usr/bin/env python3
"""Exercise screenshot discovery, metadata, allowlisting and deletion without Android."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def cargo_tests():
    harness = ROOT / '.local/android/screenshot-tests'
    (harness / 'src').mkdir(parents=True, exist_ok=True)
    (harness / 'Cargo.toml').write_text('''[package]
name = "android-screenshot-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "webp", "gif"] }
''', encoding='utf-8')
    (harness / 'src/lib.rs').write_text('''#![allow(dead_code)]
#[path = "../../../../crates/horizon-client/src/android/instances.rs"]
mod instances;
#[path = "../../../../crates/horizon-client/src/android/screenshots.rs"]
mod screenshots;
''', encoding='utf-8')
    subprocess.run(
        ['cargo', 'test', '--manifest-path', str(harness / 'Cargo.toml')],
        cwd=ROOT,
        check=True,
    )


def security_contract():
    manifest = (ROOT / 'packaging/android/AndroidManifest.xml').read_text(encoding='utf-8')
    provider = (
        '<provider android:name="dev.horizon.android.ScreenshotProvider" '
        'android:authorities="${applicationId}.screenshots" '
        'android:exported="false" android:grantUriPermissions="true"'
    )
    assert provider in manifest, 'ScreenshotProvider must be private with grantable URIs'

    kotlin = (
        ROOT
        / 'packaging/android/kotlin/dev/horizon/android/launcher/Screenshots.kt'
    ).read_text(encoding='utf-8')
    assert 'Intent.ACTION_SEND' in kotlin, 'sharing must use the Android share sheet'
    assert 'Intent.EXTRA_STREAM' in kotlin, 'sharing must attach the screenshot'
    assert 'Intent.FLAG_GRANT_READ_URI_PERMISSION' in kotlin, 'sharing needs a temporary read grant'
    assert 'ClipData.newRawUri' in kotlin, 'the URI grant must travel with the share intent'
    assert 'Uri.fromFile' not in kotlin, 'never share a raw file URI'
    assert 'EXTRA_TEXT' not in kotlin, 'never put a private path in shared text'
    share = kotlin.split('private fun shareScreenshot', 1)
    assert len(share) == 2, 'the share helper is missing'
    assert '.id' in share[1] and 'screenshotUri' in share[1], 'sharing must use the scoped gallery ID'
    assert 'ScreenshotGallery' in kotlin and 'AlertDialog' in kotlin
    assert 'DeleteScreenshotDialog' in kotlin, 'deletion needs a confirmation dialog'

    provider_java = (
        ROOT / 'packaging/android/java/dev/horizon/android/ScreenshotProvider.java'
    ).read_text(encoding='utf-8')
    assert 'openFile' in provider_java and 'nativeOpen' in provider_java
    assert 'Screenshots are read-only' in provider_java
    assert 'getAbsolutePath' in provider_java
    print('Screenshot provider/share security contract passed')


def main():
    cargo_tests()
    security_contract()


if __name__ == '__main__':
    main()
