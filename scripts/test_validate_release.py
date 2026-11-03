"""Offline subprocess regressions for validate-release.sh.

Run with: python3 -m unittest discover -s scripts -p 'test_*.py' -v
Requires bash, jq, and GNU timeout (or gtimeout on macOS).
"""

import copy
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest


SCRIPT = Path(__file__).with_name("validate-release.sh")
PHASES = (
    "tools/list (stable)",
    "tools/list (2026-07-28)",
    "deployment probe",
    "search_crates (2026-07-28)",
)
TOOLS = [{
    "name": "search_crates",
    "outputSchema": {"type": "object", "required": ["crates", "meta"]},
    "annotations": {"readOnlyHint": True, "idempotentHint": True},
}]
RESULT = {
    "isError": False,
    "structuredContent": {
        "crates": [{"name": "tower-mcp"}],
        "meta": {"total": 1},
    },
    "content": [{"type": "text", "text": "tower-mcp"}],
    "_meta": {"io.modelcontextprotocol/serverInfo": {"version": "0.4.4"}},
}

# Both binaries are replaced, so these tests never contact the deployment or
# need mcp-repl / the TypeScript client installed. The timeout utility is real.
FAKE_COMMAND = r'''
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

root = Path(os.environ["VALIDATION_FIXTURE_DIR"])
config = json.loads((root / "config.json").read_text())
args = sys.argv[1:]
if Path(sys.argv[0]).name == "node":
    assert len(args) == 2 and args[0].endswith("validate-deployment.mjs"), args
    assert args[1] == "http://offline.invalid/", args
    phase = "deployment probe"
else:
    assert len(args) == 7, args
    assert args[:1] == ["--protocol"], args
    assert args[2:6] == ["--http", "http://offline.invalid/", "--json", "-e"], args
    if args[6] == "tools":
        phase = "tools/list (" + args[1] + ")"
    else:
        assert args[1] == "2026-07-28", args
        assert args[6] == "search_crates query=tower-mcp", args
        phase = "search_crates (2026-07-28)"

def record(**extra):
    with (root / "calls.jsonl").open("a") as log:
        log.write(json.dumps(dict(phase=phase, pid=os.getpid(),
                                  pgid=os.getpgrp(), **extra)) + "\n")

record()
mode = config.get("mode", "success") if config.get("phase") == phase else "success"
if mode == "hang_once":
    marker = root / "already_hung"
    mode = "success" if marker.exists() else "hang"
    marker.touch()
if mode == "resist_term":
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    # The child inherits the ignored signal and the timeout process group.
    child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
    record(child_pid=child.pid)
if mode in ("hang", "resist_term"):
    time.sleep(60)
elif mode == "fail":
    print("fixture subprocess failed", file=sys.stderr)
    sys.exit(42)
elif phase.startswith("tools/list"):
    print(json.dumps(config["tools"]))
elif phase.startswith("search_crates"):
    print(json.dumps(config["result"]))
else:
    print("Deployment validation passed (offline fixture)")
'''


class ReleaseValidationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        missing = [name for name in ("bash", "jq") if not shutil.which(name)]
        if not (shutil.which("timeout") or shutil.which("gtimeout")):
            missing.append("GNU timeout or gtimeout")
        if missing:
            raise RuntimeError("Missing test dependencies: " + ", ".join(missing))

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="release-validation-")
        self.killed_groups = set()
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for name in ("mcp-repl", "node"):
            executable = self.bin / name
            executable.write_text("#!" + sys.executable + "\n" + FAKE_COMMAND)
            executable.chmod(0o755)
        self.config = {"tools": copy.deepcopy(TOOLS), "result": copy.deepcopy(RESULT)}

    def tearDown(self):
        self.cleanup_processes()
        self.tmp.cleanup()

    def calls(self):
        path = self.root / "calls.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def cleanup_processes(self):
        # GNU timeout creates a separate process group. Kill recorded groups as
        # well as the outer shell if a regression trips the test's own deadline.
        for pgid in {call["pgid"] for call in self.calls()}:
            if pgid != os.getpgrp():
                self.kill_group(pgid)

    def kill_group(self, pgid):
        # Without the timeout wrapper the fixture shares the outer shell's
        # group. Do not signal that group twice during deadline cleanup, or
        # signal an old group again in tearDown after it has been reaped.
        if pgid not in self.killed_groups:
            try:
                os.killpg(pgid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            self.killed_groups.add(pgid)

    def run_validation(self, *, attempts=1, timeout="3", expected_version="v0.4.4"):
        (self.root / "config.json").write_text(json.dumps(self.config))
        env = os.environ.copy()
        env.update({
            "PATH": str(self.bin) + os.pathsep + env.get("PATH", ""),
            "VALIDATION_FIXTURE_DIR": str(self.root),
            "MCP_ENDPOINT": "http://offline.invalid/",
            "MCP_REPL_BIN": str(self.bin / "mcp-repl"),
            "MCP_VALIDATION_ATTEMPTS": str(attempts),
            "MCP_VALIDATION_RETRY_DELAY_SECS": "0",
            "MCP_VALIDATION_TIMEOUT_SECS": timeout,
            "EXPECTED_VERSION": expected_version,
        })
        started = time.monotonic()
        with subprocess.Popen(
            ["bash", str(SCRIPT)], env=env, text=True,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
        ) as process:
            try:
                stdout, stderr = process.communicate(timeout=12)
            except subprocess.TimeoutExpired:
                self.kill_group(process.pid)
                self.cleanup_processes()
                stdout, stderr = process.communicate(timeout=3)
                self.fail("Validation exceeded the test deadline:\n" + stdout + stderr)
        return subprocess.CompletedProcess(process.args, process.returncode, stdout, stderr), time.monotonic() - started

    def test_success_preserves_all_four_phases_and_version_check(self):
        result, _ = self.run_validation()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("MCP release validation passed", result.stdout)
        self.assertIn('"version": "0.4.4"', result.stdout)
        self.assertEqual([call["phase"] for call in self.calls()], list(PHASES))

    def test_each_hung_phase_times_out_and_exhausts_retries(self):
        for phase in PHASES:
            with self.subTest(phase=phase):
                (self.root / "calls.jsonl").unlink(missing_ok=True)
                self.config.update(phase=phase, mode="hang")
                result, elapsed = self.run_validation(attempts=2, timeout="1")
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn(phase, result.stderr)
                self.assertIn("timed out", result.stderr)
                self.assertIn("1s", result.stderr)
                self.assertIn("attempt 2/2", result.stdout)
                self.assertIn("failed after 2 attempts", result.stderr)
                self.assertNotIn("MCP release validation passed", result.stdout)
                self.assertEqual(sum(call["phase"] == phase for call in self.calls()), 2)
                self.assertLess(elapsed, 8)
                self.cleanup_processes()

    def test_transient_timeout_can_recover_on_next_attempt(self):
        self.config.update(phase=PHASES[-1], mode="hang_once")
        result, _ = self.run_validation(attempts=2, timeout="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("timed out", result.stderr)
        self.assertIn("attempt 2/2", result.stdout)
        self.assertIn("MCP release validation passed", result.stdout)
        self.assertEqual([call["phase"] for call in self.calls()], list(PHASES) * 2)

    def test_term_resistant_command_and_descendant_are_killed(self):
        self.config.update(phase=PHASES[-1], mode="resist_term")
        result, elapsed = self.run_validation(timeout="1")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("timed out", result.stderr)
        self.assertIn(PHASES[-1], result.stderr)
        self.assertLess(elapsed, 10)
        hanging = [call for call in self.calls() if "child_pid" in call]
        self.assertEqual(len(hanging), 1)
        for pid in (hanging[0]["pid"], hanging[0]["child_pid"]):
            # A killed grandchild can briefly remain a zombie until PID 1 reaps
            # it on Linux; zombies cannot run or hold the command's output pipe.
            for _ in range(20):
                try:
                    os.kill(pid, 0)
                except ProcessLookupError:
                    break
                stat = Path(f"/proc/{pid}/stat")
                try:
                    if stat.read_text().split(")", 1)[1].lstrip().startswith("Z"):
                        break
                except FileNotFoundError:
                    pass
                time.sleep(0.05)
            else:
                self.fail(f"Subprocess {pid} survived timeout")

    def test_nonzero_subprocess_exit_is_a_failure_not_a_timeout(self):
        self.config.update(phase=PHASES[-1], mode="fail")
        result, _ = self.run_validation()
        self.assertEqual(result.returncode, 1)
        self.assertIn("fixture subprocess failed", result.stderr)
        self.assertIn("search_crates failed using the final lifecycle", result.stderr)
        self.assertNotIn("timed out", result.stderr)

    def test_schema_and_annotation_checks_still_reject_bad_tools(self):
        for field in ("required", "readOnlyHint", "idempotentHint"):
            with self.subTest(field=field):
                self.config["tools"] = copy.deepcopy(TOOLS)
                if field == "required":
                    self.config["tools"][0]["outputSchema"][field] = ["crates"]
                else:
                    self.config["tools"][0]["annotations"][field] = False
                result, _ = self.run_validation()
                self.assertEqual(result.returncode, 1)
                self.assertIn("missing its expected schema or annotations", result.stderr)

    def test_tool_result_checks_still_reject_bad_results(self):
        for defect in ("isError", "crates", "total", "text"):
            with self.subTest(defect=defect):
                self.config["result"] = copy.deepcopy(RESULT)
                fixture = self.config["result"]
                if defect == "isError":
                    fixture["isError"] = True
                elif defect == "crates":
                    fixture["structuredContent"]["crates"] = []
                elif defect == "total":
                    fixture["structuredContent"]["meta"]["total"] = 0
                else:
                    fixture["content"] = []
                result, _ = self.run_validation()
                self.assertEqual(result.returncode, 1)
                self.assertIn("unexpected tool result", result.stderr)

    def test_wrong_release_version_still_fails(self):
        result, _ = self.run_validation(expected_version="v0.4.5")
        self.assertEqual(result.returncode, 1)
        self.assertIn("live server version does not match release v0.4.5", result.stderr)

    def test_invalid_timeout_is_rejected_before_running_subprocesses(self):
        for timeout in ("0", "-1", "nonsense", "", "1.5"):
            with self.subTest(timeout=timeout):
                result, _ = self.run_validation(timeout=timeout)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("MCP_VALIDATION_TIMEOUT_SECS", result.stderr)
                self.assertEqual(self.calls(), [])


if __name__ == "__main__":
    unittest.main()
