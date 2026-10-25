"""Clouds: cloud-map cells and empty neighbours, and per frame the scroll offset, colour and fancy/fast face lists in build order, against `acacia-render`'s clouds_probe."""

OWNER = "atmos (clouds)"
CASES = 120


def run(h):
    expected = h.oracle("acacia.parity.clouds.CloudsOracle", CASES)
    h.probe("acacia-render", "clouds_probe", expected, h.out)
    for kind in ("textures", "frames"):
        h.compare_jsonl(expected / f"{kind}.jsonl", h.out / f"{kind}.jsonl", key="case", tolerance=0)
