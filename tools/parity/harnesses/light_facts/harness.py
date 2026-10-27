"""Light facts: what the Java look bakes for each mapped Bedrock state's light (emission, dampening, solid render, full collision, emissive, AO darkening, face occlusion shapes) against the game's, by `lookbake java-light-check`."""
from paritylib.home import GAME, HOME, VERSION

OWNER = "light (Java look world light)"


def run(h):
    dump = h.oracle("acacia.parity.light_facts.LightFactsOracle")
    java = HOME / "lookbake-java"
    jar = java / f"client-{VERSION}.jar"
    if not jar.exists():
        java.mkdir(parents=True, exist_ok=True)
        jar.symlink_to(GAME / "client.jar")
    out = h.cargo("run", "--release", "-q", "-p", "lookbake", "--", "java-light-check", dump / "facts.json", HOME / "bedrock-pack", java, check=False)
    h.read_probe(out)
