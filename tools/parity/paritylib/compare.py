"""Comparisons harnesses share. Each yields (case, field, expected, got) tuples."""
import json
import math


def same_number(a, b, tolerance):
    if isinstance(a, bool) or isinstance(b, bool):
        return a is b
    if isinstance(a, float) or isinstance(b, float):
        if math.isnan(a) or math.isnan(b):
            return math.isnan(a) and math.isnan(b)
        return abs(a - b) <= tolerance * max(1.0, abs(a))
    return a == b


def values(expected, got, field="", tolerance=1e-4):
    """Field by field: dict keys, list items (path `a.b[3]`), numbers within relative `tolerance`."""
    if isinstance(expected, dict) and isinstance(got, dict):
        for k in sorted(expected.keys() | got.keys()):
            path = f"{field}.{k}" if field else k
            if k not in got:
                yield path, expected[k], "<absent>"
            elif k not in expected:
                yield path, "<absent>", got[k]
            else:
                yield from values(expected[k], got[k], path, tolerance)
    elif isinstance(expected, list) and isinstance(got, list):
        if len(expected) != len(got):
            yield f"{field}.len", len(expected), len(got)
        for i, (a, b) in enumerate(zip(expected, got)):
            yield from values(a, b, f"{field}[{i}]", tolerance)
    elif isinstance(expected, (int, float)) and isinstance(got, (int, float)):
        if not same_number(expected, got, tolerance):
            yield field, expected, got
    elif expected != got:
        yield field, expected, got


def read_jsonl(path, key):
    records = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.strip():
                record = json.loads(line)
                records[str(record[key])] = record
    return records


def jsonl(expected_path, got_path, key="case", tolerance=1e-4, first_per_case=None):
    """Records matched by `key`; returns (cases, divergences). `first_per_case` caps divergences per case."""
    expected, got = read_jsonl(expected_path, key), read_jsonl(got_path, key)
    out = []
    for case, record in expected.items():
        if case not in got:
            out.append((case, "<record>", "present", "missing"))
            continue
        found = list(values(record, got[case], "", tolerance))
        out += [(case, *d) for d in found[:first_per_case]]
    out += [(case, "<record>", "absent", "present") for case in got.keys() - expected.keys()]
    return len(expected), out


def parse_probe(text):
    """A self-comparing probe's `CASES\\tn` and `DIVERGENCE\\tcase\\tfield\\texpected\\tgot` lines."""
    cases, out = None, []
    for line in text.splitlines():
        parts = line.split("\t")
        if parts[0] == "CASES" and len(parts) == 2:
            cases = int(parts[1])
        elif parts[0] == "DIVERGENCE" and len(parts) == 5:
            out.append(tuple(parts[1:]))
    return cases, out
