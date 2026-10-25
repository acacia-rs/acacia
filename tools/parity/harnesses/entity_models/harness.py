"""Entity models: every model layer the client bakes, at rest, against the look's Java entity models
as acacia draws them (`lookbake java-entities`, then the probe acacia-render/examples/java_entities):
each part's initial pose and visibility, and each drawn corner's position, normal and UV."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from entity_look import look  # noqa: E402

OWNER = "acacia-render entity (Java look entities)"


def run(h):
    models = look(h)
    dump = h.oracle("acacia.parity.entity_models.EntityModelsOracle")
    h.read_probe(h.probe("acacia-render", "java_entities", "models", models, dump / "layers.jsonl"))
