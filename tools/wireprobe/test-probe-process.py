#!/usr/bin/env python3
"""Small synthetic process-contract checks, no toolchain/corpus campaign."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

HELPER = Path(__file__).with_name("probe-process.py")


class ProcessContract(unittest.TestCase):
    def test_child_descriptor_budget_is_bounded(self):
        result = self.invoke('import resource; s,h=resource.getrlimit(resource.RLIMIT_NOFILE); assert 0 < s <= h <= 65536, (s,h)')
        self.assertEqual(result.returncode, 0, result.stderr)

    def command(self, source, timeout="5", grace="3"):
        return [sys.executable, str(HELPER), "--timeout", timeout, "--grace", grace, "--",
                sys.executable, "-c", source]

    def invoke(self, source, timeout="5"):
        return subprocess.run(self.command(source, timeout), capture_output=True, text=True, timeout=12)

    def test_domain_failure_preserved_but_signal_is_infrastructure(self):
        self.assertEqual(self.invoke("raise SystemExit(3)").returncode, 3)
        result = self.invoke("import os, signal; os.kill(os.getpid(), signal.SIGKILL)")
        self.assertEqual(result.returncode, 125)

    def test_spawn_failure_is_infrastructure(self):
        result = subprocess.run([sys.executable, str(HELPER), "--timeout", "2", "--",
                                 "/no-such-endpoint-program"], capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 125)

    @unittest.skipUnless(sys.platform == "linux", "Linux subreaper contract")
    def test_finished_orphan_is_reaped_not_reported_as_live_leak(self):
        # Erlang terminates auxiliary processes (including with signals)
        # during normal VM shutdown. Only the VM owns their exit semantics.
        for ending in ("os._exit(0)", "os.kill(os.getpid(), signal.SIGUSR1)"):
            with self.subTest(ending=ending):
                child_branch = "if pid == 0:\n    " + ending
                source = ("import os,signal,time; pid=os.fork(); "
                          f"exec({child_branch!r}); "
                          "time.sleep(0.1); os._exit(0)")
                result = self.invoke(source)
                self.assertEqual(result.returncode, 0, result.stderr)

    @unittest.skipUnless(sys.platform == "linux", "Linux subreaper contract")
    def test_live_detached_orphan_is_failure_and_is_reaped(self):
        with tempfile.TemporaryDirectory() as directory:
            pidfile = Path(directory) / "detached.pid"
            leaf = ("import os,time; from pathlib import Path; "
                    f"Path({str(pidfile)!r}).write_text(str(os.getpid())); time.sleep(60)")
            source = ("import subprocess,sys,time; "
                      f"subprocess.Popen([sys.executable,'-c',{leaf!r}],start_new_session=True); "
                      "time.sleep(0.1)")
            result = self.invoke(source)
            self.assertEqual(result.returncode, 125, result.stderr)
            self.assertIn("child left descendants", result.stderr)
            with self.assertRaises(ProcessLookupError):
                os.kill(int(pidfile.read_text()), 0)

    def test_timeout_kills_nested_supervisor_and_grandchild(self):
        with tempfile.TemporaryDirectory() as directory:
            pidfile = Path(directory) / "child.pid"
            leaf = ("import os,signal,time; from pathlib import Path; "
                    f"Path({str(pidfile)!r}).write_text(str(os.getpid())); "
                    "signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(60)")
            nested = self.command(leaf, "30", "1")
            parent = f"import subprocess; subprocess.run({nested!r})"
            result = self.invoke(parent, "2")
            self.assertEqual(result.returncode, 125, result.stderr)
            pid = int(pidfile.read_text())
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                try:
                    os.kill(pid, 0)
                except ProcessLookupError:
                    break
                time.sleep(0.05)
            else:
                self.fail(f"owned grandchild {pid} survived cleanup")

    def test_parent_signal_stops_owned_child(self):
        with tempfile.TemporaryDirectory() as directory:
            pidfile = Path(directory) / "child.pid"
            source = ("import os,time; from pathlib import Path; "
                      f"Path({str(pidfile)!r}).write_text(str(os.getpid())); time.sleep(60)")
            process = subprocess.Popen(self.command(source, "30"), stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                deadline = time.monotonic() + 5
                while not pidfile.exists() and time.monotonic() < deadline:
                    time.sleep(0.05)
                self.assertTrue(pidfile.exists())
                process.send_signal(signal.SIGTERM)
                _, error = process.communicate(timeout=8)
                self.assertEqual(process.returncode, 125, error)
                with self.assertRaises(ProcessLookupError):
                    os.kill(int(pidfile.read_text()), 0)
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=8)

    def test_synthetic_corpus_complete_or_preserves_previous_output(self):
        # Fake generator/compiler only: this checks collection mechanics, not
        # a vendor codec. The ten cells echo their existing reference bytes.
        root = HELPER.resolve().parents[2]
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            binaries = work / "bin"
            binaries.mkdir()
            cargo = binaries / "cargo"
            cargo.write_text("#!/bin/sh\nexit 0\n")
            cargo.chmod(0o755)
            target = work / "target"
            (target / "debug").mkdir(parents=True)
            generator = target / "debug/wireprobe-endpoint-gen"
            generator.write_text(
                f"#!{sys.executable}\nimport os,sys\nfrom pathlib import Path\n"
                "if os.environ.get('SYNTHETIC_FAILURE'): raise SystemExit(137)\n"
                "args=dict(zip(sys.argv[1::2],sys.argv[2::2]))\n"
                f"root=Path({str(root)!r})\n"
                "ref=root/'tests/interop-results'/Path(args['--idl']).parent.name/'zerodds-rust'/"
                "(args['--encoding']+'.'+args['--endian']+'.bin')\n"
                "program='from pathlib import Path\\nPath(\"out.bin\").write_bytes('+repr(ref.read_bytes())+')\\n'\n"
                "(Path(args['--out-dir'])/'probe.py').write_text(program)\n")
            generator.chmod(0o755)
            env = dict(os.environ, PATH=f"{binaries}:{os.environ['PATH']}",
                       CARGO_TARGET_DIR=str(target), WIREPROBE_ENDPOINT_GEN=str(generator),
                       ZERODDS_ENDPOINT_CACHE=str(work / "cache"),
                       PYTHON=sys.executable, ENDPOINT_SWEEP_TIMEOUT="30", ENDPOINT_CELL_TIMEOUT="15")
            output = work / "results.tsv"
            command = ["bash", str(HELPER.with_name("wireprobe-endpoint-corpus")),
                       "--langs", "python", "--cases", "string-unbounded-ascii", "--out", str(output)]
            result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=40)
            self.assertEqual(result.returncode, 0, result.stderr)
            before = output.read_bytes()
            rows = before.decode().splitlines()
            self.assertEqual(len(rows), 10)
            self.assertTrue(all(row.split('\t')[4] == 'same' for row in rows), result.stderr + '\n' + '\n'.join(rows))
            env['SYNTHETIC_FAILURE'] = '1'
            failed = subprocess.run(command, env=env, capture_output=True, text=True, timeout=40)
            self.assertEqual(failed.returncode, 125, failed.stderr)
            self.assertEqual(output.read_bytes(), before)
            invalid = subprocess.run(command + ['--jobs', '0'], env=env, capture_output=True, timeout=5)
            self.assertEqual(invalid.returncode, 2)


if __name__ == "__main__":
    unittest.main()
