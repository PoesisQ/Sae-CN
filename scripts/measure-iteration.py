"""Paired, sequential real-Windows measurements; public output uses aliases.

Never copy books or user configuration. Production memory/CPU measurements and
the explicitly instrumented QA first-snapshot measurements are separate phases.
"""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--books', type=Path, required=True)
parser.add_argument('--upstream', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--phase', choices=['memory', 'startup', 'both'], default='both')
parser.add_argument('--current-only', action='store_true', help='Remeasure a changed current binary; keep the earlier baseline reports')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
executables = {
    'previous': root/'artifacts/production/SaeReader-0.2.0-preview.1/SaeReader.exe',
    'current': root/'artifacts/production/SaeReader-0.3.0-preview.1/SaeReader.exe',
}
qa = {
    'previous': root/'artifacts/qa/SaeReader-0.2.0-preview.1-qa.exe',
    'current': root/'artifacts/qa/SaeReader-0.3.0-preview.1-qa.exe',
}
epubs = sorted(args.books.glob('*.epub'))
assert len(epubs) == 3, 'Expected the three opt-in local EPUB comparison books'
azw3 = next(args.books.rglob('*.azw3'))
# Fixed upstream sample alias; it is read locally and never redistributed.
hybrids = sorted(args.upstream.glob('*mobi*'))
hybrid = next((p for p in hybrids if 'huff' in p.name.lower()), None)
if hybrid is None:
    hybrid = next(p for p in hybrids if p.is_file() and 'hybrid' in p.name.lower())
cases = [('blank', None, False, True), ('pdf-single', root/'tests/fixtures/core.pdf', False, True),
         ('pdf-double', root/'tests/fixtures/core.pdf', True, True)]
for n, path in enumerate(epubs, 1):
    cases += [(f'epub-{n:02}-single', path, False, True),
              (f'epub-{n:02}-double', path, True, True)]
cases += [('azw3-single', azw3, False, False), ('azw3-double', azw3, True, False),
          ('mobi-hybrid', hybrid, False, False), ('fb2-single', root/'tests/fixtures/reading.fb2', False, False)]


def run_harness(script, version, case, path, spread, count, phase):
    folder = args.output/phase/version/case
    if (folder/'summary.json').exists():
        raise RuntimeError('Refusing to mix measurements with an existing run')
    executable = executables[version] if phase == 'memory' else qa[version]
    command = [sys.executable, str(root/'scripts'/script), '--exe', str(executable),
               '--runs', str(count), '--output', str(folder)]
    if path:
        command += ['--fixture', str(path)]
    elif phase == 'startup':
        command += ['--blank']
    if spread:
        command += ['--spread']
    if phase == 'startup':
        command += ['--scenario', 'startup']
    else:
        command += ['--seconds', '8']
    subprocess.run(command, cwd=root, check=True, stdout=subprocess.DEVNULL)
    data = json.loads((folder/'summary.json').read_text(encoding='utf-8'))
    if phase == 'memory':
        assert all(r['unexpected_exit'] is None and r['steady_median'] for r in data)
        result = {key: statistics.median(r['steady_median'][key] for r in data)
                  for key in ['private_bytes', 'private_working_set', 'working_set', 'handles']}
        result['idle_one_core_percent'] = statistics.median(r['idle_one_core_percent'] for r in data)
        result['peak_private_bytes'] = max(max(s['private_bytes'] for s in r['samples']) for r in data)
    else:
        assert data[0]['failures'] == 0
        result = data[0]['native_snapshot_ready_ms']
    print(json.dumps({'phase': phase, 'version': version, 'case': case, 'n': count, 'result': result}), flush=True)
    return {'version': version, 'case': case, 'n': count, 'result': result, 'raw': folder.relative_to(args.output).as_posix()}


results = {'inputs': [{'case': case, 'bytes': path.stat().st_size if path else 0,
                      'spread': spread, 'paired': paired}
                     for case, path, spread, paired in cases],
           'builds': {kind: {version: {'bytes': p.stat().st_size,
                                      'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
                            for version, p in paths.items()}
                      for kind, paths in [('production', executables), ('qa', qa)]}}
for phase in ['memory', 'startup']:
    if args.phase not in ['both', phase]:
        continue
    rows = []
    for case, path, spread, paired in cases:
        if phase == 'startup' and spread:
            continue
        # Consecutive previous/current cases reduce time-of-run drift. The host
        # remains a live desktop; this is not an isolated lab or OS-cold test.
        for version in (['current'] if args.current_only else ['previous', 'current'] if paired else ['current']):
            rows.append(run_harness('profile-window.py' if phase == 'memory' else 'qa-windows.py',
                                    version, case, path, spread, 3 if phase == 'memory' else 20, phase))
    results[phase] = rows
    (args.output/f'{phase}-aggregate.json').write_text(json.dumps(results, indent=2)+'\n', encoding='utf-8')
print('Measurements complete; reports contain aliases and counters only.', flush=True)
