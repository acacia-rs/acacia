"""Pixel diff of two BlueMap lowres tile sets: per LOD, pixels that differ and where (worst tiles first).

Usage: py -3 crates/acacia-anvil/tools/lowres_diff.py <golden map dir> <candidate map dir>  (…/web/maps/<id>)
Lowres pixels hold colour (top half) and height/light (bottom half), so a height-only change shows too.
"""
import sys
from pathlib import Path

from PIL import Image


def main(golden: str, candidate: str) -> None:
    g, c = Path(golden) / "tiles", Path(candidate) / "tiles"
    for lod in sorted(p.name for p in g.iterdir() if p.name != "0"):
        total = differing = 0
        worst = []
        for f in sorted((g / lod).rglob("*.png")):
            other = c / f.relative_to(g)
            a = Image.open(f).convert("RGBA")
            if not other.exists():
                worst.append((a.width * a.height, f.relative_to(g), "missing"))
                continue
            b = Image.open(other).convert("RGBA")
            n = sum(1 for p, q in zip(a.getdata(), b.getdata()) if p != q)
            total += a.width * a.height
            differing += n
            if n:
                worst.append((n, f.relative_to(g), ""))
        print(f"lod {lod}: {differing}/{total} pixels differ")
        for n, path, note in sorted(worst, reverse=True)[:5]:
            print(f"  {n:7} {path} {note}")


if __name__ == "__main__":
    main(*sys.argv[1:3])
