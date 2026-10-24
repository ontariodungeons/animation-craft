import unittest

from pipeline.lyrics_layout import designs

RECIPE = {
    "frame": [1920, 1080], "band": "top", "max_heroes": 2,
    "palette": {"ink": "#173F49", "rust": "#B8442A", "cream": "#FFF8E9"},
    "base": {"lettering": "sans-bold", "size": 58, "color": "ink", "outline": "cream"},
    "heroes": {"never": {"lettering": "serif-bold", "size": 112, "color": "rust", "motion": "pop"},
               "sea": {"lettering": "serif-italic", "size": 110, "color": "ink", "motion": "settle", "stroke": "hill"},
               "deep": {"lettering": "serif-italic", "size": 104, "color": "ink", "motion": "fall"}},
}


def words(line, texts, starts):
    return [{"word_id": f"{line}/w{i}", "line_id": line, "text": t, "start": s} for i, (t, s) in enumerate(zip(texts, starts))]


class LyricsLayoutTest(unittest.TestCase):
    def test_every_word_is_placed_inside_the_frame_with_heroes_acting(self):
        line = words("v3-l1", ["Oh", "the", "captain", "says", "never", "never", "do,", "deep", "blue", "sea"],
                     [None, 48.3, 48.7, 49.7, 50.8, 51.6, 52.1, 52.6, 53.0, 53.4])
        d = designs(RECIPE, [{"animation_id": "words-verse-3-1", "section": "verse_3", "words": line}])["words-verse-3-1"]["v3-l1"]
        placed = [i for it in d["items"] for i in it["word_ids"]]
        self.assertEqual(placed, [w["word_id"] for w in line], "every printed word once, in order")
        heroes = [it for it in d["items"] if it["motion"] != "quiet"]
        self.assertEqual([it["word_ids"] for it in heroes], [["v3-l1/w4", "v3-l1/w5"], ["v3-l1/w7"]], "repeats merge; at most two heroes")
        for it in d["items"]:
            self.assertGreaterEqual(it["x"], 0)
            self.assertLess(it["y"], 400, "the top band")
        self.assertEqual(d["draws"], [], "no hero with a stroke made the cut")

    def test_a_hero_stroke_draws_on_its_sung_beat(self):
        line = words("v5-l2", ["The", "deep", "blue", "sea"], [82.75, 83.09, 83.57, 84.25])
        d = designs({**RECIPE, "max_heroes": 1, "heroes": {"sea": RECIPE["heroes"]["sea"]}},
                    [{"animation_id": "a", "section": "verse_5", "words": line}])["a"]["v5-l2"]
        self.assertEqual(len(d["draws"]), 1)
        self.assertAlmostEqual(d["draws"][0]["paths"][0]["from"], 1.5)


if __name__ == "__main__":
    unittest.main()
