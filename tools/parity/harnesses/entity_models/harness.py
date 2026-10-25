"""Entity models: every model layer the client bakes, at rest, against the look's Java entity models
as acacia draws them (`lookbake java-entities`, then the probe acacia-render/examples/java_entities):
each part's initial pose and visibility, and each drawn corner's position, normal and UV."""
import os

from paritylib.home import GAME, HOME, VERSION

OWNER = "acacia-render entity (Java look entities)"


def look(h):
    """The look's entity models, exported by lookbake from the pinned client jar; returns its path."""
    java = HOME / "lookbake-java"
    jar = java / f"client-{VERSION}.jar"
    if not jar.exists():
        java.mkdir(parents=True, exist_ok=True)
        jar.symlink_to(GAME / "client.jar")
    os.environ.setdefault("JAVA_HOME", str(HOME / "jdk"))
    out = h.out / "look"
    h.cargo("run", "--release", "-q", "-p", "lookbake", "--", "java-entities", java, out)
    return out / "java" / "entity_models.json"


def run(h):
    models = look(h)
    dump = h.oracle("acacia.parity.entity_models.EntityModelsOracle")
    h.read_probe(h.probe("acacia-render", "java_entities", "models", models, dump / "layers.jsonl"))
