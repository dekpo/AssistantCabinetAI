"""Tests for scripts/peak_memory.py. Run from scripts/: python -m unittest test_peak_memory"""

import sys
import unittest

import peak_memory


class PeakMemory(unittest.TestCase):
    def test_a_child_that_holds_150_megabytes_reports_at_least_that_peak(self):
        code = "import time; data = bytearray(150 * 1024 * 1024); data[::4096] = b'x' * len(data[::4096]); time.sleep(0.6)"
        result = peak_memory.run([sys.executable, "-c", code])
        self.assertEqual(result.exit_code, 0)
        self.assertGreaterEqual(result.peak_bytes, 150 * 1024 * 1024)
        self.assertLess(result.peak_bytes, 1024 * 1024 * 1024)

    def test_the_exit_code_of_the_child_is_returned(self):
        result = peak_memory.run([sys.executable, "-c", "raise SystemExit(3)"])
        self.assertEqual(result.exit_code, 3)

    def test_the_environment_is_passed_to_the_child(self):
        result = peak_memory.run([sys.executable, "-c", "import os; raise SystemExit(int(os.environ['PEAK_TEST']))"], {"PEAK_TEST": "5"})
        self.assertEqual(result.exit_code, 5)


if __name__ == "__main__":
    unittest.main()
