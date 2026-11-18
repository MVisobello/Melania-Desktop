"""The full QEMU runner must publish actual wait outcomes even on failure."""
import pathlib
import re
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]


class ObserverExitEvidenceTests(unittest.TestCase):
    def test_every_wait_status_is_reported_and_failure_is_not_hidden(self):
        source = (ROOT / "scripts/run-qemu-melania-native.sh").read_text()
        waits = re.findall(r'^wait "\$(\w+)"\n(\w+)=\$\?$', source, re.M)
        names = [status for _, status in waits]
        self.assertTrue(names)
        self.assertEqual(len(names), len(set(names)))
        start = source.index("for observer_status in ")
        stop = source.index("\ndone\n", start) + len("\ndone\n")
        snippet = source[start:stop]
        assignments = "\n".join(
            f"{name}={7 if name == 'retained_drag_code' else 0}"
            for name in names
        )
        with tempfile.TemporaryDirectory() as report:
            run = subprocess.run(
                ["bash", "-eu", "-c", assignments + "\nreport=$1\n" + snippet,
                 "observer-test", report],
                check=True, text=True, capture_output=True,
            )
            evidence = (pathlib.Path(report) / "result.txt").read_text()
        expected = [
            f"MELANIA_QEMU_OBSERVER_EXIT={name} code={7 if name == 'retained_drag_code' else 0}"
            for name in names
        ]
        self.assertEqual(evidence.splitlines(), expected)
        self.assertEqual(run.stdout.splitlines(), expected)
        self.assertIn('"$retained_drag_code" -ne 0', source[stop:])
        self.assertIn("MELANIA_QEMU_NATIVE_RESULT=FAIL", source[stop:])


if __name__ == "__main__":
    unittest.main()
