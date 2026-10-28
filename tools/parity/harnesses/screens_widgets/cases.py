#!/usr/bin/env python3
"""Random widget cases, one generator per kind, drawn round-robin; one JSON case per line.

cases.py [--count N] [--kind K]... [--seed S]
Every case has the GUI size, the mouse (often over the widget), the widget's box and its
AbstractWidget state (active, visible, alpha, focused) and sometimes a scissor around it.
"""
import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'gui'))
from screencases import HEIGHTS, WIDTHS, message  # noqa: E402


def base(rng, w=None, h=None):
    width, height = rng.choice(WIDTHS), rng.choice(HEIGHTS)
    w = w if w is not None else rng.choice([20, 98, 150, 200, 204, 310, rng.randint(10, 300)])
    h = h if h is not None else rng.choice([20, 20, 20, 12, 24, rng.randint(8, 40)])
    x, y = rng.randint(-10, max(0, width - w)), rng.randint(-5, max(0, height - h))
    over = rng.random() < 0.6
    mouse = [rng.randint(x, x + max(0, w - 1)), rng.randint(y, y + max(0, h - 1))] if over else [rng.randint(0, width), rng.randint(0, height)]
    if rng.random() < 0.1:
        mouse = rng.choice([[x, y], [x + w, y], [x + w - 1, y + h - 1], [x - 1, y]])
    case = {'width': width, 'height': height, 'x': x, 'y': y, 'w': w, 'h': h, 'mouse': mouse,
            'active': rng.random() < 0.8, 'visible': rng.random() < 0.95, 'focused': rng.random() < 0.3,
            'alpha': rng.choice([1.0, 1.0, 1.0, 0.5, 0.0, round(rng.random(), 3)]), 'inWorld': rng.random() < 0.7}
    if rng.random() < 0.15:
        case['scissor'] = [rng.randint(0, width // 2), rng.randint(0, height // 2), rng.randint(width // 2, width), rng.randint(height // 2, height)]
    return case


def button(rng):
    return {**base(rng), 'message': message(rng, 1, 8), 'highlight': rng.choice([None, None, None, True, False])}


def image(rng):
    s = rng.choice([['widget/cross_button', 'widget/cross_button', 'widget/cross_button_highlighted', 'widget/cross_button_highlighted'],
                    ['widget/page_forward', 'widget/page_forward', 'widget/page_forward_highlighted', 'widget/page_forward_highlighted'],
                    ['widget/locked_button', 'widget/locked_button_disabled', 'widget/locked_button_highlighted', 'widget/locked_button_disabled']])
    return {**base(rng, rng.choice([20, 23, 14]), rng.choice([20, 13, 14])), 'message': message(rng), 'sprites': s}


def icon(rng):
    s = rng.choice(['pause_menu/bug', 'pause_menu/player_reporting', 'icon/accessibility', 'icon/language'])
    size = rng.choice([[15, 15], [16, 16], [12, 12]])
    return {**base(rng, rng.choice([20, 20, 150, 98]), 20), 'message': message(rng), 'sprites': [s, s, s, s], 'spriteSize': size,
            'spriteOffset': rng.choice([[0, 0], [0, 0], [1, -1], [-2, 0]]), 'iconOnly': rng.random() < 0.6, 'loading': rng.random() < 0.15}


def cycle(rng):
    values = [message(rng, 1, 2) for _ in range(rng.randint(1, 4))]
    return {**base(rng), 'name': message(rng, 1, 3), 'values': values, 'value': rng.randrange(len(values)),
            'display': rng.choice(['NAME_AND_VALUE', 'NAME_AND_VALUE', 'VALUE', 'HIDE']),
            'sprite': rng.choice([None, None, None, 'widget/button_highlighted'])}


def checkbox(rng):
    case = {**base(rng), 'message': message(rng, 1, 12), 'selected': rng.random() < 0.5}
    if rng.random() < 0.5:
        case['maxWidth'] = rng.randint(20, 200)
    return case


def slider(rng):
    return {**base(rng), 'message': message(rng, 1, 6), 'value': rng.choice([0.0, 1.0, round(rng.random(), 4)]), 'canChange': rng.random() < 0.5}


def editbox(rng):
    case = {**base(rng, rng.choice([150, 200, 60, 98, 310]), rng.choice([20, 20, 12, 18])), 'message': message(rng)}
    value = ''.join(rng.choice('abcdefghijk lmnoWXYZ0123:._') for _ in range(rng.choice([0, 0, 3, 10, 30, 60, 100])))
    case.update(maxLength=rng.choice([32, 32, 256, 16, 5]), bordered=rng.random() < 0.8, editable=rng.random() < 0.85,
                centered=rng.random() < 0.1, shadow=rng.random() < 0.8, invert=rng.random() < 0.7, value=value,
                focusAgo=rng.randrange(0, 5) * 300 + rng.randint(80, 220))
    n = min(len(value), case['maxLength'])
    case['ops'] = [['cursor', rng.randint(0, n), rng.random() < 0.3] if rng.random() < 0.7 else ['highlight', rng.randint(0, n)]
                   for _ in range(rng.choice([0, 1, 2, 4]))]
    if rng.random() < 0.3:
        case['suggestion'] = rng.choice(['ing', ' suggestion', 'x'])
    if rng.random() < 0.3:
        case['hint'] = rng.choice([{'translate': 'gui.socialInteractions.search_hint'}, 'Search...', {'text': 'Hint', 'color': 'aqua'}])
    if rng.random() < 0.1:
        case['textColor'] = rng.choice([-1, -2039584, -6250336])
    return case


def string(rng):
    case = {**base(rng, rng.randint(0, 200), rng.choice([9, 20, 12])), 'message': message(rng, 1, 10)}
    if rng.random() < 0.5:
        case.update(maxWidth=rng.randint(10, 200), overflow=rng.choice(['CLAMPED', 'SCROLLING']))
    return case


def multiline(rng):
    case = {**base(rng, 0, 0), 'message': message(rng, 3, 30), 'centered': rng.random() < 0.5}
    if rng.random() < 0.8:
        case['maxWidth'] = rng.randint(20, 250)
    if rng.random() < 0.4:
        case['maxRows'] = rng.randint(1, 4)
    return case


def focusable(rng):
    return {**base(rng, 0, 0), 'message': message(rng, 2, 25), 'padding': rng.choice([4, 4, 2, 0, 8]),
            'maxWidth': rng.choice([-1, -1, rng.randint(40, 300)]), 'alwaysBorder': rng.random() < 0.5,
            'fill': rng.choice(['ALWAYS', 'ON_FOCUS', 'NEVER'])}


def tabs(rng):
    case = base(rng)
    n = rng.randint(1, 5)
    case.update(titles=[message(rng, 1, 2) for _ in range(n)], selected=rng.randrange(n))
    case['mouse'] = [rng.randint(0, case['width']), rng.choice([rng.randint(0, 30), rng.randint(0, case['height'])])]
    return case


def layout_child(rng, depth):
    r = rng.random()
    if depth < 2 and r < 0.15:
        child = {'layout': layout_node(rng, depth + 1)}
    elif r < 0.25:
        child = {'spacer': [rng.randint(0, 40), rng.randint(0, 30)]}
    else:
        child = {'button': [rng.choice([20, 98, 150, 204, rng.randint(10, 120)]), rng.choice([20, 20, 15, rng.randint(5, 40)])]}
    if rng.random() < 0.5:
        child['pad'] = [rng.choice([0, 0, 4, rng.randint(0, 12)]) for _ in range(4)]
        child['align'] = [rng.choice([0.0, 0.5, 1.0, round(rng.random(), 3)]) for _ in range(2)]
    return child


def layout_node(rng, depth=0):
    kind = rng.choice(['grid', 'helper', 'linear', 'frame', 'equal'])
    children = [layout_child(rng, depth) for _ in range(rng.randint(1, 6))]
    node = {'type': kind, 'children': children}
    if kind == 'grid':
        node.update(rowSpacing=rng.randint(0, 8), colSpacing=rng.randint(0, 8))
        for c in children:
            c.update(row=rng.randint(0, 3), col=rng.randint(0, 3), rows=rng.choice([1, 1, 2]), cols=rng.choice([1, 1, 2, 3]))
    elif kind == 'helper':
        node.update(rowSpacing=rng.randint(0, 8), colSpacing=rng.randint(0, 8), columns=rng.randint(1, 3))
        for c in children:
            c['span'] = rng.randint(1, node['columns'])
    elif kind == 'linear':
        node.update(vertical=rng.random() < 0.5, spacing=rng.choice([0, 4, 8, 12]))
    elif kind == 'frame':
        node.update(minW=rng.choice([0, 0, 100, 300]), minH=rng.choice([0, 0, 50, 200]))
    else:
        node.update(w=rng.randint(0, 400), h=rng.randint(0, 200), vertical=rng.random() < 0.5)
    return node


def layout(rng):
    case = base(rng)
    case['layout'] = layout_node(rng)
    case['rect'] = [rng.choice([0, 0, 10]), rng.choice([0, 0, 33]), case['width'], case['height'],
                    rng.choice([0.5, 0.5, 0.0, 1.0]), rng.choice([0.25, 0.5, 0.0, 1.0])]
    case['mouse'] = [rng.randint(0, case['width']), rng.randint(0, case['height'])]
    return case


def selection_list(rng):
    case = base(rng)
    width, height = case['width'], case['height']
    top = rng.choice([33, 32, 48, 20])
    rows = [rng.choice([20, 20, 36, 25, rng.randint(10, 40)]) for _ in range(rng.choice([0, 1, 3, 8, 20, 40]))]
    case.update(rect=[rng.choice([0, 0, 20]), top, width - rng.choice([0, 0, 40]), max(10, height - top - rng.choice([33, 64, 40]))],
                rows=rows, rowWidth=rng.choice([220, 310, 200, 150]), scroll=rng.choice([0.0, 0.0, 10.0, 25.5, 1e9, rng.random() * 400]),
                selected=rng.randrange(len(rows)) if rows and rng.random() < 0.6 else -1)
    case['mouse'] = [rng.randint(0, width), rng.randint(0, height)]
    return case


KINDS = dict(button=button, image=image, icon=icon, cycle=cycle, checkbox=checkbox, slider=slider, editbox=editbox,
             string=string, multiline=multiline, focusable=focusable, tabs=tabs, layout=layout, list=selection_list)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--count', type=int, default=2600)
    p.add_argument('--kind', action='append')
    p.add_argument('--seed', type=int, default=1)
    a = p.parse_args()
    rng = random.Random(a.seed)
    kinds = a.kind or list(KINDS)
    for n in range(a.count):
        kind = kinds[n % len(kinds)]
        print(json.dumps({'kind': kind, **KINDS[kind](rng)}))


if __name__ == '__main__':
    main()
