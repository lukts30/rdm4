#!/usr/bin/env python3
"""Extract only RDM members, keeping archives separate and originals read-only."""
import argparse, json, subprocess, time
from pathlib import Path
p = argparse.ArgumentParser()
p.add_argument('maindata', type=Path)
p.add_argument('output', type=Path)
p.add_argument('rdaconsole')
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
rows = []
for archive in sorted(a.maindata.glob('*.rda')):
    start = time.monotonic()
    scratch = a.output / 'input.rda'
    dst = a.output / 'extracted' / archive.name
    subprocess.run(['cp', '--reflink=auto', str(archive), str(scratch)], check=True)
    try:
        with (a.output / (archive.name + '.log')).open('w') as out:
            r = subprocess.run([a.rdaconsole, 'extract', '-f', str(scratch), '-o', str(dst), '--filter', '(?i)\\.rdm$', '-n'], stdout=out, stderr=subprocess.STDOUT)
    finally:
        scratch.unlink()
    row = dict(archive=str(archive), archive_size=archive.stat().st_size, exit_code=r.returncode, rdm_count=len(list(dst.rglob('*.rdm'))) if dst.exists() else 0, seconds=time.monotonic() - start)
    rows.append(row)
    (a.output / 'extraction.json').write_text(json.dumps(rows, indent=2))
    print(archive.name, row['rdm_count'], flush=True)
    if r.returncode:
        raise SystemExit(r.returncode)
