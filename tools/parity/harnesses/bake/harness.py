"""Block bake: every Java block state's baked quads, alternatives and picks, offsets, biome colours and AO shade against `lookbake java-check`."""
from paritylib.home import GAME, HOME, VERSION

OWNER = "lookbake (Java look block bake)"


def run(h):
    dump = h.oracle("acacia.parity.bake.BakeOracle")
    java = HOME / "lookbake-java"
    jar = java / f"client-{VERSION}.jar"
    if not jar.exists():
        java.mkdir(parents=True, exist_ok=True)
        jar.symlink_to(GAME / "client.jar")
    out = h.cargo("run", "--release", "-q", "-p", "lookbake", "--", "java-check", dump / "quads.json.gz", java, check=False)
    h.read_probe(out)
