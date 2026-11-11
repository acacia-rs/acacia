"""Particles: block animateTick effects and particle providers ticked on a real ClientLevel, against `particles::java::probe` (probe acacia-render/examples/java_particles), particle by particle each tick."""
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from corpus import cases  # noqa: E402
from paritylib.home import HOME  # noqa: E402

OWNER = "atmos (particles)"

# Bedrock state -> Java state, ViaBedrock's table (GPL-3.0): fetched, never vendored. Same pin as tools/lookbake/src/mapping.rs.
TABLE = ("https://raw.githubusercontent.com/RaphiMC/ViaBedrock/6e242b278740ceccac20e5db0915b00775095b24/"
         "src/main/resources/assets/viabedrock/data/custom/blockstate_mappings.json")
TABLE_SHA1 = "1f93fd312bcc4117162a704a4e716cc3dabcfd75"


def parse(state):
    name, _, properties = state.partition("[")
    pairs = tuple(sorted(tuple(p.split("=", 1)) for p in properties.rstrip("]").split(",") if "=" in p))
    return name if ":" in name else f"minecraft:{name}", pairs


def spell(name, pairs):
    return name + (f"[{','.join(f'{k}={v}' for k, v in pairs)}]" if pairs else "")


def java_to_bedrock():
    path = HOME / "cache" / "viabedrock" / "blockstate_mappings.json"
    if not path.is_file():
        path.parent.mkdir(parents=True, exist_ok=True)
        data = urllib.request.urlopen(TABLE, timeout=60).read()
        if hashlib.sha1(data).hexdigest() != TABLE_SHA1:
            raise RuntimeError("ViaBedrock table does not match its pin")
        path.write_bytes(data)
    inverse = {}
    for bedrock, java in json.loads(path.read_text()).items():
        inverse.setdefault(parse(java), []).append(spell(*parse(bedrock)))
    return {k: sorted(v) for k, v in inverse.items()}


def bedrock(table, java, log):
    """The first Bedrock state the table maps to `java`; else the one sharing most properties (logged)."""
    key = parse(java)
    if key in table:
        return table[key][0]
    near = [(-len(set(k[1]) & set(key[1])), v[0]) for k, v in table.items() if k[0] == key[0]]
    if not near:
        return None
    unshared, choice = min(near)
    log.write(f"no Bedrock state maps to {java}: took {choice} ({-unshared} properties shared)\n")
    return choice


def run(h):
    corpus = cases()
    path = h.out / "cases.jsonl"
    path.write_text("".join(json.dumps(c) + "\n" for c in corpus))
    expected = h.oracle("acacia.parity.particles.ParticlesOracle", path, inputs=[path])
    scenes = {s["case"]: s for s in map(json.loads, (expected / "scenes.jsonl").read_text().splitlines())}
    table = java_to_bedrock()
    probe_cases = []
    for case in corpus:
        scene = scenes.get(case["case"])
        if scene is None or "error" in scene:
            h.diverge(case["case"], "<oracle>", scene and scene["error"], "")
            continue
        blocks = [[x, y, z, bedrock(table, java, h.log)] for x, y, z, java in scene["blocks"]]
        missing = [java for (*_, java), b in zip(scene["blocks"], blocks) if b[3] is None]
        if missing:
            h.diverge(case["case"], "<bedrock state>", missing[0], "unmapped")
            continue
        probe_cases.append(dict(case, blocks=blocks, foliage=scene["foliage"]))
    probe_path = h.out / "probe-cases.jsonl"
    probe_path.write_text("".join(json.dumps(c) + "\n" for c in probe_cases))
    h.probe("acacia-render", "java_particles", probe_path, expected / "seeds.jsonl", h.out / "rust.jsonl")
    h.compare_jsonl(expected / "records.jsonl", h.out / "rust.jsonl", tolerance=1e-6)
