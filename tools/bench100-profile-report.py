"""Per-phase tables from a bench100-profile.sh run: hot functions and modules by cycles and
instructions, thread CPU vs run-queue wait, off-CPU stacks. Phases come from swarm.jsonl:
idle (before join), settle (join to steady), steady (steady to end).
usage: bench100-profile-report.py <run dir> [--top N]
Reads profile/cpu.data and profile/offcpu.data with `perf script --no-inline` (seconds; inlined
frames collapse into their caller, use perf annotate -l for lines).
"""
import collections
import json
import os
import re
import subprocess
import sys

PHASES = ("idle", "settle", "steady")
HEADER = re.compile(r"^(\S.*?)\s+(\d+)(?:/\d+)?\s+([\d.]+):\s+(\d+)\s+(\S+?):")
FRAME = re.compile(r"^\s+([0-9a-f]+)\s+(.*)\s+\((.*)\)\s*$")
SWITCH = re.compile(r"^(\S.*?)\s+(\d+)(?:/\d+)?\s+([\d.]+):\s+PERF_RECORD_SWITCH\s+(IN|OUT)\b(.*)")
HASH = re.compile(r"::h[0-9a-f]{16}$")


def phases(run):
    times = {}
    with open(os.path.join(run, "swarm.jsonl"), errors="replace") as f:
        for line in f:
            if '"phase"' in line:
                j = json.loads(line)
                times[j["phase"]] = j["t"]
    return times.get("join", 0.0), times.get("steady", float("inf")), times.get("end", float("inf"))


def monotonic(bounds, run):
    """Wall-clock phase bounds as perf's CLOCK_MONOTONIC timestamps (clock.txt: real - monotonic)."""
    offset = float(open(os.path.join(run, "profile/clock.txt")).read())
    return tuple(b - offset for b in bounds)


def phase_of(t, bounds):
    join, steady, end = bounds
    if t < join:
        return "idle"
    if t < steady:
        return "settle"
    return "steady" if t < end else None


def group(comm):
    return re.sub(r"[-_]?\d+$", "-*", comm)


def perf_script(data, *args):
    return subprocess.Popen(["perf", "script", "-i", data, "--no-inline", "-F", "comm,tid,time,event,ip,sym,dso,period", *args],
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, errors="replace", bufsize=1 << 20)


def records(proc):
    """Yields (comm, tid, t, period, event, frames leaf first)."""
    head = None
    frames = []
    for line in proc.stdout:
        if not line.strip():
            if head:
                yield (*head, frames)
            head, frames = None, []
            continue
        m = HEADER.match(line)
        if m:
            head = (m[1].strip(), int(m[2]), float(m[3]), int(m[4]), m[5].split("/")[0])
            frames = []
            continue
        if head:
            f = FRAME.match(line)
            if f:
                frames.append((HASH.sub("", f[2].strip().split("+0x")[0]) or "[unknown]", f[3]))
    if head:
        yield (*head, frames)


def module(sym, dso):
    base = dso.rsplit("/", 1)[-1]
    if "kernel" in dso or dso.startswith("[") or base.startswith("[k"):
        return "kernel"
    if base.startswith(("libc", "ld-linux", "libm", "libpthread")):
        return "libc"
    s = sym.lstrip("<&*")
    m = re.match(r"([a-zA-Z_][a-zA-Z0-9_]*)::", s)
    if not m:
        return "?other"
    crate = m[1]
    if crate in ("core", "std", "alloc", "hashbrown", "indexmap"):
        return "rust-std"
    return crate


def add(table, key, value):
    table[key] = table.get(key, 0) + value


def cpu(run, bounds):
    """{(phase, event, thread group): {"self": {sym:n}, "incl": {sym:n}, "mod": {crate:n}, "total": n}}"""
    out = collections.defaultdict(lambda: {"self": {}, "incl": {}, "mod": {}, "total": 0})
    for comm, _tid, t, period, event, frames in records(perf_script(os.path.join(run, "profile/cpu.data"))):
        ph = phase_of(t, bounds)
        if ph is None:
            continue
        for g in (group(comm), "ALL"):
            b = out[(ph, event.split(":")[0], g)]
            b["total"] += period
            if frames:
                add(b["self"], frames[0][0], period)
                add(b["mod"], module(*frames[0]), period)
                for sym in {f[0] for f in frames}:
                    add(b["incl"], sym, period)
    return out


def table(rows, total, top, fmt=str):
    return "\n".join(f"| {key} | {fmt(v)} | {100 * v / total:.1f}% |" for key, v in sorted(rows.items(), key=lambda kv: -kv[1])[:top])


def tasks(run, bounds):
    """Per thread and phase: CPU, run-queue wait, wall, and switches (deltas of tasks.tsv)."""
    series = collections.defaultdict(list)
    with open(os.path.join(run, "profile/tasks.tsv")) as f:
        for line in f:
            t, tid, comm, cpu_ns, runq, _sl, vol, invol, _st = line.rstrip("\n").split("\t")
            series[tid].append((float(t), comm, int(cpu_ns), int(runq), int(vol), int(invol)))
    rows = {}
    for tid, s in series.items():
        for ph, (a, b) in {"settle": (bounds[0], bounds[1]), "steady": (bounds[1], bounds[2])}.items():
            w = [x for x in s if a <= x[0] <= b]
            if len(w) >= 2 and w[-1][0] > w[0][0]:
                dt = w[-1][0] - w[0][0]
                rows[(ph, w[-1][1], tid)] = (dt, (w[-1][2] - w[0][2]) / 1e9, (w[-1][3] - w[0][3]) / 1e9, w[-1][4] - w[0][4], w[-1][5] - w[0][5])
    return rows


def app_frames(frames):
    """Application symbols from leaf inward, skipping kernel and libc noise."""
    return [f[0] for f in frames if module(*f) not in ("kernel", "libc")]


def offcpu(run, bounds):
    """Time between each switch-out and the next switch-in, by phase, thread group, cause and stack."""
    path = os.path.join(run, "profile/offcpu.data")
    if not os.path.exists(path):
        return {}
    proc = subprocess.Popen(["perf", "script", "-i", path, "--no-inline", "--show-switch-events", "-F", "comm,tid,time,event,ip,sym,dso,period"],
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, errors="replace", bufsize=1 << 20)
    # switch-out state per tid: (t, preempt, stack). A sample precedes its OUT record.
    last_stack, out_at = {}, {}
    table_ = collections.defaultdict(lambda: collections.defaultdict(float))
    counts = collections.defaultdict(int)
    head = None
    frames = []

    def flush():
        nonlocal head, frames
        if head:
            last_stack[head[1]] = frames
        head, frames = None, []

    for line in proc.stdout:
        m = SWITCH.match(line)
        if m:
            flush()
            comm, tid, t = m[1].strip(), int(m[2]), float(m[3])
            if m[4] == "OUT":
                out_at[tid] = (t, "preempt" in m[5], last_stack.pop(tid, []))
            elif tid in out_at:
                t0, preempt, stack = out_at.pop(tid)
                ph = phase_of(t0, bounds)
                if ph:
                    names = app_frames(stack)[:8]
                    key = (ph, group(comm), "preempted" if preempt else "blocked", " < ".join(names) or "(no user frames)")
                    table_[key]["t"] += t - t0
                    counts[key] += 1
            continue
        h = HEADER.match(line)
        if h:
            flush()
            head = (h[1].strip(), int(h[2]))
            continue
        if not line.strip():
            flush()
            continue
        f = FRAME.match(line)
        if f and head:
            frames.append((HASH.sub("", f[2].strip().split("+0x")[0]), f[3]))
    flush()
    return {k: (v["t"], counts[k]) for k, v in table_.items()}


def stats(run, bounds):
    """Per phase and thread group: sums of stat.csv's per-interval counters."""
    totals = collections.defaultdict(lambda: collections.defaultdict(float))
    path = os.path.join(run, "profile/stat.csv")
    start_path = os.path.join(run, "profile/stat.start")
    if not (os.path.exists(path) and os.path.exists(start_path)):
        return totals
    start = float(open(start_path).read())
    with open(path, errors="replace") as f:
        for line in f:
            parts = line.rstrip("\n").split(",")
            if line.startswith("#") or len(parts) < 5:
                continue
            try:
                t, value = float(parts[0]), float(parts[2])
            except ValueError:
                continue
            ph = phase_of(start + t, bounds)
            if ph:
                thread = group(re.sub(r"-\d+$", "", parts[1]))
                totals[(ph, thread)][parts[4]] += value
    return totals


def report(run, top):
    bounds = phases(run)
    print(f"# Profile of {run}\nphases: join {bounds[0]:.1f} steady {bounds[1]:.1f} end {bounds[2]:.1f}\n")
    mono = monotonic(bounds, run)
    results = cpu(run, mono)
    for (ph, event, g), b in sorted(results.items(), key=lambda kv: (PHASES.index(kv[0][0]), kv[0][1], kv[0][2] != "ALL", -kv[1]["total"])):
        if not b["total"] or (g != "ALL" and b["total"] < 0.005 * results[(ph, event, "ALL")]["total"]):
            continue
        print(f"\n## {ph} / {event} / {g} (total {b['total']:.3g})\n")
        print("self | samples | share\n---|---|---")
        print(table(b["self"], b["total"], top, lambda v: f"{v:.3g}"))
        print("\ninclusive | samples | share\n---|---|---")
        print(table(b["incl"], b["total"], top, lambda v: f"{v:.3g}"))
        print("\nmodule (self) | samples | share\n---|---|---")
        print(table(b["mod"], b["total"], 20, lambda v: f"{v:.3g}"))
    print("\n## Thread scheduling (tasks.tsv deltas)\n\nphase | thread | wall s | on-CPU s | run-queue wait s | voluntary | involuntary\n---|---|---|---|---|---|---")
    for (ph, comm, tid), r in sorted(tasks(run, bounds).items()):
        print(f"{ph} | {comm} ({tid}) | {r[0]:.1f} | {r[1]:.2f} | {r[2]:.2f} | {r[3]} | {r[4]}")
    print("\n## perf stat per thread group\n\nphase | thread | task-clock ms | cycles | instructions | IPC | context-switches\n---|---|---|---|---|---|---")
    for (ph, thread), v in sorted(stats(run, bounds).items(), key=lambda kv: (kv[0][0], -kv[1]["instructions"])):
        ipc = v["instructions"] / v["cycles"] if v["cycles"] else 0
        print(f"{ph} | {thread} | {v['task-clock']:.0f} | {v['cycles']:.3g} | {v['instructions']:.3g} | {ipc:.2f} | {v['context-switches']:.0f}")
    off = offcpu(run, mono)
    if off:
        print("\n## Off-CPU (context-switch spans, sum of seconds per thread group)\n\nphase | thread | cause | stack (leaf first) | seconds | switches\n---|---|---|---|---|---")
        for k, (t, n) in sorted(off.items(), key=lambda kv: (kv[0][0], kv[0][1], kv[0][2], -kv[1][0]))[:top * 8]:
            print(f"{k[0]} | {k[1]} | {k[2]} | {k[3][:300]} | {t:.2f} | {n}")


if __name__ == "__main__":
    args = sys.argv[1:]
    top = 30
    if "--top" in args:
        i = args.index("--top")
        top = int(args[i + 1])
        del args[i:i + 2]
    report(args[0], top)
