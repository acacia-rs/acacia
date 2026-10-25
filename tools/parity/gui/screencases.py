"""Shared case pieces for the screens_* GUI harnesses: GUI sizes and random text components.

Their cases.py run as scripts; they import this with tools/parity/gui on sys.path.
Keep the text ASCII: the probe's stand-in font only knows the ASCII page.
"""

KEYS = ['menu.returnToGame', 'gui.done', 'gui.cancel', 'options.title', 'menu.options', 'gui.advancements', 'gui.stats',
        'menu.disconnect', 'menu.returnToMenu', 'options.accessibility.title', 'options.fov', 'deathScreen.respawn',
        'selectWorld.edit', 'options.skinCustomisation.title', 'menu.shareToLan', 'gui.yes', 'gui.no',
        'options.accessibility.high_contrast', 'options.chat.title', 'controls.keybinds.title', 'options.online.title']
WORDS = ['alpha', 'beta', 'gamma', 'Delta', 'iron', 'X', 'wide', 'WWWWWW', 'il1', 'a', 'the', 'quick', 'brown', 'fox', '%']
COLORS = ['red', 'gold', 'aqua', '#336699', 'dark_gray', 'yellow']
WIDTHS = [427, 320, 480, 640, 961, 200]
HEIGHTS = [240, 255, 270, 360, 501, 150]


def words(rng, lo, hi):
    return ' '.join(rng.choice(WORDS) for _ in range(rng.randint(lo, hi)))


def message(rng, lo=1, hi=4):
    """A translatable, styled, list or plain-string component of `lo`..`hi` words."""
    r = rng.random()
    if r < 0.4:
        return {'translate': rng.choice(KEYS)}
    if r < 0.6:
        return {'text': words(rng, lo, hi), 'color': rng.choice(COLORS), 'bold': rng.random() < 0.4}
    if r < 0.7:
        return [{'text': words(rng, 1, 2) + ' '}, {'text': words(rng, 1, 2), 'italic': True, 'color': rng.choice(COLORS)}]
    return words(rng, lo, hi)


def size(rng):
    return {'width': rng.choice(WIDTHS), 'height': rng.choice(HEIGHTS)}


def mouse(rng, case):
    return [rng.randint(0, case['width']), rng.randint(0, case['height'])]
