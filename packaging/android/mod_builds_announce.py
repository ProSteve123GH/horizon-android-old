#!/usr/bin/env python3
"""Point a builds repository at one APK's commit and prepare its clean base cache.

The signed-APK workflow runs this after the artifact is built. It needs a fine-grained token
for the builds repository with **Contents: read and write** (``versions.json``) and
**Actions: read and write** (cache lookup and ``base`` dispatch). The workflow passes it as
``GH_TOKEN``; the token is never compiled into, or shipped with, the APK.

Why the APK's version name, not the game's base version: a dev APK is
``0.7.7-dev.<run>.<commit>``, and the launcher sends that label. Writing it here keeps the
source-export fallback (an APK built without ``.git``) on the same commit as the APK, while
the release's own ``0.7.7`` key is left alone by a dev build.
"""

import argparse
import base64
import json
import os
import sys
import urllib.error
import urllib.request

ROOT_API = os.environ.get('GITHUB_API_URL', 'https://api.github.com').rstrip('/')
DEFAULT_REPO = 'ProSteve123GH/horizon-mod-builds'
USER_AGENT = 'Horizon-Android-Build/1.0'


def api(method, path, token, payload=None, allow_404=False):
    """Call the GitHub REST API and return ``(status, decoded_json)``."""
    data = None if payload is None else json.dumps(payload).encode()
    request = urllib.request.Request(ROOT_API + path, data=data, method=method)
    request.add_header('Accept', 'application/vnd.github+json')
    request.add_header('X-GitHub-Api-Version', '2022-11-28')
    request.add_header('User-Agent', USER_AGENT)
    if token:
        request.add_header('Authorization', f'Bearer {token}')
    if data is not None:
        request.add_header('Content-Type', 'application/json')
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            body = response.read()
            return response.status, json.loads(body) if body else None
    except urllib.error.HTTPError as error:
        detail = error.read().decode(errors='replace')
        if allow_404 and error.code == 404:
            return 404, None
        raise SystemExit(f'GitHub {method} {path} failed: {error.code} {detail}') from error


def merged_versions(versions, version, commit):
    """Return a copy of the mapping with ``version`` pointing at ``commit``."""
    updated = dict(versions)
    updated[version] = commit
    return updated


def versions_text(versions):
    """The canonical on-disk form: sorted keys, two spaces, one trailing newline."""
    return json.dumps(versions, indent=2, sort_keys=True) + '\n'


def update_versions(repo, version, commit, token):
    """Upsert one key in the builds repository's ``versions.json``."""
    path = f'/repos/{repo}/contents/versions.json'
    status, payload = api('GET', path, token, allow_404=True)
    if status == 404:
        versions, sha = {}, None
    else:
        versions = json.loads(base64.b64decode(payload['content']))
        sha = payload['sha']
    if not isinstance(versions, dict):
        raise SystemExit(f'{repo}: versions.json is not an object')
    if versions.get(version) == commit:
        print(f'versions.json already maps {version} to {commit}')
        return False
    updated = merged_versions(versions, version, commit)
    body = {
        'message': f'versions.json: {version} -> {commit[:12]}',
        'content': base64.b64encode(versions_text(updated).encode()).decode(),
    }
    if sha:
        body['sha'] = sha
    api('PUT', path, token, body)
    print(f'versions.json: {version} -> {commit}')
    return True


def base_cached(repo, commit, token):
    """Whether the builds repository already has ``base-<commit>``."""
    key = f'base-{commit}'
    _, payload = api('GET', f'/repos/{repo}/actions/caches?key={key}&per_page=100', token)
    return any(cache.get('key') == key for cache in (payload or {}).get('actions_caches', []))


def dispatch_base(repo, version, commit, token, ref='main'):
    """Ask the builds repository to make the clean base cache for ``commit``."""
    path = f'/repos/{repo}/actions/workflows/base.yml/dispatches'
    api('POST', path, token, {'ref': ref, 'inputs': {'version': version, 'commit': commit}})
    print(f'dispatched base on {repo} for {commit} (version {version})')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        '--repo',
        default=os.environ.get('MOD_BUILDS_REPO') or DEFAULT_REPO,
        help=f'the builds repository (default: {DEFAULT_REPO})',
    )
    parser.add_argument('--version', required=True, help="the APK's version name")
    parser.add_argument('--commit', required=True, help='the full source commit')
    parser.add_argument('--ref', default='main', help='the base workflow branch (default: main)')
    args = parser.parse_args(argv)

    commit = args.commit.strip()
    if len(commit) != 40 or any(character not in '0123456789abcdef' for character in commit):
        raise SystemExit(f'not a full source commit: {args.commit!r}')
    token = os.environ.get('GH_TOKEN')
    if not token:
        raise SystemExit('GH_TOKEN is not set; set MOD_BUILDS_TOKEN in the workflow')

    update_versions(args.repo, args.version, commit, token)
    if base_cached(args.repo, commit, token):
        print(f'base-{commit} is already cached; not dispatching base')
        return 0
    dispatch_base(args.repo, args.version, commit, token, args.ref)
    return 0


if __name__ == '__main__':
    sys.exit(main())
