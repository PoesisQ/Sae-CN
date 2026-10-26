"""Collect 0.3 anonymous reports and an explicit original-fixture image list."""
import hashlib
import json
from pathlib import Path
import re
import shutil

root=Path(__file__).resolve().parents[1]
source=root/'artifacts/iteration-0.3'
output=root/'docs/evidence/iteration-0.3'
output.mkdir(parents=True,exist_ok=True)

def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def write(path,value):path.write_bytes((json.dumps(value,indent=2,ensure_ascii=False)+'\n').encode('utf-8'))
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()

bench=source/'measurements'
final_bench=source/'measurements-final'
before_memory=read(bench/'memory-aggregate.json')
before_startup=read(bench/'startup-aggregate.json')
memory=read(final_bench/'memory-aggregate.json')
startup=read(final_bench/'startup-aggregate.json')
memory['memory']=[r for r in before_memory['memory'] if r['version']=='previous']+memory['memory']
startup['startup']=[r for r in before_startup['startup'] if r['version']=='previous']+startup['startup']
builds=memory['builds']
builds['before_startup_optimization']=before_memory['builds']
for kind,folder in [('production','artifacts/production/SaeReader-0.3.0-preview.1/SaeReader.exe'),
                    ('qa','artifacts/qa/SaeReader-0.3.0-preview.1-qa.exe')]:
    assert digest(root/folder)==builds[kind]['current']['sha256']
builds['machine']=read(source/'machine.json')
builds['imports']=read(source/'imports.json')
assert set(builds['imports']['0.2.0-preview.1'])==set(builds['imports']['0.3.0-preview.1'])
paths=[root/'Cargo.toml',root/'Cargo.lock',root/'build.rs',root/'assets/sae-reader.rc']
for folder in ['src','ui','native']:
    paths += [p for p in (root/folder).rglob('*') if p.is_file()]
paths += [root/'vendor/mupdf-sys-0.8.0/mupdf/source/html/html-doc.c',
          root/'vendor/mupdf-sys-0.8.0/wrapper/document.c',root/'vendor/libmobi/UPSTREAM.txt']
builds['source_inputs']={p.relative_to(root).as_posix():digest(p) for p in sorted(paths)}
write(output/'builds.json',builds)
write(output/'memory-summary.json',memory)
write(output/'startup-summary.json',startup)
raw={}
for stage,folder in [('before_startup_optimization',bench),('final',final_bench)]:
    raw[stage]={}
    for phase in ['memory','startup']:
        cases={}
        for path in sorted((folder/phase).glob('*/*/summary.json')):
            reports=[read(p) for p in sorted(path.parent.glob('*.json')) if p.name!='summary.json']
            # The memory harness stores samples in summary.json itself; startup
            # has independent process reports as well as its aggregate.
            data=read(path)
            if phase=='memory':assert all(d['unexpected_exit'] is None for d in data)
            else:assert all(not d['errors'] and d['exit_code']==0 for d in reports)
            cases[path.parent.relative_to(folder/phase).as_posix()]={'summary':data,'reports':reports}
        raw[stage][phase]=cases
write(output/'measurements-raw.json',raw)
scenarios=read(source/'verified-release/window-scenarios.json')
assert len(scenarios)==12 and all(not v['errors'] and v['exit_code']==0 for v in scenarios.values())
assert scenarios['navigation']['operations']==111
assert scenarios['detents']['operations']==75
assert scenarios['stress']['operations']==730
write(output/'window-scenarios.json',scenarios)
tests=(source/'release-tests.log').read_text(encoding='utf-8-sig').replace('\r\n','\n')
assert 'FAILED' not in tests and sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests)))==32
(output/'test-release.log').write_bytes(tests.encode('utf-8'))
for name in ['qa-build.log','production-build.log']:
    text=(source/name).read_text(encoding='utf-8-sig').replace('\r\n','\n')
    assert 'Finished `release`' in text and 'error' not in text
    (output/name).write_bytes(text.encode('utf-8'))
if (source/'manual-input.json').exists():write(output/'manual-input.json',read(source/'manual-input.json'))
pictures={
    'ui-contents-light.png':('contents','contents-01.png'),
    'ui-contents-scrolled.png':('contents','contents-03.png'),
    'ui-contents-night.png':('contents','contents-05.png'),
    'ui-contents-narrow.png':('contents','contents-06.png'),
    'ui-contents-narrow-scrolled.png':('contents','contents-07.png'),
    'ui-contents-scale-100.png':('contents-100','contents-06.png'),
    'ui-contents-scale-125.png':('contents-125','contents-06.png'),
    'ui-contents-scale-200.png':('contents-200','contents-06.png'),
    'ui-dial-page.png':('detents','detent-04.png'),
    'ui-dial-rotated.png':('detents','detent-06.png'),
    'ui-dial-fill.png':('detents','detent-75.png'),
    'ui-fb2.png':('fb2','first.png'),
    'ui-fb2-night.png':('fb2','view-05.png'),
    'ui-azw3.png':('azw3','view-02.png'),
    'ui-mobi.png':('mobi','first.png'),
    'ui-pdf-theme.png':('features','feature-05.png'),
    'ui-ruby-math.png':('features','feature-19.png'),
    'ui-pdf-toolbar.png':('chrome','chrome-02.png'),
    'ui-immersive.png':('chrome','chrome-14.png'),
}
images=[]
for target,(folder,name) in pictures.items():
    path=source/f'verified-release/{folder}/fixture-00-screenshots/{name}'
    shutil.copy2(path,output/target)
    images.append({'file':target,'sha256':digest(path),'build':builds['qa']['current']['sha256'],
                   'capture':'native Slint software renderer; original CC0 fixture only'})
write(output/'images.json',images)
print(json.dumps({'production':builds['production']['current'],'qa':builds['qa']['current'],
                  'images':len(images),'scenarios':len(scenarios),'tests':32},indent=2))
