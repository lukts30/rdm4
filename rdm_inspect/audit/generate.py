#!/usr/bin/env python3
"""Stratified real-model round trips and controlled glTF writer probes."""
import argparse, base64, collections, hashlib, json, struct, subprocess
from pathlib import Path
import os
ROOT = Path(os.environ.get('RDM_AUDIT_ROOT', Path.cwd() / 'audit-results'))
FORMATS = ['P4h_N4b_G4b_B4b_T2h', 'P4h_N4b_G4b_B4b_T2h_I4b', 'P4h_N4b_G4b_B4b_T2h_I4b_W4b', 'P3f_N3f_G3f_B3f_T2f_C4b', 'P4h_N4b_G4b_B4b_T2h_C4b_C4b', 'P4h_T2h_C4b']

def synthetic(path, bones=4, large=False, bind_matrices=False):
    data = bytearray()
    views = []
    acc = []

    def accessor(values, code, typ, fmt, width, bounds=False):
        while len(data) % 4:
            data.append(0)
        off = len(data)
        for v in values:
            data.extend(struct.pack('<' + fmt, *v))
        views.append(dict(buffer=0, byteOffset=off, byteLength=len(data) - off))
        a = dict(bufferView=len(views) - 1, componentType=code, count=len(values), type=typ)
        if bounds:
            a.update(min=[min((v[j] for v in values)) for j in range(width)], max=[max((v[j] for v in values)) for j in range(width)])
        acc.append(a)
        return len(acc) - 1
    pos = [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)]
    if large:
        pos[1] = (100000.0, 0.0, 0.0)
    attrs = {'POSITION': accessor(pos, 5126, 'VEC3', '3f', 3, True), 'NORMAL': accessor([(0.0, 0.0, 1.0)] * 3, 5126, 'VEC3', '3f', 3), 'TANGENT': accessor([(1.0, 0.0, 0.0, 1.0)] * 3, 5126, 'VEC4', '4f', 4), 'TEXCOORD_0': accessor([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)], 5126, 'VEC2', '2f', 2), 'COLOR_0': accessor([(1.0, 0.5, 0.25, 1.0)] * 3, 5126, 'VEC4', '4f', 4), 'COLOR_1': accessor([(0.25, 1.0, 0.5, 1.0)] * 3, 5126, 'VEC4', '4f', 4), 'JOINTS_0': accessor([(bones - 1 if bones > 4 else 0, 1, 2, 3)] * 3, 5123, 'VEC4', '4H', 4), 'WEIGHTS_0': accessor([(0.25, 0.25, 0.25, 0.25)] * 3, 5126, 'VEC4', '4f', 4)}
    inds = accessor([(0,), (1,), (2,)], 5123, 'SCALAR', 'H', 1)
    nodes = [dict(mesh=0, skin=0, name='mesh')] + [dict(name='bone_' + str(i)) for i in range(bones)]
    nodes[1]['children'] = list(range(2, bones + 1))
    times = accessor([(0.0,), (1.0,)], 5126, 'SCALAR', 'f', 1, True)
    rot = accessor([(0.0, 0.0, 0.0, 1.0)] * 2, 5126, 'VEC4', '4f', 4)
    doc = dict(asset={'version': '2.0'}, scene=0, scenes=[{'nodes': [0, 1]}], nodes=nodes, skins=[{'joints': list(range(1, bones + 1))}], meshes=[{'primitives': [{'attributes': attrs, 'indices': inds, 'material': 0}]}], materials=[{}], animations=[{'name': 'idle', 'samplers': [{'input': times, 'output': rot, 'interpolation': 'LINEAR'}], 'channels': [{'sampler': 0, 'target': {'node': 1, 'path': 'rotation'}}]}], buffers=[{'byteLength': len(data), 'uri': 'data:application/octet-stream;base64,' + base64.b64encode(data).decode()}], bufferViews=views, accessors=acc)
    if bind_matrices:
        identity = tuple((1.0 if i % 5 == 0 else 0.0 for i in range(16)))
        doc['skins'][0]['inverseBindMatrices'] = accessor([identity] * bones, 5126, 'MAT4', '16f', 16)
        doc['buffers'][0] = {'byteLength': len(data), 'uri': 'data:application/octet-stream;base64,' + base64.b64encode(data).decode()}
    path.write_text(json.dumps(doc))

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('converter')
    args = ap.parse_args()
    out = ROOT / 'generated'
    out.mkdir(exist_ok=True)
    results = []

    def run(label, command):
        p = subprocess.run([args.converter, *command], capture_output=True, text=True, timeout=90)
        log = out / (label + '.log')
        log.write_text(p.stdout + p.stderr)
        results.append(dict(label=label, command=[args.converter, *command], exit_code=p.returncode, log=str(log)))
        return p.returncode == 0
    selected = {}
    for game, root in [('anno2070', os.environ['RDM_AUDIT_2070']), ('anno1800', os.environ['RDM_AUDIT_1800'])]:
        for r in map(json.loads, open(ROOT / game / 'files.jsonl')):
            if not r.get('models'):
                continue
            m = r['models'][0]
            k = (game, r['endian'], str(r['root_shape']), str(m['components']), m['index_type'], m['influences'], min(r['record_counts'].get('model', 0), 2), min(r['record_counts'].get('skeleton', 0), 2))
            if k not in selected or r['size'] < selected[k][1]['size']:
                selected[k] = (root, r)
    (ROOT / 'selected-sources.json').write_text(json.dumps([dict(root=a, file=r['file'], sha256=r['sha256'], models=r['models'], record_counts=r['record_counts']) for a, r in selected.values()], indent=2))
    for i, (root, r) in enumerate(selected.values()):
        label = f'source-{i:03}'
        folder = out / label
        folder.mkdir(exist_ok=True)
        glb = folder / 'source.glb'
        m = r['models'][0]
        skin = m['skeleton_index'] >= 0 and r['record_counts'].get('skeleton', 0) > 0
        if run(label + '-export', ['-i', str(Path(root) / r['file']), '-o', str(glb), '--force'] + (['-s'] if skin else [])):
            fmt = FORMATS[2] if skin and any((c[0] == 6 for c in m['components'])) else FORMATS[1] if skin else FORMATS[0]
            run(label + '-import', ['-i', str(glb), '-o', str(folder / 'roundtrip.rdm'), '-g', fmt, '--no_transform', '--force'] + (['-s'] if skin else []))
        print(label, r['file'], flush=True)
    for kind, bones, large in [('weighted', 4, False), ('joint256', 257, False), ('large-position', 4, True)]:
        folder = out / kind
        folder.mkdir(exist_ok=True)
        gltf = folder / 'input.gltf'
        synthetic(gltf, bones, large)
        for i, fmt in enumerate(FORMATS if kind == 'weighted' else [FORMATS[2]]):
            dst = folder / f'format-{i}'
            dst.mkdir(exist_ok=True)
            skin = 'I4b' in fmt
            run(kind + f'-{i}', ['-i', str(gltf), '-o', str(dst / 'mesh.rdm'), '-g', fmt, '--no_transform', '--force'] + (['-s', '-a'] if skin and kind == 'weighted' else ['-s'] if skin else []))
    (ROOT / 'generation.json').write_text(json.dumps(results, indent=2))
    print('commands', len(results), 'failed', sum((x['exit_code'] != 0 for x in results)))
if __name__ == '__main__':
    main()
