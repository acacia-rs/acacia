#!/usr/bin/env python3
"""Random options-screen cases: a page, random values for every option, the list scrolled; one JSON case per line.

cases.py [--count N] [--kind K]... [--seed S]
`values` maps an Options getter's name (`sound.<source>` for volumes, `modelPart.<part>` for the
skin layers) to its value; booleans, ints, doubles (always written with a point) or enum names.
"""
import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'gui'))
from screencases import mouse, size  # noqa: E402

BOOLS = ['darkMojangStudiosBackground', 'hideLightningFlash', 'hideSplashTexts', 'cutoutLeaves', 'vignette', 'improvedTransparency',
         'ambientOcclusion', 'highContrast', 'highContrastBlockOutline', 'allowCursorChanges', 'quitShortcuts', 'narratorHotkey', 'autoJump',
         'rotateWithMinecart', 'operatorItemsTab', 'autoSuggestions', 'chatColors', 'chatLinks', 'chatLinksPrompt', 'enableVsync',
         'entityShadows', 'forceUnicodeFont', 'japaneseGlyphVariants', 'invertMouseX', 'invertMouseY', 'discreteMouseScroll',
         'realmsNotifications', 'allowServerListing', 'reducedDebugInfo', 'inGameNotification', 'showSubtitles', 'directionalAudio',
         'backgroundForChatOnly', 'fullscreen', 'exclusiveFullscreen', 'bobView', 'toggleCrouch', 'toggleSprint', 'toggleAttack',
         'toggleUse', 'hideMatchedNames', 'showAutosaveIndicator', 'onlyShowSecureChat', 'saveChatDrafts']
BOOLS += ['modelPart.' + p for p in ['cape', 'jacket', 'left_sleeve', 'right_sleeve', 'left_pants_leg', 'right_pants_leg', 'hat']]
ENUMS = {
    'preferredGraphicsBackend': ['DEFAULT', 'DEFAULT', 'OPENGL', 'VULKAN'], 'graphicsPreset': ['FAST', 'FANCY', 'FABULOUS', 'CUSTOM'],
    'inactivityFpsLimit': ['MINIMIZED', 'AFK'], 'cloudStatus': ['OFF', 'FAST', 'FANCY'], 'prioritizeChunkUpdates': ['NONE', 'PLAYER_AFFECTED', 'NEARBY'],
    'chatVisibility': ['FULL', 'SYSTEM', 'HIDDEN'], 'mainHand': ['LEFT', 'RIGHT'], 'textureFiltering': ['NONE', 'RGSS', 'ANISOTROPIC'],
    'attackIndicator': ['OFF', 'CROSSHAIR', 'HOTBAR'], 'particles': ['ALL', 'DECREASED', 'MINIMAL'], 'musicFrequency': ['DEFAULT', 'FREQUENT', 'CONSTANT'],
    'musicToast': ['NEVER', 'PAUSE', 'PAUSE_AND_TOAST'], 'sharePresence': ['NONE', 'LIMITED', 'ALL'],
    'narrator': ['OFF', 'ALL', 'CHAT', 'SYSTEM'], 'soundDevice': ['','OpenAL Soft on Speakers', 'Headphones'],
}
INTS = {'renderDistance': (2, 32), 'simulationDistance': (5, 32), 'cloudRange': (2, 128), 'weatherRadius': (3, 10), 'menuBackgroundBlurriness': (0, 10),
        'mipmapLevels': (0, 4), 'maxAnisotropyBit': (1, 3), 'biomeBlendRadius': (0, 7), 'sprintWindow': (0, 10), 'fov': (30, 110), 'guiScale': (0, 6)}
UNITS = ['sensitivity', 'chatOpacity', 'chatLineSpacing', 'textBackgroundOpacity', 'panoramaSpeed', 'chatScale', 'chatWidth', 'chatHeightUnfocused',
         'chatHeightFocused', 'screenEffectScale', 'fovEffectScale', 'darknessEffectScale', 'glintSpeed', 'glintStrength', 'damageTiltStrength', 'gamma']
UNITS += ['sound.' + s for s in ['master', 'music', 'record', 'weather', 'block', 'hostile', 'neutral', 'player', 'ambient', 'voice', 'ui']]
# IntRange.xmap options: the int range and the value it maps to.
MAPPED = {'entityDistanceScaling': (2, 20, lambda k: k / 4.0), 'chunkSectionFadeInTime': (0, 40, lambda k: k / 20.0),
          'chatDelay': (0, 60, lambda k: k / 10.0), 'notificationDisplayTime': (5, 100, lambda k: k / 10.0),
          'framerateLimit': (1, 26, lambda k: k * 10), 'mouseWheelSensitivity': (-200, 100, lambda k: 10.0 ** (k / 100.0))}
PAGES = ['options', 'video', 'sound', 'controls', 'mouse', 'chat', 'accessibility', 'skin', 'font', 'online']


def int_value(rng, lo, hi):
    return rng.choice([lo, hi, rng.randint(lo, hi), rng.randint(lo, hi)])


def values(rng):
    v = {k: rng.random() < 0.5 for k in BOOLS}
    v.update({k: rng.choice(names) for k, names in ENUMS.items()})
    v.update({k: int_value(rng, lo, hi) for k, (lo, hi) in INTS.items()})
    v.update({k: float(rng.choice([0.0, 1.0, 0.5, rng.random(), round(rng.random(), 2)])) for k in UNITS})
    v.update({k: to(int_value(rng, lo, hi)) for k, (lo, hi, to) in MAPPED.items()})
    return v


def case(rng, page):
    c = {'kind': page, **size(rng), 'values': values(rng), 'telemetry': rng.random() < 0.5,
         'highContrastPack': rng.random() < 0.5, 'minecart': rng.random() < 0.3, 'friends': rng.random() < 0.5,
         'friendRequests': rng.random() < 0.5, 'tick': rng.random() < 0.5, 'scroll': rng.choice([0, 0, 10, 60, 200, 1000])}
    c['mouse'] = mouse(rng, c)
    if rng.random() < 0.6:
        c['mouse'] = [c['width'] // 2 + rng.randint(-160, 160), rng.randint(0, c['height'])]
    return c


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--count', type=int, default=2000)
    p.add_argument('--kind', action='append')
    p.add_argument('--seed', type=int, default=1)
    a = p.parse_args()
    rng = random.Random(a.seed)
    kinds = a.kind or PAGES
    for n in range(a.count):
        print(json.dumps(case(rng, kinds[n % len(kinds)])))


if __name__ == '__main__':
    main()
