#!/usr/bin/env python3
import struct, unittest
from audit import analyze

def sample(shared=False, collision=False):
    b = bytearray(b'RDM\x01' + struct.pack('<4I', 20, 0, 4, 28))
    b += struct.pack('<2I', 1, 48) + struct.pack('<12I', 84, 0, 0, 0, 0, 0, 164 if collision else 0, 0, 0, 0, 0, 0)
    b += struct.pack('<2I', 1, 72) + struct.pack('<18I', 164, 164 if shared else 0, *[0] * 16)
    b += struct.pack('<2I', 3, 1) + b'abc'
    return b

class AuditTests(unittest.TestCase):

    def test_complete_tree(self):
        r = analyze(sample())
        self.assertTrue(r['physical_complete'])
        self.assertTrue(r['tree_complete'])
        self.assertTrue(r['preorder'])

    def test_shared_payload_is_not_a_tree(self):
        r = analyze(sample(shared=True))
        self.assertTrue(r['physical_complete'])
        self.assertFalse(r['tree_complete'])
        self.assertIn('shared_blocks', r['metrics'])

    def test_scalar_collision_does_not_add_an_edge(self):
        a = analyze(sample())
        b = analyze(sample(collision=True))
        self.assertEqual(a['edges'], b['edges'])
        self.assertTrue(b['tree_complete'])

    def test_unreachable_block(self):
        b = sample() + struct.pack('<2I', 2, 1) + b'xy'
        r = analyze(b)
        self.assertTrue(r['physical_complete'])
        self.assertFalse(r['tree_complete'])
        self.assertEqual(len(r['unreached']), 1)

    def test_trailing_byte(self):
        self.assertFalse(analyze(sample() + b'\x00')['physical_complete'])

    def test_oversized_payload(self):
        b = sample()
        struct.pack_into('<I', b, 20, 4294967295)
        self.assertFalse(analyze(b)['physical_complete'])
if __name__ == '__main__':
    unittest.main()
