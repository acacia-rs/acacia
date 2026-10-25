"""World light end to end: real Bedrock chunks from the movement traces in crates/acacia-bot/tests/traces (the test BDS: a flat overworld with built courses, generated Nether and End), mapped to Java states by lookbake's table, lit by Java's LevelLightEngine and drawn with its BlockModelLighter, against acacia's light engine and mesher with the Java look: each face both draw, corner by corner (block light, sky light, grey)."""
from paritylib.home import HOME, ROOT
from paritylib.java_look import java_dir

OWNER = "light (Java look world light)"

TRACES = ROOT / "crates" / "acacia-bot" / "tests" / "traces"
# (trace, marks): each mark's first time, the 5x5 chunks around the player; the inner 3x3 compared.
SCENES = [
    ("drills-dimensions.btrc.gz", ["nether > overworld", "the_end > overworld", "overworld > nether"]),
    ("fuzz-a.btrc.gz", [f"r{i}" for i in range(0, 20, 4)]),
    ("fuzz-b.btrc.gz", [f"r{i}" for i in range(2, 20, 4)]),
    ("drills-honey.btrc.gz", ["honey0"]),
    ("drills-powder-snow.btrc.gz", ["powder1"]),
    ("drills-ceiling-edge.btrc.gz", ["duck0"]),
]
RADIUS = 2


def run(h):
    scenes = h.out / "scenes.jsonl"
    parts = []
    for i, (trace, marks) in enumerate(SCENES):
        part = h.out / f"scenes-{i}.jsonl"
        h.cargo("run", "--release", "-q", "-p", "acacia-bot", "--example", "trace_region", "--", TRACES / trace, str(RADIUS), part, *marks)
        parts.append(part.read_text(encoding="utf-8"))
    scenes.write_text("".join(parts), encoding="utf-8")
    java = java_dir()
    mapping = java / "blockstate_mappings.json"
    if not mapping.exists():
        h.cargo("run", "--release", "-q", "-p", "lookbake", "--", "fetch-java", java)
    quads = h.oracle("acacia.parity.world_light.WorldLightOracle", scenes, mapping, inputs=[scenes, mapping], heap="8g") / "quads.jsonl"
    out = h.cargo("run", "--release", "-q", "-p", "lookbake", "--", "java-world-light", scenes, quads, HOME / "bedrock-pack", java, check=False)
    h.read_probe(out)
