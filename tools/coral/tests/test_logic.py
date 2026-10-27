"""Unit tests for the stdlib-only modules of tools/coral (run in CI)."""
import math
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from coraltools import compiler_log, ctc, devices, gate, turns, wer  # noqa: E402


class WerTest(unittest.TestCase):
    def test_normalizes_and_counts(self):
        r = wer.ErrorRate()
        r.add("Guten Tag, Frau Müller!", "guten tag frau muller")
        self.assertEqual(r.words, 4)
        self.assertEqual(r.word_edits, 1)
        self.assertAlmostEqual(r.wer, 0.25)
        self.assertGreater(r.cer, 0)

    def test_empty_reference(self):
        self.assertIsNone(wer.ErrorRate().wer)

    def test_edit_distance(self):
        self.assertEqual(wer.edit_distance("kitten", "sitting"), 3)


class CtcTest(unittest.TestCase):
    def test_windows_cover_input(self):
        self.assertEqual(ctc.windows(100, 40, 10), [(0, 40), (30, 70), (60, 100)])
        self.assertEqual(ctc.windows(10, 40, 10), [(0, 40)])
        with self.assertRaises(ValueError):
            ctc.windows(10, 4, 4)

    def test_stitch_keeps_each_frame_once(self):
        size, overlap, sub, total = 40, 8, 2, 100
        outs = []
        for start, end in ctc.windows(total, size, overlap):
            outs.append([[float(start + 2 * k)] for k in range(size // sub)])
        joined = [row[0] for row in ctc.stitch(outs, size, overlap, total, sub)]
        self.assertEqual(joined, [float(v) for v in range(0, total, sub)])

    def test_greedy_collapses_and_times(self):
        labels = [" ", "a", "b", "<blank>"]
        ids = [1, 1, 3, 1, 0, 2, 2, 3]
        frames = [[1.0 if i == k else 0.0 for i in range(4)] for k in ids]
        text, words = ctc.greedy(frames, labels, blank=3, frame_secs=0.02)
        self.assertEqual(text, "aa b")
        self.assertEqual(words[0][0], "aa")
        self.assertAlmostEqual(words[1][1], 0.10)

    def test_sentencepiece_words(self):
        labels = ["▁gu", "ten", "▁tag", "<blank>"]
        text, _ = ctc.collapse([0, 1, 3, 2], labels, blank=3)
        self.assertEqual(text, "guten tag")

    def test_confidence_prefers_sharper_output(self):
        sharp = [[5.0, 0.0, 0.0]] * 4
        flat = [[1.0, 0.8, 0.0]] * 4
        self.assertGreater(ctc.confidence(sharp, 2), ctc.confidence(flat, 2))
        self.assertEqual(ctc.confidence([[0.0, 0.0, 9.0]], 2), -math.inf)


class TurnsTest(unittest.TestCase):
    """Same scenarios as the tests of `Turns` in realtime.rs."""

    RATE = 8000

    def tone(self, secs):
        return [math.sin(i * 0.3) * 0.3 for i in range(int(secs * self.RATE))]

    def silence(self, secs):
        return [0.0] * int(secs * self.RATE)

    def test_detects_turns_with_lead_in_and_times(self):
        t = turns.Turns(self.RATE, time=10.0)
        audio = self.silence(1.0) + self.tone(2.0) + self.silence(1.0)
        actions = []
        for i in range(0, len(audio), 1234):
            actions += t.push(audio[i:i + 1234])
        commits = [a for a in actions if isinstance(a, turns.Commit)]
        self.assertEqual(len(commits), 1)
        self.assertAlmostEqual(commits[0].start, 10.7, delta=0.03)
        self.assertAlmostEqual(commits[0].end, 13.7, delta=0.03)
        sent = sum(len(a.samples) for a in actions if isinstance(a, turns.Append))
        self.assertLess(sent, int(3.1 * self.RATE))
        self.assertIsNone(t.finish())

    def test_splits_long_speech_and_drops_clicks(self):
        t = turns.Turns(self.RATE)
        actions = t.push(self.tone(40.0))
        self.assertEqual(len([a for a in actions if isinstance(a, turns.Commit)]), 2)
        self.assertIsInstance(t.finish(), turns.Commit)
        t = turns.Turns(self.RATE)
        actions = t.push(self.tone(0.06) + self.silence(1.0))
        self.assertFalse(any(isinstance(a, turns.Commit) for a in actions))
        self.assertTrue(any(isinstance(a, turns.Clear) for a in actions))


LOG_PARTIAL = """Edge TPU Compiler version 16.0.384591198
Started a compilation timeout timer of 180 seconds.

Model compiled successfully in 812 ms.

Input model: model.tflite
Input size: 9.81MiB
Output model: model_edgetpu.tflite
Output size: 9.95MiB
On-chip memory used for caching model parameters: 7.12MiB
On-chip memory remaining for caching model parameters: 0.00B
Off-chip memory used for streaming uncached model parameters: 2.61MiB
Number of Edge TPU subgraphs: 1
Total number of operations: 214
Operation log: model_edgetpu.log

Model successfully compiled but not all operations are supported by the Edge TPU.
Number of operations that will run on Edge TPU: 210
Number of operations that will run on CPU: 4

Operator                       Count      Status

CONV_2D                        120        Mapped to Edge TPU
LOG_SOFTMAX                    1          Operation not supported
TRANSPOSE                      3          More than one subgraph is not supported
"""

LOG_FULL = """Edge TPU Compiler version 16.0.384591198
On-chip memory used for caching model parameters: 210.50KiB
Off-chip memory used for streaming uncached model parameters: 0.00B
Number of Edge TPU subgraphs: 1
Total number of operations: 20
"""


class CompilerLogTest(unittest.TestCase):
    def test_partial_mapping(self):
        r = compiler_log.parse(LOG_PARTIAL)
        self.assertEqual(r["compiler_version"], "16.0.384591198")
        self.assertEqual((r["ops_total"], r["ops_tpu"], r["ops_cpu"]), (214, 210, 4))
        self.assertAlmostEqual(r["tpu_share"], 210 / 214)
        self.assertEqual(r["off_chip_bytes"], int(2.61 * 1024**2))
        self.assertEqual([o["op"] for o in r["operators"]], ["CONV_2D", "LOG_SOFTMAX", "TRANSPOSE"])
        self.assertEqual(r["operators"][1]["status"], "Operation not supported")

    def test_full_mapping(self):
        r = compiler_log.parse(LOG_FULL)
        self.assertEqual((r["ops_tpu"], r["ops_cpu"], r["tpu_share"]), (20, 0, 1.0))
        self.assertEqual(r["on_chip_bytes"], int(210.5 * 1024))


class DevicesTest(unittest.TestCase):
    def make(self, root, name, vid, pid, bus, dev):
        d = Path(root) / name
        d.mkdir()
        for k, v in (("idVendor", vid), ("idProduct", pid), ("busnum", bus), ("devnum", dev), ("speed", "5000")):
            (d / k).write_text(v + "\n")

    def test_usb_states_and_pcie(self):
        with tempfile.TemporaryDirectory() as sysfs, tempfile.TemporaryDirectory() as dev:
            self.make(sysfs, "2-1", "18d1", "9302", "2", "7")
            self.make(sysfs, "2-2", "1a6e", "089a", "2", "3")
            self.make(sysfs, "1-1", "046d", "c52b", "1", "2")
            (Path(dev) / "apex_0").write_text("")
            found = devices.discover(Path(sysfs), Path(dev))
        self.assertEqual([d["kind"] for d in found], ["usb", "usb", "pcie"])
        self.assertEqual(found[0]["state"], "ready")
        self.assertEqual(found[0]["node"], "/dev/bus/usb/002/007")
        self.assertEqual(found[1]["state"], "firmware-not-loaded")
        self.assertEqual([d["delegate_device"] for d in found], ["usb:0", "usb:1", "pci:0"])


class GateTest(unittest.TestCase):
    LIVE = {"calls": 1, "backlog_secs": 0.1, "latency_p50": 0.4, "latency_p95": 0.9,
            "cpu_cores": 0.3, "wer": 0.3}

    def test_pass(self):
        e = gate.evaluate(self.LIVE, {"cpu_cores": 1.5}, True)
        self.assertEqual(e["result"], "PASS")
        self.assertIn("Gate result: **PASS**", gate.markdown(e))

    def test_pending_without_rating_or_baseline(self):
        self.assertEqual(gate.evaluate(self.LIVE, None, None)["result"], "PENDING")

    def test_fail_beats_pending(self):
        slow = {**self.LIVE, "latency_p95": 2.4}
        self.assertEqual(gate.evaluate(slow, None, None)["result"], "FAIL")
        self.assertEqual(gate.evaluate(self.LIVE, {"cpu_cores": 0.4}, True)["result"], "FAIL")

    def test_cpu_is_compared_per_channel(self):
        three_calls = {**self.LIVE, "calls": 3, "channels": 6, "cpu_cores": 0.9}
        e = gate.evaluate(three_calls, {"cpu_cores": 1.5, "channels": 2}, True)
        self.assertEqual(e["checks"][2]["status"], "PASS")
        self.assertIn("ratio 0.2", e["checks"][2]["value"])

    def test_replace_section(self):
        doc = "a\n<!-- x:begin -->\nold\n<!-- x:end -->\nb"
        self.assertEqual(gate.replace_section(doc, "<!-- x:begin -->", "<!-- x:end -->", "new"),
                         "a\n<!-- x:begin -->\nnew\n<!-- x:end -->\nb")


if __name__ == "__main__":
    unittest.main()
