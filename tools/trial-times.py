"""Wall-clock profile of a finished schematic trial in a testbox dir: the release build, each structure's run
(game seconds, and the stalled tail after its last placement), and the whole run.
usage: python3 tools/trial-times.py ~/work/<dir>"""
import glob, json, os, re, sys
from datetime import datetime

d = os.path.expanduser(sys.argv[1])
done = os.path.exists(f"{d}/run.done")
start, end = os.path.getmtime(f"{d}/run.sh"), os.path.getmtime(f"{d}/run.done") if done else datetime.now().timestamp()
hms = lambda t: datetime.fromtimestamp(t).strftime("%H:%M:%S")
births, rows = [], []
for log in sorted(glob.glob(f"{d}/trial-*.log")):
    if log.endswith(".try1.log"):
        continue
    name = os.path.basename(log)[6:-4]
    txt = open(log, errors="replace").read()
    m = re.findall(r"(PASS|FAIL) after (\d+) ticks \((\d+) s\)", txt)
    journal = (glob.glob(f"{d}/journal-{name}/B*.jsonl") or [None])[0]
    born = os.stat(journal).st_birthtime if journal and hasattr(os.stat(journal), "st_birthtime") else None
    born = born or (os.path.getctime(f"{d}/journal-{name}") if journal else None)
    acts, last = [], 0
    for line in open(journal, errors="replace") if journal else []:
        try:
            o = json.loads(line)
        except ValueError:
            continue
        last = max(last, o.get("tick", 0))
        if o.get("e") == "acted" and o.get("kind") in ("place", "break"):
            acts.append(o["tick"])
    tail = last - acts[-1] if acts else 0
    if born:
        births.append(born)
    verdict, ticks, secs = (m[-1][0], int(m[-1][1]), int(m[-1][2])) if m else ("ERROR", 0, 0)
    rows.append((name, verdict, ticks, secs, tail, born, os.path.getmtime(log)))
first = min(births) if births else end
print(f"run {hms(start)} -> {hms(end) if done else 'still running'}: {(end - start) / 60:.1f} min; "
      f"build + first BDS start {first - start:.0f} s")
for name, verdict, ticks, secs, tail, born, done in sorted(rows, key=lambda r: r[5] or 0):
    print(f"  {name:20} {verdict:5} {secs:4} s game, {ticks:6} ticks, stalled tail {tail:5} ticks, "
          f"{hms(born) if born else '?':8} -> {hms(done)}")
