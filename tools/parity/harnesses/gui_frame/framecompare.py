"""The gui_frame comparison: FrameOracle's GuiRenderer draws turned into the quads they put on
screen (tiles split, glyphs with their shadow and bold copies, the pose applied, cut to the
scissor as acacia cuts them) and matched against gui-probe's painted quads (frame.rs's records).

A case matches when both lists agree in order or, failing that, hold the same quads with every
overlapping pair in the same order: GuiRenderer's sort reorders elements of one node that don't
overlap, which shows nowhere on screen."""
import json
import math

TEXTURED = {"pipeline/gui": "alpha", "pipeline/gui_textured": "alpha", "pipeline/gui_textured_premultiplied_alpha": "alpha",
            "pipeline/gui_invert": "invert", "pipeline/crosshair": "invert", "pipeline/vignette": "darken",
            "pipeline/gui_nausea_overlay": "add", "pipeline/gui_text_highlight": "add"}


def blend(pipeline):
    path = pipeline.split(":", 1)[-1]
    return TEXTURED.get(path, "?" + path)


def u32(c):
    return c & 0xFFFFFFFF


def texture(name):
    """`minecraft:textures/atlas/gui.png#minecraft:widget/button` -> `sprite:widget/button`."""
    if "#" in name:
        return "sprite:" + name.split("#", 1)[1].split(":", 1)[-1]
    path = name.split(":", 1)[-1]
    if path.startswith("textures/atlas/"):
        return "atlas"
    return "tex:" + path.removeprefix("textures/").removesuffix(".png")


def transform(m, x, y):
    return m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]


def lerp_color(a, b, t):
    out = 0
    for shift in (24, 16, 8, 0):
        ca, cb = (a >> shift) & 255, (b >> shift) & 255
        out |= int(math.floor(ca + (cb - ca) * t + 0.5)) << shift
    return out


def window_cut(scissor, window):
    """The scissor as the GPU applies it: inside the framebuffer (a scrolling label's scissor is not cut to the screen)."""
    if not scissor:
        return None
    x, y = scissor["position"]["x"], scissor["position"]["y"]
    l, t = max(x, 0), max(y, 0)
    r, b = min(x + scissor["width"], window[0]), min(y + scissor["height"], window[1])
    return {"position": {"x": l, "y": t}, "width": max(r - l, 0), "height": max(b - t, 0)}


def place(q, pose, scissor):
    """`q` through `pose` and cut to `scissor` as paint::place_quad and paint::cut do; None when cut away."""
    l, t, r, b = q["r"]
    (x0, y0), (x1, y1) = transform(pose, l, t), transform(pose, r, b)
    uv = q.get("uv")
    if uv:
        ul, vt, ur, vb = uv
        if x1 < x0:
            ul, ur = ur, ul
        if y1 < y0:
            vt, vb = vb, vt
        uv = [ul, vt, ur, vb]
    rect = [min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)]
    if "s" in q:
        q["s"] = [v * pose[0] for v in q["s"]]
    if scissor:
        s = [scissor["position"]["x"], scissor["position"]["y"]]
        s += [s[0] + scissor["width"], s[1] + scissor["height"]]
        l, t, r, b = rect
        nl, nt, nr, nb = max(l, s[0]), max(t, s[1]), min(r, s[2]), min(b, s[3])
        if nl >= nr or nt >= nb:
            return None
        frac = lambda f, to, at: 0.0 if to == f else (at - f) / (to - f)  # noqa: E731
        if uv:
            ul, vt, ur, vb = uv
            uv = [ul + (ur - ul) * frac(l, r, nl), vt + (vb - vt) * frac(t, b, nt), ul + (ur - ul) * frac(l, r, nr), vt + (vb - vt) * frac(t, b, nb)]
        if q["k"] == "fill":
            top, bottom = q["c"]
            q["c"] = [lerp_color(top, bottom, frac(t, b, nt)), lerp_color(top, bottom, frac(t, b, nb))]
        rect = [nl, nt, nr, nb]
    q["r"] = rect
    if uv:
        q["uv"] = uv
    return q


def tiles(d):
    w, h, tw, th = d["x1"] - d["x0"], d["y1"] - d["y0"], d["tileWidth"], d["tileHeight"]
    u0, u1, v0, v1 = d["u0"], d["u1"], d["v0"], d["v1"]
    for tx in range(0, w, tw):
        cw, cu = (tw, u1) if tw <= w - tx else (w - tx, u0 + (w - tx) / tw * (u1 - u0))
        for ty in range(0, h, th):
            ch, cv = (th, v1) if th <= h - ty else (h - ty, v0 + (h - ty) / th * (v1 - v0))
            yield [d["x0"] + tx, d["y0"] + ty, d["x0"] + tx + cw, d["y0"] + ty + ch], [u0, v0, cu, cv]


def glyph_quads(g):
    """BakedSheetGlyph.renderChar / renderEffect: shadow (and its bold copy) first, then the glyph."""
    shadow = g["shadowColor"] != 0
    if "x0" in g:
        o = g["shadowOffset"]
        rects = [(o, g["shadowColor"])] if shadow else []
        return [([g["x0"] + d, g["y1"] + d, g["x1"] + d, g["y0"] + d], c, [0.0, 0.0]) for d, c in rects + [(0.0, g["color"])]]
    style, info = g.get("style") or {}, g["glyph"]
    bold, italic = bool(style.get("bold")), bool(style.get("italic"))
    ex = 0.1 if bold else 0.0
    skew = [1.0 - 0.25 * info["up"], 1.0 - 0.25 * info["down"]] if italic else [0.0, 0.0]
    copies = []
    if shadow:
        so = g["shadowOffset"]
        copies.append((so, so, g["shadowColor"]))
        if bold:
            copies.append((g["boldOffset"] + so, so, g["shadowColor"]))
    copies.append((0.0, 0.0, g["color"]))
    if bold:
        copies.append((g["boldOffset"], 0.0, g["color"]))
    return [([g["x"] + dx + info["left"] - ex, g["y"] + dy + info["up"] - ex, g["x"] + dx + info["right"] + ex, g["y"] + dy + info["down"] + ex], c, skew)
            for dx, dy, c in copies]


def java_quads(draws, window):
    out = []
    for d in draws:
        ab, pose, scissor = d["afterBlur"], d["pose"], window_cut(d.get("scissorArea"), window)
        kind = d["kind"]
        if kind == "GlyphRenderState":
            for rect, c, skew in glyph_quads(d["renderable"]):
                l, t, r, b = rect
                out.append(place({"k": "glyph", "r": [min(l, r), min(t, b), max(l, r), max(t, b)], "c": [u32(c)], "s": skew, "ab": ab}, pose, scissor))
        elif kind == "ColoredRectangleRenderState":
            x0, x1 = sorted((d["x0"], d["x1"]))
            out.append(place({"k": "fill", "r": [x0, d["y0"], x1, d["y1"]], "c": [u32(d["col1"]), u32(d["col2"])], "b": blend(d["pipeline"]), "ab": ab}, pose, scissor))
        else:
            tex = d["textureSetup"]["texure0"]
            if tex.startswith("parity:gui_item/"):
                out.append(place({"k": "item", "r": [d["x0"], d["y0"], d["x1"], d["y1"]], "ab": ab}, pose, scissor))
                continue
            parts = tiles(d) if kind == "TiledBlitRenderState" else [([d["x0"], d["y0"], d["x1"], d["y1"]], [d["u0"], d["v0"], d["u1"], d["v1"]])]
            for rect, uv in parts:
                q = {"k": "blit", "t": texture(tex), "uv": uv, "r": rect, "c": [u32(d["color"])], "b": blend(d["pipeline"]), "ab": ab}
                out.append(place(q, pose, scissor))
    return [q for q in out if q is not None]


def same(a, b):
    if a.keys() != b.keys() or any(a[k] != b[k] for k in ("k", "t", "b", "ab") if k in a):
        return False
    for key, tol in (("r", 2e-3), ("uv", 2e-3), ("s", 2e-3)):
        # plus two f32 ulps: the credits scroll rects a million pixels away
        if key in a and any(abs(x - y) > tol + abs(x) * 2.4e-7 for x, y in zip(a[key], b[key])):
            return False
    return "c" not in a or all(all(abs(((x >> s) & 255) - ((y >> s) & 255)) <= 1 for s in (0, 8, 16, 24)) for x, y in zip(a["c"], b["c"]))


def bounds(q):
    """ScreenRectangle.transformMaxBounds's integer box, which GuiRenderState sorts by."""
    l, t, r, b = q["r"]
    x, y = math.floor(l), math.floor(t)
    return x, y, x + math.ceil(r - l), y + math.ceil(b - t)


def overlap(a, b):
    a, b = bounds(a), bounds(b)
    return a[0] < b[2] and b[0] < a[2] and a[1] < b[3] and b[1] < a[3]


def match(java, rust):
    """Java index -> acacia index, greedily by key; None when some quad has no partner."""
    buckets = {}
    for j, q in enumerate(rust):
        buckets.setdefault((q["k"], q.get("t"), math.floor(q["r"][0])), []).append(j)
    pairs = []
    for q in java:
        x = q["r"][0]
        # same() allows a tolerance, so a partner may sit across a bucket edge
        keys = {(q["k"], q.get("t"), math.floor(v)) for v in (x - 2e-3, x + 2e-3)}
        found = next(((k, j) for k in keys for j in buckets.get(k, []) if same(q, rust[j])), None)
        if found is None:
            return None, q
        buckets[found[0]].remove(found[1])
        pairs.append(found[1])
    return pairs, None


def misordered(java, pairs):
    """A pair of overlapping quads drawn in the other order, or None."""
    cells = {}
    for i, q in enumerate(java):
        l, t, r, b = q["r"]
        for cx in range(int(l // 32), int(r // 32) + 1):
            for cy in range(int(t // 32), int(b // 32) + 1):
                cells.setdefault((cx, cy), []).append(i)
    for members in cells.values():
        for n, i in enumerate(members):
            for k in members[n + 1:]:
                if pairs[i] > pairs[k] and overlap(java[i], java[k]):
                    return i, k
    return None


def diverge(case, java, rust):
    if len(java) == len(rust) and all(same(a, b) for a, b in zip(java, rust)):
        return None
    pairs, missing = match(java, rust) if len(java) == len(rust) else (None, None)
    if pairs is None:
        i = next((i for i, (a, b) in enumerate(zip(java, rust)) if not same(a, b)), min(len(java), len(rust)))
        head = f"quad {i} of {len(java)} (acacia: {len(rust)}){'; unmatched ' + json.dumps(missing) if missing else ''}"
        return head, json.dumps(java[i:i + 2]), json.dumps(rust[i:i + 2])
    bad = misordered(java, pairs)
    if bad is None:
        return None
    i, k = bad
    return f"overlapping quads {i} and {k} of {len(java)} drawn in the other order", json.dumps([java[i], java[k]]), json.dumps([rust[pairs[i]], rust[pairs[k]]])


def compare(vanilla, rust, blends, source):
    """(cases, divergences) of one source's frames."""
    vanilla = [json.loads(line) for line in open(vanilla, encoding="utf-8") if line.strip()]
    rust = [json.loads(line) for line in open(rust, encoding="utf-8") if line.strip()]
    if len(vanilla) != len(rust):
        raise RuntimeError(f"{source}: {len(vanilla)} oracle lines, {len(rust)} probe lines")
    divergences = []
    for n, (v, r) in enumerate(zip(vanilla, rust)):
        if "error" in v:
            divergences.append((f"{source}#{n}", "oracle error", v["error"], ""))
            continue
        found = diverge(n, java_quads(v["draws"], v["window"]), r["frame"])
        if found:
            divergences.append((f"{source}#{n}", *found))
    return len(vanilla), divergences
