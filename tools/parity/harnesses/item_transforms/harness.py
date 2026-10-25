"""Item transforms: where the camera holds an item in first person (FirstPersonHandsAndItemsRenderer
.submitArmWithItem's pose before the item's display) over hands, lowering, swings, use animations,
crossbows and the spin attack, against item::first_person::frame; probe
acacia-render/examples/first_person. The display transforms after it are item_models'."""
import itertools
import json

OWNER = "acacia-render item (Java look items)"

ATTACKS = [0.0, 0.15, 0.5, 0.85]
LOWERED = [0.0, 0.4]
USES = [None, {"kind": "block", "held": 3.0}]
USES += [{"kind": "eat", "held": h, "duration": d} for h in (0.0, 3.5, 12.0, 25.0) for d in (16, 32)]
USES += [{"kind": k, "held": h} for k in ("bow", "trident") for h in (0.0, 1.5, 6.0, 30.0)]
USES += [{"kind": "crossbow", "held": h, "charge": 25.0} for h in (0.0, 4.0, 30.0)]


def cases():
    out = []
    for left, main, lowered, attack in itertools.product((False, True), (True, False), LOWERED, ATTACKS):
        base = {"left": left, "main_hand": main, "lowered": lowered, "attack": attack, "swinging": attack > 0, "spinning": False}
        for using in USES:
            out.append({**base, "using": using, "crossbow": None})
        out.append({**base, "using": None, "crossbow": None, "spinning": True})
        for charged in (False, True):
            for using in (None, USES[-2]):
                out.append({**base, "using": using, "crossbow": charged})
    return [{"case": f"#{i}", **c} for i, c in enumerate(out)]


def run(h):
    corpus = h.out / "cases.jsonl"
    corpus.write_text("".join(json.dumps(c) + "\n" for c in cases()))
    dump = h.oracle("acacia.parity.item_transforms.ItemTransformsOracle", corpus, inputs=[corpus])
    h.read_probe(h.probe("acacia-render", "first_person", dump / "hands.jsonl"))
