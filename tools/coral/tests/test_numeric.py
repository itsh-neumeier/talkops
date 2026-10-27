"""Tests for the numpy modules of tools/coral. Skipped without numpy."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
try:
    import numpy as np
except ImportError:  # the CI job runs without numpy
    np = None


@unittest.skipIf(np is None, "numpy not installed")
class AudioTest(unittest.TestCase):
    def test_wav_roundtrip_and_resample(self):
        from coraltools import audio

        x = (0.5 * np.sin(2 * np.pi * 440 * np.arange(16000) / 16000)).astype(np.float32)
        with tempfile.TemporaryDirectory() as d:
            audio.write_wav(f"{d}/a.wav", x, 16000)
            y, rate = audio.read_wav(f"{d}/a.wav")
        self.assertEqual(rate, 16000)
        self.assertLess(np.abs(x - y).max(), 1e-3)
        z = audio.resample(x, 16000, 8000)
        self.assertEqual(len(z), 8000)
        self.assertAlmostEqual(float(np.sqrt((z[500:-500] ** 2).mean())), 0.5 / np.sqrt(2), delta=0.02)

    def test_telephone_removes_high_band(self):
        from coraltools import audio

        t = np.arange(16000) / 16000
        low = 0.3 * np.sin(2 * np.pi * 1000 * t)
        high = 0.3 * np.sin(2 * np.pi * 6000 * t)
        for codec in ("pcmu", "pcma"):
            self.assertGreater(np.std(audio.telephone(low.astype(np.float32), 16000, codec)[800:-800]), 0.15)
            self.assertLess(np.std(audio.telephone(high.astype(np.float32), 16000, codec)[800:-800]), 0.02)

    def test_g711_roundtrip_error_is_small(self):
        from coraltools import audio

        x = np.linspace(-0.9, 0.9, 1001).astype(np.float32)
        self.assertLess(np.abs(audio.mulaw_roundtrip(x) - x).max(), 0.03)
        self.assertLess(np.abs(audio.alaw_roundtrip(x) - x).max(), 0.03)


@unittest.skipIf(np is None, "numpy not installed")
class FeaturesTest(unittest.TestCase):
    def test_shape_and_normalisation(self):
        from coraltools.features import Frontend, log_mel

        fe = Frontend()
        x = np.random.default_rng(0).normal(0, 0.1, 25500).astype(np.float32)
        f = log_mel(x, fe)
        self.assertEqual(f.shape, (64, fe.frames(len(x))))
        self.assertLess(np.abs(f.mean(axis=1)).max(), 1e-3)

    def test_filterbank_rows_are_triangles(self):
        from coraltools.features import mel_filterbank

        fb = mel_filterbank(16000, 512, 64)
        self.assertEqual(fb.shape, (64, 257))
        self.assertTrue((fb >= 0).all())
        self.assertTrue((fb.sum(axis=1) > 0).all())


@unittest.skipIf(np is None, "numpy not installed")
class StreamTest(unittest.TestCase):
    """Stream and Turn with a fake model in place of the TFLite interpreter."""

    def fake(self, language, letter):
        from coraltools.features import Frontend
        from coraltools.runtime import CtcModel

        class Fake(CtcModel):
            calls = 0

            def __post_init__(self):
                self.fe = Frontend()
                self.window, self.overlap, self.subsampling = 64, 16, 2
                self.labels = [" ", letter, "<blank>"]
                self.blank = 2
                self.language = language
                self.layout = "nct"

            def window_logits(self, samples):
                Fake.calls += 1
                y = np.full((self.window // 2, 3), -5.0, dtype=np.float32)
                y[:, 2] = -0.1
                y[::8, 1] = -0.01  # one letter every 8 output frames
                return y

        return Fake(path="", tpu=None, meta={"labels": [" ", letter], "window_frames": 64})

    def test_stream_runs_windows_while_audio_arrives(self):
        m = self.fake("de", "a")
        s = m.stream()
        for _ in range(10):
            s.push(np.zeros(1600, dtype=np.float32))  # 100 ms
        before = type(m).calls
        self.assertGreater(before, 0)
        text, words, conf = s.finish()
        self.assertTrue(set(text) <= {"a"})
        self.assertGreater(len(text), 5)
        self.assertLessEqual(type(m).calls - before, 2)
        self.assertGreater(conf, -1)

    def test_recognizer_modes(self):
        from coraltools.runtime import Recognizer

        de, en = self.fake("de", "d"), self.fake("en", "e")
        x = np.zeros(16000, dtype=np.float32)
        t = Recognizer({"de": de, "en": en}, "fixed", preferred="en").turn()
        t.push(x)
        self.assertEqual(t.finish()["language"], "en")
        t = Recognizer({"de": de, "en": en}, "confidence", preferred="de").turn()
        t.push(x)
        r = t.finish()
        self.assertIn(r["language"], ("de", "en"))

        class Lid:
            window_samples = 4000

            def classify(self, samples):
                return "en", 0.9

        t = Recognizer({"de": de, "en": en}, "classifier", Lid(), preferred="de").turn()
        t.push(x[:1000])
        self.assertIsNone(t.language)
        t.push(x[1000:])
        r = t.finish()
        self.assertEqual((r["language"], t.language), ("en", "en"))
        self.assertTrue(set(r["text"]) <= {"e"})


if __name__ == "__main__":
    unittest.main()
