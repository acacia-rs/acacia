#!/usr/bin/env python3
"""Fetches the 26.3 asset index and its font objects (Unifont and the definitions naming it), which
the client jar lacks, into the launcher's layout: GUI_ASSETS_ROOT (default ~/gui-assets) gets
indexes/<id>.json and objects/<hh>/<hash>, every file SHA-1 checked. Nothing fetched is committed.

fetch.py [--version-json PATH]
"""
import argparse
import hashlib
import json
import os
import urllib.request

OBJECTS = 'https://resources.download.minecraft.net'
PREFIX = 'minecraft/font/'


def sha1(data):
    return hashlib.sha1(data).hexdigest()


def pinned(path, digest, url):
    if os.path.exists(path):
        with open(path, 'rb') as f:
            if sha1(f.read()) == digest:
                return
    with urllib.request.urlopen(url) as r:
        data = r.read()
    if sha1(data) != digest:
        raise SystemExit(f'{url}: got {sha1(data)}, expected {digest}')
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path + '.part', 'wb') as f:
        f.write(data)
    os.replace(path + '.part', path)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    home = os.path.expanduser('~')
    parser.add_argument('--version-json', default=os.path.join(os.environ.get('JAVA_TRUTH', f'{home}/java-truth'), 'versions/26.3/version.json'))
    args = parser.parse_args()
    root = os.environ.get('GUI_ASSETS_ROOT', f'{home}/gui-assets')
    with open(args.version_json) as f:
        index = json.load(f)['assetIndex']
    index_path = os.path.join(root, 'indexes', index['id'] + '.json')
    pinned(index_path, index['sha1'], index['url'])
    with open(index_path) as f:
        objects = json.load(f)['objects']
    fonts = {k: v for k, v in objects.items() if k.startswith(PREFIX)}
    for key, o in sorted(fonts.items()):
        h = o['hash']
        pinned(os.path.join(root, 'objects', h[:2], h), h, f'{OBJECTS}/{h[:2]}/{h}')
    print(f'index {index["id"]}: {len(fonts)} font objects in {root}/objects')


if __name__ == '__main__':
    main()
