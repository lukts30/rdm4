#!/usr/bin/env python3
"""Check packed joint/weight data against the model-selected skeleton."""
import collections, json, struct
from pathlib import Path
import os
ROOT = Path(os.environ.get('RDM_AUDIT_ROOT', Path.cwd() / 'audit-results'))
for game, base in [('anno2070', os.environ['RDM_AUDIT_2070']), ('anno1800', os.environ['RDM_AUDIT_1800'])]:
    out = []
    dedup = set()
    for r in map(json.loads, open(ROOT / game / 'files.jsonl')):
        if r['sha256'] in dedup:
            continue
        dedup.add(r['sha256'])
        if not any((m['skeleton_index'] >= 0 for m in r.get('models', []))):
            continue
        b = (Path(base) / r['file']).read_bytes()
        e = '<' if b[3] else '>'
        u = lambda at: struct.unpack_from(e + 'I', b, at)[0]
        root = u(16)
        sk = u(root + 12)
        ss = u(sk - 4)
        for m in r['models']:
            if m['skeleton_index'] < 0:
                continue
            at = m['record']
            bp = u(sk + m['skeleton_index'] * ss)
            bones = u(bp - 8)
            vp = u(at + 12)
            vs = m['vertex_stride']
            jo = []
            wo = []
            off = 0
            for sem, typ, interp, count in m['components']:
                if sem == 7:
                    jo.append(off)
                if sem == 6:
                    wo.append(off)
                off += {3: 2, 5: 4, 6: 2, 7: 4}[typ] * count

            def word(at):
                v = b[at:at + 4]
                return v if e == '<' else v[::-1]
            sums = collections.Counter()
            active = collections.Counter()
            bad = 0
            examples = []
            for i in range(m['vertices']):
                pos = vp + i * vs
                js = [j for off in jo for j in word(pos + off)]
                ws = [w for off in wo for w in word(pos + off)] if wo else [255] + [0] * (len(js) - 1)
                if not js:
                    continue
                sums[sum(ws)] += 1
                active[sum((w > 0 for w in ws))] += 1
                invalid = [(j, w) for j, w in zip(js, ws) if w and j >= bones]
                if invalid:
                    bad += 1
                    if len(examples) < 2:
                        examples.append(dict(vertex=i, pairs=invalid))
            out.append(dict(file=r['file'], sha256=r['sha256'], record=at, vertices=m['vertices'], declared=m['influences'], joint_sets=len(jo), weight_sets=len(wo), bones=bones, weight_sums=dict(sums), active_influences=dict(active), out_of_range_vertices=bad, examples=examples))
    (ROOT / (game + '-skin.json')).write_text(json.dumps(out, indent=2))
    print(game, 'unique skinned models', len(out), 'bad joint models', sum((x['out_of_range_vertices'] > 0 for x in out)), 'zero weight models', sum(('0' in {str(k) for k in x['weight_sums']} for x in out)), 'over declared models', sum((max(x['active_influences'], default=0) > x['declared'] for x in out)), flush=True)
