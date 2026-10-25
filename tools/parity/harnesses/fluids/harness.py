"""Liquids: FluidRenderer.tesselate over pools, levels, falls, spills, lava, waterlogged blocks, neighbours, biomes and random mixes against acacia-render's `mesh::liquid::java_faces` (example fluids_probe) on the same cells, the world mapped to Bedrock as the Java look maps it and the block table from a Java look baked here; then every block state's fluid facts (occluded faces, legacy solid, #blocks_fluid_flow, overlay, sturdy faces, solid render) against its FluidRules."""
import hashlib
import json
import shutil
import subprocess

from paritylib.home import GAME, HOME, ROOT, VERSION

OWNER = "atmos (liquids)"
TOLERANCE = 1e-4


def run(h):
    truth = h.oracle("acacia.parity.fluids.FluidsOracle")
    java = java_dir()
    look = bake_look(h, java)
    got = h.out / "probe.jsonl"
    facts = h.probe("acacia-render", "fluids_probe", truth / "cases.jsonl", java / "blockstate_mappings.json", look, got, truth / "facts.jsonl")
    h.add(*compare(truth / "quads.jsonl", got))
    h.read_probe(facts)


def java_dir():
    """lookbake's Java directory, its jar the oracle's (lookbake fetches the rest)."""
    java = HOME / "lookbake-java"
    jar = java / f"client-{VERSION}.jar"
    if not jar.exists():
        java.mkdir(parents=True, exist_ok=True)
        jar.symlink_to(GAME / "client.jar")
    return java


def bake_look(h, java):
    """The Java look, baked by this checkout's lookbake over the Bedrock pack (fetched once into the
    home); rebaked when lookbake's or acacia-render's sources change."""
    pack, out = HOME / "bedrock-pack", h.out / "look"
    if not (pack / "blocks.json").exists():
        subprocess.run(["bash", ROOT / "tools" / "fetch-vanilla-pack.sh"], check=True, stdout=h.log, stderr=h.log)
        shutil.rmtree(pack, ignore_errors=True)
        shutil.move(ROOT / "assets" / "vanilla", pack)
    sources = sorted([*(ROOT / "tools" / "lookbake" / "src").rglob("*.rs"), *(ROOT / "crates" / "acacia-render" / "src").rglob("*.rs")])
    key = hashlib.sha256(b"".join(p.read_bytes() for p in sources)).hexdigest()
    stamp = out / "sources.sha256"
    if h.fresh or not stamp.exists() or stamp.read_text() != key:
        h.cargo("run", "--release", "-q", "-p", "lookbake", "--", "java", pack, java, out)
        stamp.write_text(key)
    return out


def canonical(quad):
    """(key, vertices): rotated to start at the least position; keyed by sprite, layer and positions to 1/1000."""
    vertices = quad["vertices"]
    position = [tuple(round(c * 1000) for c in v[:3]) for v in vertices]
    start = min(range(4), key=lambda i: position[i])
    order = [(start + i) % 4 for i in range(4)]
    key = (vertices[0][8], quad["layer"], tuple(position[i] for i in order))
    return key, [vertices[i] for i in order]


def by_key(quads):
    out = {}
    for quad in quads:
        key, vertices = canonical(quad)
        out.setdefault(key, []).append(vertices)
    return out


def describe(key):
    sprite, layer, position = key
    return f"{sprite.rsplit('/', 1)[-1]}/{layer}@" + " ".join(",".join(f"{c / 1000:g}" for c in p) for p in position)


def compare(expected_path, got_path):
    expected = {r["case"]: r for r in map(json.loads, open(expected_path, encoding="utf-8"))}
    got = {r["case"]: r for r in map(json.loads, open(got_path, encoding="utf-8"))}
    out = []
    for case, record in expected.items():
        mine = got.get(case)
        if mine is None:
            out.append((case, "<record>", "present", "missing"))
            continue
        if mine["unmapped"]:
            out.append((case, "unmapped", "", " ".join(sorted(set(mine["unmapped"])))))
        found = []
        want, have = by_key(record["quads"]), by_key(mine["quads"])
        for key in sorted(want.keys() | have.keys()):
            a, b = want.get(key, []), have.get(key, [])
            if len(a) != len(b):
                found.append((case, f"quads {describe(key)}", len(a), len(b)))
                continue
            for va, vb in zip(sorted(a), sorted(b)):
                found += [(case, f"{field} {describe(key)}", x, y) for field, x, y in vertex_differences(va, vb)]
        out += found[:4]
        if len(found) > 4:
            out.append((case, "more", "", f"{len(found) - 4} more"))
    return len(expected), out


def vertex_differences(a, b):
    for i, (va, vb) in enumerate(zip(a, b)):
        if any(abs(x - y) > TOLERANCE for x, y in zip(va[3:5], vb[3:5])):
            yield f"uv[{i}]", va[3:5], vb[3:5]
        if va[5:7] != vb[5:7]:
            yield f"color[{i}]", f"{va[5]:06x}/{va[6]}", f"{vb[5]:06x}/{vb[6]}"
        if va[7] != vb[7]:
            yield f"light[{i}]", va[7], vb[7]
