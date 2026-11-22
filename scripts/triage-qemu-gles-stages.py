#!/usr/bin/env python3
"""Triage the experimental QEMU GLES DMA-BUF path without claiming GPU success.

Usage: python3 scripts/triage-qemu-gles-stages.py guest.log
       python3 scripts/triage-qemu-gles-stages.py --self-test

The trace is advisory. A successful stage trace is NOT a KMS receipt, a
client Wayland buffer release, a passed QEMU screenshot, or CI success.
"""
import argparse
from pathlib import Path
import sys
import unittest

PREFIX = "MELANIA_NATIVE_CLIENT_DMABUF_GLES_STAGE="
STAGES = (
    "cleanup_previous", "allocate", "compose", "finish",
    "finish_returned", "fence_completed", "download", "map",
    "cleanup_completed", "complete",
)


def analyze(lines):
    frames = []
    current = []
    errors = []
    for number, line in enumerate(lines, 1):
        if PREFIX not in line:
            continue
        fields = line.split(PREFIX, 1)[1].strip().split()
        if not fields:
            errors.append(f"line {number}: empty GLES stage marker")
            continue
        if len(fields) != 1:
            errors.append(f"line {number}: GLES stage marker has trailing tokens: {fields[1:]!r}")
            continue
        stage = fields[0]
        if stage not in STAGES:
            errors.append(f"line {number}: unknown stage {stage!r}")
            continue
        if stage == STAGES[0]:
            if current:
                frames.append(current)
            current = []
        elif not current:
            errors.append(f"line {number}: stage {stage} without frame start")
            continue
        expected = STAGES[len(current)] if len(current) < len(STAGES) else None
        if stage != expected:
            errors.append(
                f"line {number}: expected {expected!r}, observed {stage!r}"
            )
        current.append(stage)
    if current:
        frames.append(current)
    for index, stages in enumerate(frames, 1):
        if stages != list(STAGES):
            next_stage = STAGES[len(stages)] if len(stages) < len(STAGES) else None
            errors.append(
                f"frame {index}: incomplete/invalid; last={stages[-1]!r}; "
                f"next expected={next_stage!r}"
            )
    if len(frames) < 2:
        errors.append(f"only {len(frames)} GLES frame(s) observed; two required")
    return frames, errors


class Tests(unittest.TestCase):
    def test_two_complete_frames(self):
        lines = [PREFIX + stage for _ in range(2) for stage in STAGES]
        frames, errors = analyze(lines)
        self.assertEqual(len(frames), 2)
        self.assertEqual(errors, [])

    def test_second_frame_finish_crash(self):
        lines = [PREFIX + s for s in STAGES + STAGES[:4]]
        frames, errors = analyze(lines)
        self.assertEqual(len(frames), 2)
        self.assertTrue(any("frame 2:" in e and "finish" in e for e in errors))

    def test_missing_fence_cannot_pass(self):
        lines = [PREFIX + s for s in STAGES if s != "fence_completed"] * 2
        _, errors = analyze(lines)
        self.assertTrue(errors)

    def test_no_markers_fails_closed(self):
        _, errors = analyze(["GLES initialization failed"])
        self.assertTrue(errors)

    def test_empty_stage_fails_closed_without_exception(self):
        frames, errors = analyze([PREFIX, PREFIX + "   ", PREFIX + "complete"])
        self.assertEqual(frames, [])
        self.assertTrue(any("empty GLES stage marker" in error for error in errors))
        self.assertTrue(any("without frame start" in error for error in errors))

    def test_trailing_tokens_cannot_masquerade_as_complete_frames(self):
        lines = [PREFIX + stage + " unexpected" for _ in range(2) for stage in STAGES]
        frames, errors = analyze(lines)
        self.assertEqual(frames, [])
        self.assertTrue(any("trailing tokens" in error for error in errors))
        self.assertTrue(any("two required" in error for error in errors))

    def test_timestamped_single_stage_marker_remains_valid(self):
        lines = ["2026-11-21T20:00:00Z INFO " + PREFIX + stage for _ in range(2) for stage in STAGES]
        frames, errors = analyze(lines)
        self.assertEqual(len(frames), 2)
        self.assertEqual(errors, [])

    def test_malformed_marker_after_two_valid_frames_still_fails(self):
        lines = [PREFIX + stage for _ in range(2) for stage in STAGES]
        lines.append(PREFIX + "complete trailing")
        frames, errors = analyze(lines)
        self.assertEqual(len(frames), 2)
        self.assertTrue(any("trailing tokens" in error for error in errors))

    def test_third_frame_crash_after_two_valid_frames_still_fails(self):
        lines = [PREFIX + stage for _ in range(2) for stage in STAGES]
        lines.extend(PREFIX + stage for stage in STAGES[:5])
        frames, errors = analyze(lines)
        self.assertEqual(len(frames), 3)
        self.assertTrue(any("frame 3:" in error for error in errors))

    def test_unknown_stage_fails_closed(self):
        _, errors = analyze([PREFIX + "unknown"])
        self.assertTrue(errors)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", nargs="?", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(Tests)
        return 0 if unittest.TextTestRunner().run(suite).wasSuccessful() else 1
    if args.log is None:
        parser.error("provide a guest log or --self-test")
    try:
        with args.log.open(encoding="utf-8", errors="replace") as stream:
            frames, errors = analyze(stream)
    except OSError as exc:
        print(f"Cannot read log: {exc}", file=sys.stderr)
        return 2
    for i, frame in enumerate(frames, 1):
        print(f"frame {i}: {len(frame)}/{len(STAGES)} stages; last={frame[-1]}")
    for error in errors:
        print(f"FAIL: {error}", file=sys.stderr)
    if not errors:
        print("Stage markers complete for two frames; KMS/QMP/CI still require independent verification.")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
