#!/usr/bin/env python3
"""Classify extra forward candidates; pass the audit result directory.

Bone scalar range includes bounds recovered in the executable analysis.
Overlaps are classified by intersecting known fields, not file alignment.
Each file count must reproduce the saved inspector candidate count.
"""
import sys, struct, re, csv, json, collections, bisect
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from audit import LAYOUTS
SCALARS = {'frames': [(0, 32)], 'components': [(0, 16)], 'model': [(24, 52)], 'vertex_link': [(4, 8)], 'index_format': [(0, 4)], 'tri_groups': [(0, 12)], 'bones': [(4, 60)], 'animation': [(8, 16)]}

def inspect(path):
    b = path.read_bytes()
    endian = '<' if b[3] else '>'
    u = lambda a: struct.unpack_from(endian + 'I', b, a)[0]
    blocks = {}
    h = 20
    while h < len(b):
        n, s = struct.unpack_from(endian + 'II', b, h)
        blocks[h + 8] = (n, s)
        h += 8 + n * s
    types = {}
    pointers = set()
    todo = [(u(16), 'root')]
    while todo:
        p, t = todo.pop()
        if p in types:
            continue
        types[p] = t
        n, s = blocks[p]
        for i in range(n):
            for off, child in LAYOUTS.get(t, []):
                a = p + i * s + off
                pointers.add(a)
                q = u(a)
                if q:
                    todo.append((q, child))
    starts = list(blocks)
    pat = re.compile(b'(?=(' + b'|'.join((re.escape(struct.pack(endian + 'I', p)) for p in starts)) + b'))')
    rows = []
    for match in pat.finditer(b):
        a = match.start()
        k = bisect.bisect_right(starts, a) - 1
        if k < 0:
            continue
        p = starts[k]
        n, s = blocks[p]
        off = (a - p) % s
        if a + 4 > p + n * s or off + 4 > s or a in pointers:
            continue
        q = u(a)
        if q <= p:
            continue
        t = types[p]
        cat = 'unknown'
        detail = 'unmapped bytes'
        ranges = SCALARS.get(t, [])
        if t in ('vertices', 'indices'):
            ranges = [(0, s)]
        if any((lo <= off and off + 4 <= hi for lo, hi in ranges)):
            cat = 'scalar'
            detail = t
        elif any((off < po + 4 and off + 4 > po for po, _ in LAYOUTS.get(t, []))):
            cat = 'overlaps_pointer'
            detail = t
        elif any((off < hi and off + 4 > lo for lo, hi in ranges)):
            cat = 'overlaps_scalar'
            detail = t
        rows.append(dict(file=str(path), source_payload=p, source_type=t, record=(a - p) // s, field_offset=off, file_offset=a, target_payload=q, target_type=types[q], category=cat, detail=detail, bytes=b[a:a + 4].hex()))
    return rows

def main():
    ROOT = Path(sys.argv[1])
    allrows = []
    for corpus in ['anno2070', 'anno1800']:
        rows = list(csv.DictReader((ROOT / (corpus + '-inspector.tsv')).open(), delimiter='\t'))
        selected = [r for r in rows if int(r['forward_candidates']) > int(r['blocks']) - 1]
        for i, r in enumerate(selected):
            out = inspect(Path(r['file']))
            expected = int(r['forward_candidates']) - (int(r['blocks']) - 1)
            assert len(out) == expected, (r['file'], len(out), expected)
            for x in out:
                x['corpus'] = corpus
            allrows += out
            if i % 100 == 0:
                print(corpus, i, len(selected), flush=True)
    summary = collections.Counter(((r['corpus'], r['category'], r['source_type']) for r in allrows))
    (ROOT / 'extra-matches.jsonl').write_text(''.join((json.dumps(x) + '\n' for x in allrows)))
    (ROOT / 'extra-matches-summary.json').write_text(json.dumps({'total': len(allrows), 'groups': [dict(corpus=c, category=k, source_type=t, count=n) for (c, k, t), n in sorted(summary.items())]}, indent=2))
    print((ROOT / 'extra-matches-summary.json').read_text())
if __name__ == '__main__':
    main()
