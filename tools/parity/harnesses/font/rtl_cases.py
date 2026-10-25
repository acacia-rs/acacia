"""Right-to-left text for the font cases: Hebrew, Arabic (letters, lam-alef, harakat, digits,
presentation forms), Latin, digits, neutrals, brackets, bidi controls, joiners, boundary neutrals,
newlines, legacy codes and a few other scripts mixed in."""

HEBREW = [chr(c) for c in range(0x05D0, 0x05EB)]
HEBREW_MARKS = [chr(c) for c in range(0x05B0, 0x05BE)] + ['ֿ', 'ׁ', 'ׂ', 'ׇ']
ARABIC = [chr(c) for c in list(range(0x0621, 0x063B)) + list(range(0x0641, 0x064B))]
ARABIC_EXTRA = [chr(c) for c in [0x0671, 0x067E, 0x0686, 0x0698, 0x06A9, 0x06AF, 0x06BA, 0x06BE, 0x06C1, 0x06CC, 0x06D2, 0x06D3, 0x0640]]
LAM_ALEF = ['ل' + a for a in 'آأإا']
HARAKAT = [chr(c) for c in range(0x064B, 0x0656)] + ['ٰ']
ARABIC_DIGITS = [chr(c) for c in range(0x0660, 0x066A)] + ['٫', '٬']
PERSIAN_DIGITS = [chr(c) for c in range(0x06F0, 0x06FA)]
ARABIC_PUNCT = ['،', '؛', '؟', '٪', '۔']
PRESENTATION = [chr(c) for c in list(range(0xFE70, 0xFEFD)) + list(range(0xFB50, 0xFBB2)) + list(range(0xFDF0, 0xFDFE))]
LATIN = [chr(c) for c in list(range(0x41, 0x5B)) + list(range(0x61, 0x7B))] + ['é', 'ß', 'Ж']
DIGITS = list('0123456789')
NEUTRALS = list(' .,:;!?-+=*/%$#&@_\'"~^|\\') + ['  ', '\t', ' ', ' ', '…', '°', '€']
BRACKETS = list('()[]{}<>') + ['«', '»', '‹', '›', '⁅', '⁆', '〈', '〉', '〈',
                               '〉', '∈', '≤', '≥', '√', '∕', '⧼', '⧽']
MARKS = ['؜', '‎', '‏']
EMBEDDINGS = ['‪', '‫', '‬', '‭', '‮']
ISOLATES = ['⁦', '⁧', '⁨', '⁩']
JOINERS = ['‌', '‍']
BOUNDARY = ['­', '​', '⁠', '﻿', '\u0001', '\u007F', '\U000E0001', '\U000E0041']
SEPARATORS = ['\n', '\n', '\r', '\r\n', ' ', '\u001C', '\u001F', '\u000B', '\u0085']
OTHERS = ['ܐ', 'ܒ', 'ހ', 'ߊ', 'ࠀ', '中', 'क', 'ก', '\U00010900', '\U0001E900',
          '\U0001D400', '\U0001F600', '￿', '�', '̀', '́', 'ि']

POOLS = [  # (weight, pool)
    (14, HEBREW), (3, HEBREW_MARKS), (14, ARABIC), (3, ARABIC_EXTRA), (4, LAM_ALEF), (4, HARAKAT),
    (3, ARABIC_DIGITS), (2, PERSIAN_DIGITS), (2, ARABIC_PUNCT), (3, PRESENTATION), (10, LATIN), (5, DIGITS),
    (10, NEUTRALS), (5, BRACKETS), (2, MARKS), (3, EMBEDDINGS), (3, ISOLATES), (2, JOINERS), (1, BOUNDARY),
    (2, SEPARATORS), (2, OTHERS),
]
WEIGHTS = [w for w, _ in POOLS]


def character(rng):
    pool = rng.choices(POOLS, WEIGHTS)[0][1]
    return rng.choice(pool)


def word(rng):
    """A run of one script, so letters join and digits group as in real text."""
    pool = rng.choices(POOLS, WEIGHTS)[0][1]
    return ''.join(rng.choice(pool) for _ in range(rng.randint(1, 6)))


def string(rng, codes):
    out = []
    for _ in range(rng.choice([1, 2, 3, 4, 6, 8, 12, 16])):
        r = rng.random()
        if r < 0.06:
            out.append('§' + rng.choice(codes))
        elif r < 0.5:
            out.append(word(rng))
        else:
            out.append(character(rng))
    return ''.join(out)


def component(rng, style, codes, depth=0):
    c = {'text': string(rng, codes), **style(rng)}
    if depth < 2 and rng.random() < 0.4:
        c['extra'] = [component(rng, style, codes, depth + 1) for _ in range(rng.randint(1, 3))]
    return c
