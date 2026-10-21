#!/usr/bin/env python3
"""Audit extracted game corpora and a stratified converter-output corpus."""
import argparse, json, os, subprocess, sys
from pathlib import Path

def main():
    p = argparse.ArgumentParser(description=__doc__)
    for key in ['anno2070', 'anno1800', 'converter', 'inspector', 'output']:
        p.add_argument('--' + key, type=Path, required=True)
    a = p.parse_args()
    a.output = a.output.resolve()
    a.output.mkdir(parents=True, exist_ok=True)
    if any(a.output.iterdir()):
        p.error('--output must be a fresh empty directory')
    here = Path(__file__).parent
    env = dict(os.environ, RDM_AUDIT_ROOT=str(a.output), RDM_AUDIT_2070=str(a.anno2070.resolve()), RDM_AUDIT_1800=str(a.anno1800.resolve()), RDM_AUDIT_CONVERTER=str(a.converter.resolve()))

    def call(script, *args):
        with (a.output / (script + '.log')).open('w') as log:
            subprocess.run([sys.executable, str(here / script), *map(str, args)], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)

    def inspect(label, root):
        paths = a.output / (label + '-paths.txt')
        paths.write_text(''.join((str(p) + '\n' for p in sorted(root.rglob('*.rdm')))))
        with paths.open() as src, (a.output / (label + '-inspector.tsv')).open('w') as out:
            subprocess.run([str(a.inspector.resolve())], stdin=src, stdout=out, check=True)
    for label, root in [('anno2070', a.anno2070.resolve()), ('anno1800', a.anno1800.resolve())]:
        print('Auditing', label, flush=True)
        call('audit.py', root, a.output / label)
        inspect(label, root)
    call('skin_audit.py')
    call('generate.py', a.converter.resolve())
    call('probes.py')
    call('audit.py', a.output / 'generated', a.output / 'generated-audit')
    inspect('generated', a.output / 'generated')
    print('Reports:', a.output)
if __name__ == '__main__':
    main()
