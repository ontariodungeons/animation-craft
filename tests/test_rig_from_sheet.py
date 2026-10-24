import tempfile
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

from pipeline.rig_from_sheet import trimmed


class TrimmedCellTest(unittest.TestCase):
    def test_neighbouring_cells_sliver_at_the_cell_edge_is_dropped(self):
        rgba = np.zeros((100, 100, 4), np.uint8)
        rgba[40:80, 30:70] = [200, 50, 50, 255]   # the cell's own drawing
        rgba[10:14, 20:60] = [90, 60, 30, 255]    # a neighbour's boots crossing the cell's top line
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'cell.png'
            Image.fromarray(rgba).save(path)
            kept = trimmed(path, cell=[0, 10, 100, 100])
            whole = trimmed(path)
        self.assertEqual(kept.shape[:2], (40, 40))
        self.assertEqual(whole.shape[:2], (70, 50))


if __name__ == '__main__':
    unittest.main()
