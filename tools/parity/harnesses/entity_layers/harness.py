"""Entity layers: what the client's render layers place on a posed model, against acacia's.
Held items: the matrix ItemInHandLayer hands each arm's item (the model's own translateToHand,
then the layer's turn and fist offset, transcribed in the oracle) for every armed model on the
entity_poses corpus, against Mesh::hand and item::hand::fist; probe
acacia-render/examples/java_entities."""
import sys
from pathlib import Path

HARNESSES = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(HARNESSES / "entity_poses"), str(HARNESSES / "entity_models")]
import entity_corpus  # noqa: E402
from entity_look import look  # noqa: E402

OWNER = "acacia-render entity (Java look entities)"


def run(h):
    models = look(h)
    corpus = entity_corpus.write(h, entity_corpus.ARMED)
    dump = h.oracle("acacia.parity.entity_layers.EntityLayersOracle", corpus, inputs=[corpus])
    h.read_probe(h.probe("acacia-render", "java_entities", "layers", models, dump / "layers.jsonl"))
