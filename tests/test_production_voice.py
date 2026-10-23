from __future__ import annotations

import tempfile
import unittest
import wave
from pathlib import Path

from pipeline.production import spec as prod_spec
from pipeline.production import voice


def _wav_bytes(seconds: float, rate: int = 24000) -> bytes:
    with tempfile.NamedTemporaryFile(suffix=".wav") as fh:
        with wave.open(fh.name, "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(rate)
            w.writeframes(b"\x00\x00" * int(seconds * rate))
        return Path(fh.name).read_bytes()


class FakeVoicebox:
    def __init__(self, seconds_per_char: float = 0.05) -> None:
        self.calls: list[tuple[str, str, str]] = []
        self.seconds_per_char = seconds_per_char

    def generate(self, text: str, profile_id: str, instruct: str = "") -> bytes:
        self.calls.append((text, profile_id, instruct))
        return _wav_bytes(len(text) * self.seconds_per_char)


def _spec(tmp: Path) -> Path:
    shot = prod_spec.Shot("01a", 2.0, lines=[
        prod_spec.Line("01-01", "A", "Hello there friend."),           # 19 chars -> 0.95 s
        prod_spec.Line("01-02", "B", "Hi.", direction="off")])          # 3 chars -> 0.15 s
    spec = prod_spec.Spec("T", 30, 1920, 1080,
                          [prod_spec.Chapter("01", "Open", "", 0.0, 2.0, [shot])])
    path = tmp / "production.json"
    prod_spec.save_spec(spec, path)
    return path


class TestVoice(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.profiles = {"A": "pa", "B": "pb"}

    def test_generate_writes_wav_and_measures_duration(self) -> None:
        path = _spec(self.root)
        lines = voice.generate_lines(path, self.root / "vo", self.profiles, client=FakeVoicebox())
        self.assertEqual([l.wav for l in lines], ["vo/01-01.wav", "vo/01-02.wav"])
        self.assertAlmostEqual(lines[0].duration_s, 0.95, places=2)
        saved = prod_spec.load_spec(path).chapters[0].shots[0].lines
        self.assertAlmostEqual(saved[1].duration_s, 0.15, places=2)

    def test_shot_duration_grows_when_lines_need_more_time(self) -> None:
        path = _spec(self.root)
        voice.generate_lines(path, self.root / "vo", self.profiles, client=FakeVoicebox(0.2))
        shot = prod_spec.load_spec(path).chapters[0].shots[0]
        # 22 chars * 0.2 s = 4.4 s of speech + 2 * 0.4 s breathing room
        self.assertAlmostEqual(shot.duration_s, 5.2, places=2)

    def test_existing_wavs_are_reused_unless_requested(self) -> None:
        path = _spec(self.root)
        fake = FakeVoicebox()
        voice.generate_lines(path, self.root / "vo", self.profiles, client=fake)
        voice.generate_lines(path, self.root / "vo", self.profiles, client=fake)
        self.assertEqual(len(fake.calls), 2)
        voice.generate_lines(path, self.root / "vo", self.profiles, client=fake, only=["01-02"])
        self.assertEqual(len(fake.calls), 3)

    def test_unknown_speaker_raises_before_generating(self) -> None:
        path = _spec(self.root)
        fake = FakeVoicebox()
        with self.assertRaisesRegex(KeyError, "B"):
            voice.generate_lines(path, self.root / "vo", {"A": "pa"}, client=fake)
        self.assertEqual(fake.calls, [])


if __name__ == "__main__":
    unittest.main()
