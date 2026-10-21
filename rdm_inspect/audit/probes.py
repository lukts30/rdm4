#!/usr/bin/env python3
import json, math, struct, subprocess
from pathlib import Path
import os
from generate import synthetic, FORMATS
ROOT = Path(os.environ.get('RDM_AUDIT_ROOT', Path.cwd() / 'audit-results'))
BIN = os.environ['RDM_AUDIT_CONVERTER']
out = ROOT / 'generated'
results = []

def run(label, args):
    p = subprocess.run([BIN, *args], capture_output=True, text=True)
    log = out / (label + '.log')
    log.write_text(p.stdout + p.stderr)
    results.append(dict(label=label, command=[BIN, *args], exit_code=p.returncode, log=str(log)))
    return p.returncode == 0
for kind, bones, large in [('weighted-explicit', 4, False), ('joint256-explicit', 257, False), ('large-position-explicit', 4, True)]:
    folder = out / kind
    folder.mkdir(exist_ok=True)
    gltf = folder / 'input.gltf'
    synthetic(gltf, bones, large, True)
    for i, fmt in enumerate([FORMATS[1], FORMATS[2]] if kind == 'weighted-explicit' else [FORMATS[2]]):
        dst = folder / f'format-{i}'
        dst.mkdir(exist_ok=True)
        run(kind + f'-{i}', ['-i', str(gltf), '-o', str(dst / 'mesh.rdm'), '-g', fmt, '--no_transform', '--force', '-s'] + (['-a'] if kind == 'weighted-explicit' else []))
rows = json.loads((ROOT / 'anno1800-skin.json').read_text())
z = next((r for r in rows if '0' in r['weight_sums']))
folder = out / 'zero-weights'
folder.mkdir(exist_ok=True)
run('zero-weights', ['-i', str(Path(os.environ['RDM_AUDIT_1800']) / z['file']), '-o', str(folder / 'source.glb'), '-s', '--force'])
(ROOT / 'probes.json').write_text(json.dumps(results, indent=2))
print([(r['label'], r['exit_code']) for r in results])
