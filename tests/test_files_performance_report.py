"""Unit tests for opt-in Files timing parser (no desktop dependencies)."""
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
SCRIPT=ROOT/"scripts/summarize-files-performance.py"
MODULE=runpy.run_path(str(SCRIPT))


class FilesPerformanceReport(unittest.TestCase):
    def test_mixed_process_prefixes_and_unknown_metrics(self):
        samples=MODULE["parse_timing_lines"]([
            "[files] MELANIA_FILES_CLEAR_MS=12 tiles=120 generation=5\n",
            "MELANIA_FILES_CLEAR_MS=42 tiles=200 generation=6\n",
            "MELANIA_FILES_SCAN_MS=7 path=/tmp/x\n",
            "MELANIA_FILES_UNKNOWN_MS=999\n",
            "OTHER_APP_CLEAR_MS=1000\n",
        ])
        self.assertEqual(samples["MELANIA_FILES_CLEAR_MS"],[12,42])
        self.assertEqual(samples["MELANIA_FILES_SCAN_MS"],[7])
        self.assertNotIn("MELANIA_FILES_UNKNOWN_MS",samples)

    def test_percentile_uses_nearest_rank(self):
        percentile=MODULE["percentile_nearest_rank"]
        self.assertEqual(percentile([25,2,12,9,100],50),12)
        self.assertEqual(percentile([25,2,12,9,100],95),100)
        self.assertIsNone(percentile([],95))

    def test_cli_reports_sample_counts_and_missing_data(self):
        with tempfile.TemporaryDirectory() as folder:
            path=Path(folder)/"vm.log"
            path.write_text("MELANIA_FILES_RENDER_MS=13 entries=100\n"
                            "MELANIA_FILES_RENDER_MS=29 entries=200\n")
            result=subprocess.run([sys.executable,str(SCRIPT),str(path)],
                text=True,capture_output=True,check=False)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertIn("RENDER_MS",result.stdout)
            self.assertIn("29",result.stdout)
            path.write_text("No Files metrics in this log\n")
            result=subprocess.run([sys.executable,str(SCRIPT),str(path)],
                text=True,capture_output=True,check=False)
            self.assertEqual(result.returncode,1)
            self.assertIn("No recognized",result.stdout)


if __name__=="__main__":
    unittest.main()
