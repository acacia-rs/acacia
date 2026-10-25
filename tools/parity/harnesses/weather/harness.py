"""Weather: rain/snow columns and quads, precipitation and temperature per biome and height, rain splashes and sound, lightning bolt geometry against `acacia-render`'s weather_probe."""

OWNER = "atmos (weather)"
CASES = 400


def run(h):
    expected = h.oracle("acacia.parity.weather.WeatherOracle", CASES)
    h.probe("acacia-render", "weather_probe", expected, h.out)
    for kind in ("columns", "climate", "splashes", "bolts"):
        h.compare_jsonl(expected / f"{kind}.jsonl", h.out / f"{kind}.jsonl", key="case", tolerance=0)
