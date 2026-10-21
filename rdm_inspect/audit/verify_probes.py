#!/usr/bin/env python3
import json, math, struct
from pathlib import Path
import os
ROOT = Path(os.environ.get('RDM_AUDIT_ROOT', Path.cwd() / 'audit-results'))

def glb(path):
    b = path.read_bytes()
    n = struct.unpack_from('<I', b, 12)[0]
    doc = json.loads(b[20:20 + n])
    return (doc, b[28 + n:])

def values(doc, b, idx):
    a = doc['accessors'][idx]
    v = doc['bufferViews'][a['bufferView']]
    off = v.get('byteOffset', 0) + a.get('byteOffset', 0)
    width = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4, 'MAT4': 16}[a['type']]
    fmt = {5121: 'B', 5123: 'H', 5125: 'I', 5126: 'f'}[a['componentType']]
    size = struct.calcsize('<' + fmt) * width
    stride = v.get('byteStride', size)
    return [struct.unpack_from('<' + fmt * width, b, off + i * stride) for i in range(a['count'])]

def model(path):
    b = path.read_bytes()
    u = lambda at: struct.unpack_from('<I', b, at)[0]
    root = u(16)
    m = u(root + 4)
    link = u(m + 4)
    desc = u(link)
    vp = u(m + 12)
    comps = []
    off = 0
    for j in range(u(desc - 8)):
        c = struct.unpack_from('<4I', b, desc + j * 16)
        comps.append((c, off))
        off += {5: 4, 6: 2, 7: 4}[c[1]] * c[3]
    return (b, m, u(link + 4), vp, u(vp - 8), u(vp - 4), comps)

def main():
    result = {}
    p = ROOT / 'generated/zero-weights/source.glb'
    d, b = glb(p)
    nonfinite = {}
    for prim in d['meshes'][0]['primitives']:
        for name, a in prim['attributes'].items():
            if name.startswith('WEIGHTS'):
                nonfinite[name] = sum((not math.isfinite(x) for row in values(d, b, a) for x in row))
    result['zero_weights'] = {'file': str(p), 'nonfinite_components': nonfinite}
    for label in ['weighted-explicit/format-1', 'joint256-explicit/format-0', 'source-033', 'source-053']:
        p = ROOT / 'generated' / label / ('roundtrip.rdm' if label.startswith('source') else 'mesh.rdm')
        b, m, decl, vp, n, s, cs = model(p)
        js = [off for c, off in cs if c[0] == 7]
        ws = [off for c, off in cs if c[0] == 6]
        sums = [sum((b[vp + i * s + off + j] for off in ws for j in range(4))) for i in range(n)]
        row = {'file': str(p), 'declared_influences': decl, 'joint_sets': len(js), 'weight_sets': len(ws), 'weight_sum_min': min(sums), 'weight_sum_max': max(sums), 'first_vertex_joints': [list(b[vp + off:vp + off + 4]) for off in js], 'first_vertex_weights': [list(b[vp + off:vp + off + 4]) for off in ws]}
        if label.startswith('source'):
            d, buf = glb(p.parent / 'source.glb')
            prim = d['meshes'][0]['primitives'][0]
            row['input_weight_sets'] = [k for k in prim['attributes'] if k.startswith('WEIGHTS')]
            row['input_joint_sets'] = [k for k in prim['attributes'] if k.startswith('JOINTS')]
        result[label] = row
    (ROOT / 'probe-verification.json').write_text(json.dumps(result, indent=2))
    print(json.dumps(result, indent=2))
if __name__ == '__main__':
    main()
