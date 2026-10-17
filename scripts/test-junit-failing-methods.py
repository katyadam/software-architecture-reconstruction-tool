"""Regression checks for mutation-result attribution in benchmark scripts."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class JUnitFailuresTest(unittest.TestCase):
    def parse(self, xml):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / "report.xml"
            report.write_text(xml)
            return subprocess.run(
                [sys.executable, str(Path(__file__).with_name("junit-failing-methods.py")), str(report)],
                capture_output=True, text=True,
            )

    def test_failure_after_self_closing_pass_is_attributed_to_failed_case(self):
        result = self.parse('<testsuite><testcase name="passed"/><testcase name="failed"><failure/></testcase></testsuite>')
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertEqual(["failed"], json.loads(result.stdout))

    def test_namespace_and_error_are_supported(self):
        result = self.parse('<testsuite xmlns="urn:junit"><testcase name="broken"><error/></testcase><testcase name="skipped"><skipped/></testcase></testsuite>')
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertEqual(["broken"], json.loads(result.stdout))

    def test_malformed_report_fails_instead_of_reporting_no_failures(self):
        self.assertNotEqual(0, self.parse('<testsuite>').returncode)


if __name__ == "__main__":
    unittest.main()
