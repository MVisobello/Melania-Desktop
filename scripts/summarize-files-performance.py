#!/usr/bin/env python3
"""Summarize opt-in Melania Files timing logs, without changing the app or VM.

Run Melania with MELANIA_PROFILE_FILES=1, then pass the captured stderr log.
Metrics describe measured events, not frame-rate or a benchmark score.
"""
import argparse
from collections import defaultdict
from math import ceil
from pathlib import Path
import re
import sys

METRICS = (
    "FIRST_WINDOW_MS", "FIRST_TAB_MS", "UI_STALL_MS", "SCAN_MS",
    "CLEAR_MS", "RENDER_MS", "THUMBNAILS_MS",
)
LABELS = {f"MELANIA_FILES_{name}" for name in METRICS}
METRIC_RE = re.compile(r"\b(MELANIA_FILES_[A-Z_]+)=(\d+)\b")


def parse_timing_lines(lines):
    samples = defaultdict(list)
    for line in lines:
        for key, value in METRIC_RE.findall(line):
            if key in LABELS:
                samples[key].append(int(value))
    return samples


def percentile_nearest_rank(samples, percent):
    ordered = sorted(samples)
    if not ordered:
        return None
    return ordered[max(0, ceil(len(ordered) * percent / 100) - 1)]


def format_report(label, samples):
    rows = [f"{label}", "metric                    count  p50 ms  p95 ms  max ms"]
    for metric in METRICS:
        values = samples.get(f"MELANIA_FILES_{metric}", ())
        if not values:
            continue
        rows.append(
            f"{metric:<25} {len(values):>5} {percentile_nearest_rank(values,50):>7}"
            f" {percentile_nearest_rank(values,95):>7} {max(values):>7}"
        )
    if len(rows) == 2:
        rows.append("No recognized Melania Files timing samples.")
    return "\n".join(rows)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("logs", nargs="+", type=Path, help="captured Melania stderr logs")
    args = parser.parse_args(argv)
    any_samples = False
    for log in args.logs:
        try:
            with log.open(encoding="utf-8", errors="replace") as stream:
                samples = parse_timing_lines(stream)
        except OSError as error:
            parser.error(f"{log}: {error}")
        print(format_report(str(log), samples))
        print()
        any_samples = any_samples or bool(samples)
    return 0 if any_samples else 1


if __name__ == "__main__":
    sys.exit(main())
