#!/usr/bin/env python3
"""Read-only corpus audit. Emits JSONL per file and a JSON aggregate.

Physical scanning and schema traversal are independent. Only declared pointer
fields become semantic edges. Unrecognized records/unreached blocks are reported,
not silently called invalid. Input roots should contain archive-separated members.
"""
import argparse, array, collections, hashlib, json, math, struct, sys
from pathlib import Path
LAYOUTS = {'root': [(0, 'export'), (4, 'model'), (8, 'material_link'), (12, 'skeleton'), (16, 'animation'), (20, 'root_vertices')], 'export': [(0, 'string'), (4, 'string')], 'model': [(0, 'name'), (4, 'vertex_link'), (8, 'index_format'), (12, 'vertices'), (16, 'indices'), (20, 'tri_groups')], 'name': [(0, 'string')], 'vertex_link': [(0, 'components')], 'material_link': [(0, 'material')], 'material': [(0, 'string'), (4, 'string')], 'skeleton': [(0, 'bones'), (8, 'companion')], 'bones': [(0, 'string')], 'animation': [(0, 'string'), (4, 'tracks')], 'tracks': [(0, 'string'), (4, 'frames')], 'root_vertices': [(0, 'vertex_link'), (8, 'vertices')]}
WIDTHS = {'root': (48, 52), 'export': (72,), 'model': (68, 92), 'name': (28,), 'vertex_link': (24,), 'components': (16,), 'index_format': (20,), 'tri_groups': (28,), 'material_link': (28,), 'material': (48,), 'skeleton': (32,), 'bones': (84,), 'animation': (48,), 'tracks': (24,), 'frames': (32,), 'root_vertices': (64,), 'string': (1,), 'companion': (1,)}

def analyze(data):
    r = {'size': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'findings': [], 'metrics': {}}

    def flag(code, **kw):
        r['metrics'][code] = r['metrics'].get(code, 0) + 1
        if sum((x['code'] == code for x in r['findings'])) < 3:
            r['findings'].append(dict(code=code, **kw))
    if len(data) < 28 or data[:3] != b'RDM' or data[3] not in (0, 1):
        r['parse_error'] = 'magic/preamble'
        return r
    e = '<' if data[3] else '>'
    r['endian'] = 'LE' if data[3] else 'BE'
    u = lambda p: struct.unpack_from(e + 'I', data, p)[0]
    pre = list(struct.unpack_from(e + '4I', data, 4))
    r['preamble'] = pre
    if pre != [20, 0, 4, 28]:
        flag('preamble_variant', words=pre)
    blocks = {}
    h = 20
    while h < len(data):
        if h + 8 > len(data):
            r['parse_error'] = f'partial header at {h}'
            break
        c, s = struct.unpack_from(e + 'II', data, h)
        end = h + 8 + c * s
        if not c or not s or end > len(data):
            r['parse_error'] = f'invalid block at {h}: {c} x {s}, end {end}'
            break
        blocks[h + 8] = (c, s)
        h = end
    r['physical_complete'] = h == len(data)
    r['blocks'] = len(blocks)
    if not r['physical_complete']:
        return r
    r['odd_headers'] = sum(((p - 8) % 2 for p in blocks))
    r['root_shape'] = blocks.get(pre[3])
    types = {}
    edges = []
    todo = [(pre[3], 'root')]
    seen = set()
    records = collections.defaultdict(list)
    while todo:
        p, t = todo.pop()
        if p not in blocks:
            flag('invalid_target', target=p, type=t)
            continue
        if p in seen:
            if types[p] != t:
                flag('conflicting_type', target=p, types=[types[p], t])
            continue
        seen.add(p)
        types[p] = t
        c, s = blocks[p]
        if t in WIDTHS and s not in WIDTHS[t]:
            flag('record_width', type=t, stride=s)
        for i in range(c):
            at = p + i * s
            records[t].append((at, s))
            for off, ct in LAYOUTS.get(t, []):
                if off + 4 > s:
                    flag('short_record', type=t)
                    continue
                q = u(at + off)
                if q:
                    edges.append((p, q, at + off, ct))
                    todo.append((q, ct))
                    if q <= at + off:
                        flag('backward_reference', source=at + off, target=q, type=t)
    r['edges'] = len(edges)
    r['unreached'] = [p for p in blocks if p not in seen]
    if r['unreached']:
        flag('unreached_blocks', blocks=r['unreached'][:12])
    incoming = collections.Counter((q for p, q, a, t in edges))
    shared = [p for p, n in incoming.items() if n > 1]
    if shared:
        flag('shared_blocks', blocks=shared[:12])
    r['tree_complete'] = not r['unreached'] and all((incoming[p] == (0 if p == pre[3] else 1) for p in blocks)) and (not any((q not in blocks for p, q, a, t in edges)))
    children = collections.defaultdict(list)
    for p, q, a, t in edges:
        children[p].append(q)
    order = []
    visited = set()

    def walk(p):
        if p in visited:
            return
        visited.add(p)
        order.append(p)
        for q in sorted(children[p]):
            walk(q)
    walk(pre[3])
    r['preorder'] = r['tree_complete'] and order == list(blocks)
    if r['tree_complete'] and (not r['preorder']):
        flag('not_preorder')
    r['record_counts'] = {t: len(v) for t, v in records.items() if t not in ('vertices', 'indices', 'frames', 'string', 'companion')}
    r['shapes'] = {t: sorted(set((s for a, s in v))) for t, v in records.items()}
    models = []

    def block(q):
        return blocks[q]
    for at, s in records['model']:
        try:
            vp, ip, tp = (u(at + 12), u(at + 16), u(at + 20))
            vc, vs = block(vp)
            ic, isz = block(ip)
            sk = struct.unpack_from(e + 'i', data, at + 24)[0]
            if sk < -1 or sk >= len(records['skeleton']):
                flag('skeleton_index_range', record=at, index=sk)
            fmt = u(at + 8)
            it = u(fmt)
            desc = u(u(at + 4))
            dc, ds = block(desc)
            components = [list(struct.unpack_from(e + '4I', data, desc + j * ds)) for j in range(dc)]
            width = sum(({3: 2, 5: 4, 6: 2, 7: 4}.get(x[1], 0) * x[3] for x in components))
            if width != vs:
                flag('vertex_stride_mismatch', record=at, calculated=width, stored=vs, components=components)
            if any((x[1] not in (5, 6, 7) for x in components)):
                flag('unsupported_vertex_storage', record=at, components=components)
            influences = u(u(at + 4) + 4)
            if it not in (3, 5) or {3: 2, 5: 4}.get(it) != isz:
                flag('index_type_mismatch', record=at, type=it, stride=isz)
            inds = array.array('H' if isz == 2 else 'I')
            inds.frombytes(data[ip:ip + ic * isz])
            if (e == '<') != (sys.byteorder == 'little'):
                inds.byteswap()
            mx = max(inds, default=0)
            if mx >= vc:
                flag('index_out_of_range', record=at, maximum=mx, vertices=vc)
            if ic % 3:
                flag('index_count_not_triangles', record=at, count=ic)
            tc, ts = block(tp)
            groups = [struct.unpack_from(e + '3I', data, tp + j * ts) for j in range(tc)]
            for start, n, mat in groups:
                if start + n > ic:
                    flag('tri_group_range', record=at, start=start, count=n, indices=ic)
                if mat >= len(records['material_link']):
                    flag('material_index_range', record=at, material=mat, materials=len(records['material_link']))
            bounds = struct.unpack_from(e + '6f', data, at + 28)
            if not all((math.isfinite(x) for x in bounds)) or any((bounds[j] > bounds[j + 3] for j in range(3))):
                flag('invalid_model_bounds', record=at)
            tail = [u(x) for x in range(at + 52, at + s, 4)]
            if any(tail):
                flag('nonzero_model_tail', record=at, tail=tail)
            models.append(dict(record=at, stride=s, vertices=vc, vertex_stride=vs, index_type=it, index_width=isz, skeleton_index=sk, influences=influences, components=components, tri_groups=tc))
        except (KeyError, struct.error, ValueError) as ex:
            flag('semantic_read_error', record=at, error=str(ex))
    r['models'] = models
    r['sampling_rates'] = [u(at + 12) for at, s in records['animation']]
    for at, s in records['skeleton']:
        try:
            bp = u(at)
            bc, bs = block(bp)
            parents = [u(bp + j * bs + 32) for j in range(bc)]
            names = []
            for j, par in enumerate(parents):
                if par != 4294967295 and par >= bc:
                    flag('bone_parent_range', skeleton=at, bone=j, parent=par)
                if par != 4294967295 and par >= j:
                    flag('bone_parent_not_prior', skeleton=at, bone=j, parent=par)
                np = u(bp + j * bs)
                nc, ns = block(np)
                names.append(data[np:np + nc * ns].hex())
            if len(set(names)) != len(names):
                flag('duplicate_bone_names', skeleton=at)
            if u(at + 8):
                flag('skeleton_companion_present', skeleton=at, target=u(at + 8))
        except (KeyError, struct.error) as ex:
            flag('skeleton_read_error', record=at, error=str(ex))
    for at, s in records['tracks']:
        try:
            fp = u(at + 4)
            if not fp:
                flag('null_frame_pointer', track=at)
                continue
            fc, fs = block(fp)
            prev = -float('inf')
            for j in range(fc):
                time = struct.unpack_from(e + 'f', data, fp + j * fs + 28)[0]
                if not math.isfinite(time) or time < prev:
                    flag('frame_time_order', track=at, frame=j)
                    break
                prev = time
        except (KeyError, struct.error) as ex:
            flag('track_read_error', record=at, error=str(ex))
    return r

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('root', type=Path)
    ap.add_argument('output', type=Path)
    args = ap.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    summary = collections.Counter()
    metrics = collections.Counter()
    shapes = collections.Counter()
    unique = set()
    virtual = set()
    with (args.output / 'files.jsonl').open('w') as out:
        for i, p in enumerate(sorted(args.root.rglob('*.rdm'))):
            try:
                r = analyze(p.read_bytes())
            except Exception as ex:
                r = {'audit_error': repr(ex)}
            r['file'] = str(p.relative_to(args.root))
            out.write(json.dumps(r, allow_nan=False) + '\n')
            summary['files'] += 1
            if 'sha256' in r:
                unique.add(r['sha256'])
            parts = p.relative_to(args.root).parts
            virtual.add('/'.join(parts[1:]) if parts[0].lower().endswith('.rda') else '/'.join(parts))
            for key in ['physical_complete', 'tree_complete', 'preorder']:
                summary[key] += bool(r.get(key))
            if 'parse_error' in r:
                summary['parse_error'] += 1
            if 'audit_error' in r:
                summary['audit_error'] += 1
            summary[r.get('endian', 'unknown')] += 1
            if r.get('root_shape'):
                shapes[str(r['root_shape'])] += 1
            metrics.update(r.get('metrics', {}))
            if i % 2000 == 0:
                print(i, p.name, flush=True)
    summary.update(unique_contents=len(unique), unique_virtual_paths=len(virtual))
    result = dict(counts=dict(summary), finding_instances=dict(metrics), root_shapes=dict(shapes))
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
if __name__ == '__main__':
    main()
