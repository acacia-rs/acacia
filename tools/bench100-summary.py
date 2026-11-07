"""Endstone-style summary of tools/bench100.sh run dirs: one markdown table, a column per run.

TPS is game time advanced per wall second, from `time query gametime` replies (swarm.jsonl); a
server without game time (Chorus) falls back to its own reported TPS average, marked `~`.
Main thread per tick = tick thread CPU seconds per wall second / TPS.
"""
import json
import os
import re
import statistics
import sys

CLK = os.sysconf("SC_CLK_TCK")


def lines(path):
    try:
        with open(path, errors="replace") as f:
            return f.read().splitlines()
    except FileNotFoundError:
        return []


def load(run):
    r = {"phases": {}, "gametime": [], "views": [], "spawned": set(), "disconnects": [], "meta": {}}
    for line in lines(os.path.join(run, "meta.txt")):
        key, _, value = line.partition(" ")
        r["meta"][key] = value
    for line in lines(os.path.join(run, "swarm.jsonl")):
        try:
            j = json.loads(line)
        except ValueError:
            continue
        if "phase" in j:
            r["phases"][j["phase"]] = j["t"]
        elif "gametime" in j:
            r["gametime"].append((j["t"], j["gametime"]))
        elif "view" in j:
            r["views"].append(j)
        elif j.get("event") == "spawned":
            r["spawned"].add(j["id"])
        elif j.get("event") == "disconnected":
            r["disconnects"].append(j.get("reason", ""))
    r["samples"] = [tuple(float(x) for x in l.split()[:3]) for l in lines(os.path.join(run, "samples.tsv")) if len(l.split()) >= 3]
    r["threads"] = {}
    for l in lines(os.path.join(run, "threads.tsv")):
        t, tid, ticks, *comm = l.split()
        r["threads"].setdefault(tid, []).append((float(t), int(ticks), " ".join(comm)))
    r["console_tps"] = console_tps(run)
    return r


def console_tps(run):
    """(time, reported TPS average) from `status performance` replies, paired with the poll times."""
    polls = [float(l) for l in lines(os.path.join(run, "console-polls.tsv")) if l.strip()]
    values = []
    for l in lines(os.path.join(run, "server.log")):
        m = re.search(r"TPS:\s*[\d.]+\D*?([\d.]+) avg", re.sub("§.", "", l))
        if m:
            values.append(float(m.group(1)))
    return list(zip(polls, values))


def rate(samples, t0, t1):
    s = [p for p in samples if t0 <= p[0] <= t1]
    if len(s) < 2 or s[-1][0] <= s[0][0]:
        return None
    return (s[-1][1] - s[0][1]) / (s[-1][0] - s[0][0])


def lowest_window(samples, t0, t1, width=15.0):
    s = [p for p in samples if t0 <= p[0] <= t1]
    worst = None
    for i, a in enumerate(s):
        b = next((b for b in s[i + 1:] if b[0] - a[0] >= width), None)
        if b:
            v = (b[1] - a[1]) / (b[0] - a[0])
            worst = v if worst is None else min(worst, v)
    return worst


def at(series, t):
    """Last value at or before t."""
    best = None
    for p in series:
        if p[0] <= t:
            best = p
    return best


def tick_thread(r, t0, t1):
    pattern = r["meta"].get("tick_re", "")
    best = None
    for tid, series in r["threads"].items():
        a, b = at(series, t0), at(series, t1)
        if not a or not b:
            continue
        used = (b[1] - a[1]) / CLK / max(b[0] - a[0], 1e-9)
        matches = bool(pattern) and re.search(pattern, b[2])
        key = (bool(matches), used)
        if best is None or key > best[0]:
            best = (key, used, b[2])
    return best and (best[1], best[2])


def summarise(run):
    r = load(run)
    p = r["phases"]
    join, steady, end = p.get("join"), p.get("steady"), p.get("end")
    row = {"run": os.path.basename(os.path.normpath(run)), "target": r["meta"].get("target", "?").split()[0]}
    if not (join and steady and end):
        row["note"] = f"incomplete: phases {sorted(p)}"
        return row
    approx = not r["gametime"]
    tps_series = r["gametime"] or r["console_tps"]
    if approx:
        tps = statistics.mean(v for t, v in tps_series if steady <= t <= end) if any(steady <= t <= end for t, _ in tps_series) else None
        low = min((v for t, v in tps_series if join <= t <= steady), default=None)
    else:
        tps, low = rate(tps_series, steady, end), lowest_window(tps_series, join, steady)
    tick = tick_thread(r, steady, end)
    cpu = rate(r["samples"], steady, end)
    rss = [s[2] for s in r["samples"] if steady <= s[0] <= end]
    loads = [v["load90"] for v in r["views"] if v.get("load90") is not None]
    mark = "~" if approx else ""
    row.update({
        "Steady TPS": f"{mark}{tps:.2f}" if tps is not None else "n/a",
        "Main thread per tick": f"{tick[0] / tps * 1000:.2f} ms" if tick and tps else "n/a",
        "View load median": f"{statistics.median(loads):.1f} s" if loads else "n/a",
        "Lowest 15 s TPS while views load": f"{mark}{low:.2f}" if low is not None else "n/a",
        "CPU cores": f"{cpu / CLK:.2f}" if cpu is not None else "n/a",
        "RSS": f"{statistics.mean(rss) / 1024:.0f} MiB (max {max(rss) / 1024:.0f})" if rss else "n/a",
        "bots spawned / with a view": f"{len(r['spawned'])} / {len(loads)} of {len(r['views'])}",
        "disconnects": str(len(r["disconnects"])),
        "settle": f"{steady - join:.0f} s",
        "tick thread": f"{tick[1]} ({tick[0]:.2f} cores)" if tick else "n/a",
    })
    if r["disconnects"]:
        row["first disconnect"] = r["disconnects"][0][:80]
    return row


def main(runs):
    rows = [summarise(run) for run in runs]
    keys = []
    for row in rows:
        keys += [k for k in row if k not in keys]
    print("| | " + " | ".join(f"{r['target']} ({r['run']})" for r in rows) + " |")
    print("|---" * (len(rows) + 1) + "|")
    for k in keys:
        if k not in ("run", "target"):
            print(f"| {k} | " + " | ".join(r.get(k, "") for r in rows) + " |")
    if any(not r.get("Steady TPS", "").startswith("~") for r in rows):
        print("\nTPS: game time per wall second (bot 0 polls `time query gametime` every 5 s); `~`: the server's own TPS average.")


if __name__ == "__main__":
    main(sys.argv[1:] or sys.exit("usage: bench100-summary.py <run dir>..."))
