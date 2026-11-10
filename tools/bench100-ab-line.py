"""One line per bench100 run: disconnects, load, steady TPS and tick-thread instructions per tick."""
import json
import re
import sys

run, report = sys.argv[1:3]
disconnects, phases = 0, {}
for line in open(f"{run}/swarm.jsonl"):
    try:
        j = json.loads(line)
    except ValueError:
        continue
    disconnects += j.get("event") == "disconnected"
    if "phase" in j:
        phases[j["phase"]] = j["t"]
tps = re.search(r"Steady TPS \| ~?([\d.]+|n/a)", open(f"{run}/summary.md").read()).group(1)
stat = re.search(r"^steady \| sapling-shard-\* \| [^|]+\| [^|]+\| ([\d.e+]+)", open(report).read(), re.M)
seconds = phases.get("end", 0) - phases.get("steady", 0)
per_tick = "n/a"
if stat and tps != "n/a" and seconds > 0:
    per_tick = f"{float(stat.group(1)) / (float(tps) * seconds) / 1e6:.1f} M"
print(f"{run.rsplit('/', 1)[-1]}: disconnects {disconnects}  steady TPS {tps}  tick-thread instructions/tick {per_tick}", flush=True)
