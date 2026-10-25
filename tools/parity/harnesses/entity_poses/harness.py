"""Entity poses: each Java model the look draws, posed by the client's own setupAnim on a corpus of
render states (entity_corpus.py), against our port (acacia-render entity/java/anim) drawn by the
probe acacia-render/examples/java_entities: each part's pose and each drawn corner. A layer whose
setupAnim we have not ported is one divergence per case."""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path[:0] = [str(HERE), str(HERE.parent / "entity_models")]
import entity_corpus  # noqa: E402
from entity_look import look  # noqa: E402

OWNER = "acacia-render entity (Java look entities)"


def run(h):
    models = look(h)
    corpus = entity_corpus.write(h, entity_corpus.EVERY)
    dump = h.oracle("acacia.parity.entity_poses.EntityPosesOracle", corpus, inputs=[corpus])
    h.read_probe(h.probe("acacia-render", "java_entities", "poses", models, dump / "poses.jsonl"))
