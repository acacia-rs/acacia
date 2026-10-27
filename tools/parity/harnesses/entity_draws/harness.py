"""Entity draws: everything EntityRenderer.submit hands the collector for every kind the look
draws with a Java model, over random own-state booleans, enums, dyes and puffs: each model's layer,
texture, colour and render type, against Library::appearance and Library::gear with the state
answered as Bedrock would (examples/draws_probe/bridge.rs); probe acacia-render/examples/draws_probe."""
import sys
from pathlib import Path

sys.path[:0] = [str(Path(__file__).resolve().parents[1] / "entity_models")]
from entity_look import look_dir  # noqa: E402

OWNER = "acacia-render entity (Java look entities)"


def run(h):
    look = look_dir(h)
    dump = h.oracle("acacia.parity.entity_draws.EntityDrawsOracle")
    h.read_probe(h.probe("acacia-render", "draws_probe", look, dump / "draws.jsonl"))
