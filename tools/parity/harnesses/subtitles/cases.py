#!/usr/bin/env python3
"""Subtitle scripts: clock changes, sounds played (subtitle key, position, range, looping) and frames
(listener transform, options, GUI size). One JSON case per frame: the script up to and including
that frame, whose draws are compared.

cases.py [--count N]   N scripts (Horizon's test_subtitle_parity.py, MIT, (c) Ludovic Marie)
"""
import argparse
import json
import math
import os
from pathlib import Path
import random

ASSETS = Path(os.environ.get('GUI_ASSETS', Path.home() / 'gui-assets' / 'assets' / 'minecraft'))


def subtitle_keys():
    lang = json.loads((ASSETS / 'lang' / 'en_us.json').read_text(encoding='utf-8'))
    return sorted(k for k in lang if k.startswith('subtitles.'))


def listener(rng, pos):
    yaw, pitch = math.radians(rng.uniform(-180, 180)), math.radians(rng.uniform(-90, 90))
    forward = [-math.sin(yaw) * math.cos(pitch), -math.sin(pitch), math.cos(yaw) * math.cos(pitch)]
    up = [-math.sin(yaw) * -math.sin(pitch), math.cos(pitch), math.cos(yaw) * -math.sin(pitch)]
    if rng.random() < 0.2:
        forward, up = rng.choice([([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]), ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
                                  ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0])])
    return {'pos': pos, 'forward': forward, 'up': up}


def script(rng, keys):
    pool = rng.sample(keys, rng.choice([1, 3, 8, 20])) + ['subtitles.acacia.unknown']
    t = rng.randint(0, 10_000_000)
    pos = [rng.uniform(-100, 100), rng.uniform(-60, 300), rng.uniform(-100, 100)]
    here = listener(rng, pos)
    mult = rng.choice([0.5, 1.0, 1.0, 1.5, 2.0, 3.5, 10.0])
    show = rng.random() < 0.9
    opacity, chat_only = rng.choice([0.5, 0.0, 1.0, 0.25]), rng.random() < 0.6
    size = rng.choice([(427, 240), (320, 240), (480, 270), (961, 501)])
    ops = [{'op': 'time', 'ms': t, 'sub': rng.randint(0, 999999)}]
    locations = [[pos[i] + rng.uniform(-30, 30) for i in range(3)] for _ in range(6)]

    def frame():
        nonlocal here
        if rng.random() < 0.3:
            here = listener(rng, [p + rng.uniform(-2, 2) for p in here['pos']])
        return {'op': 'frame', **here, 'show': show, 'mult': mult, 'opacity': opacity, 'chatOnly': chat_only,
                'w': size[0], 'h': size[1]}
    ops.append(frame())
    for _ in range(rng.choice([5, 20, 60])):
        r = rng.random()
        if r < 0.45:
            at = rng.choice(locations) if rng.random() < 0.6 else [here['pos'][i] + rng.uniform(-20, 20) for i in range(3)]
            if rng.random() < 0.05:
                at = list(here['pos'])
            key = rng.choice(pool) if rng.random() < 0.9 else None
            ops.append({'op': 'play', 'key': key, 'at': at, 'range': rng.choice([16.0, 16.0, 8.0, 48.0, 2.5, 100.0, None]),
                        'looping': rng.random() < 0.15})
        elif r < 0.75:
            t += rng.choice([0, 1, 16, 50, 499, 500, 501, 1000, 2999, 3000, 3001, rng.randint(0, 8000)])
            ops.append({'op': 'time', 'ms': t, 'sub': rng.randint(0, 999999)})
        else:
            if rng.random() < 0.08:
                show = not show
            if rng.random() < 0.05:
                mult = rng.choice([0.5, 1.0, 2.0, 10.0])
            ops.append(frame())
    ops.append(frame())
    return ops


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--count', type=int, default=1500)
    args = parser.parse_args()
    rng = random.Random(0x5355425449544c45)
    keys = subtitle_keys()
    for n in range(args.count):
        ops = script(rng, keys)
        for i, op in enumerate(ops):
            if op['op'] == 'frame':
                print(json.dumps({'kind': 'subtitles', 'script': n, 'ops': ops[:i + 1]}, ensure_ascii=False))


if __name__ == '__main__':
    main()
