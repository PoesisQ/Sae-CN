"""Copy only privacy-safe counters and original-fixture UI evidence into docs."""
import hashlib
import json
from pathlib import Path
import shutil

root=Path(__file__).resolve().parents[1];out=root/'docs/evidence/feedback';out.mkdir(parents=True,exist_ok=True)
measurements=root/'artifacts/measurements'
for folder in measurements.iterdir():
    if not folder.is_dir():continue
    target=out/folder.name;target.mkdir(exist_ok=True)
    # Profiles contain private paths; never copy directories or profile state.
    for f in folder.glob('*.json'):shutil.copy2(f,target/f.name)
for name in ['window-stress','window-visual-final','local-bench','local-bench-pdf']:
    target=out/name;target.mkdir(exist_ok=True)
    for pattern in ['*.json','*.jsonl']:
        for f in (root/'artifacts'/name).glob(pattern):shutil.copy2(f,target/f.name)
views={'first':'paper-single','view-01':'image-table','view-02':'paper-double','view-03':'contents','view-04':'typography','view-05':'night','view-06':'narrow'}
for src,name in views.items():shutil.copy2(root/f'artifacts/window-visual-final/fixture-00-screenshots/{src}.png',out/f'ui-{name}.png')
for scale in ['1','1.25','1.5','2']:
    shutil.copy2(root/f'artifacts/ui-scale-{scale}/fixture-00-screenshots/view-06.png',out/f'ui-controlled-scale-{scale}.png')
for variant in [1,2]:shutil.copy2(root/f'artifacts/ui-design-{variant}/fixture-00-screenshots/view-02.png',out/f'ui-design-{variant}.png')
shutil.copy2(root/'artifacts/ui-pdf/fixture-00-screenshots/first.png',out/'ui-pdf.png')
builds={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in {
    'production':root/'artifacts/production/SaeReader.exe','qa':root/'artifacts/qa/sae-reader-qa.exe',
    'baseline':root/'artifacts/baseline/sae-reader-old.exe'}.items()}
(out/'builds.json').write_text(json.dumps(builds,indent=2),encoding='utf-8')
print('Copied anonymous counters and original-fixture UI evidence; excluded book files, images and profiles.')
