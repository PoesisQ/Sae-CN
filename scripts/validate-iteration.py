"""Run focused 0.3 native-window regressions on the visible Windows desktop."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

root=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--output',type=Path,default=root/'artifacts/iteration-0.3/verified')
args=parser.parse_args()
exe=root/'artifacts/qa/SaeReader-0.3.0-preview.1-qa.exe'
cases=[('navigation','core.pdf','navigation',None),('detents','core.pdf','detents',None),
       ('contents','catalog.epub','contents',None),('features','core.pdf','features',None),
       ('chrome','core.pdf','chrome',None),('fb2','reading.fb2','visual',None),
       ('azw3','kindle-minimal.azw3','visual',None),('mobi','kindle-palmdoc.mobi','visual',None)]
cases += [(f'contents-{scale}','catalog.epub','contents',scale/100) for scale in [100,125,200]]
cases += [('stress','core-epub3.epub','stress',None)]
results={}
for name,fixture,scenario,scale in cases:
    folder=args.output/name
    if (folder/'summary.json').exists():raise RuntimeError('Refusing to mix runs: '+name)
    command=[sys.executable,str(root/'scripts/qa-windows.py'),'--exe',str(exe),
             '--fixture',str(root/'tests/fixtures'/fixture),'--scenario',scenario,'--output',str(folder)]
    if scale:command += ['--scale',str(scale)]
    subprocess.run(command,cwd=root,check=True)
    results[name]=json.loads((folder/'fixture-00.json').read_text(encoding='utf-8'))
    assert not results[name]['errors'] and results[name]['exit_code']==0,name
    (args.output/'window-scenarios.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'scenarios':len(results),'operations':sum(v['operations'] for v in results.values()),'failures':0}),flush=True)
