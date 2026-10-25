"""The look's Java entity models for the entity harnesses: `lookbake java-entities` on the pinned client jar."""
import os

from paritylib.home import GAME, HOME, VERSION


def look(h):
    """Exports the models into the harness's run dir; returns the path of entity_models.json."""
    java = HOME / "lookbake-java"
    jar = java / f"client-{VERSION}.jar"
    if not jar.exists():
        java.mkdir(parents=True, exist_ok=True)
        jar.symlink_to(GAME / "client.jar")
    os.environ.setdefault("JAVA_HOME", str(HOME / "jdk"))
    out = h.out / "look"
    h.cargo("run", "--release", "-q", "-p", "lookbake", "--", "java-entities", java, out)
    return out / "java" / "entity_models.json"
