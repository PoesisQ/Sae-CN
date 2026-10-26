"""Collect only anonymous counters and original-fixture images, never profiles/books."""
import hashlib
import json
from pathlib import Path
import shutil
import statistics

root=Path(__file__).resolve().parents[1]
source=root/'artifacts/next'
output=root/'docs/evidence/iteration-0.2'
output.mkdir(parents=True,exist_ok=True)
def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def atomic_bytes(path,data):
    temporary=path.with_name(path.name+'.tmp')
    temporary.write_bytes(data)
    temporary.replace(path)
def write(path,value):atomic_bytes(path,(json.dumps(value,indent=2)+'\n').encode('utf-8'))
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
builds=read(source/'builds.json')
assert digest(root/'artifacts/production/SaeReader-0.2.0-preview.1/SaeReader.exe')==builds['production']
assert digest(root/'artifacts/qa/sae-reader-qa.exe')==builds['qa']
write(output/'builds.json',builds)
memory={}
earlier_baseline={}
for mode in ('blank','single','double','pdf'):
    earlier=read(source/f'baseline-{mode}/summary.json')
    if mode=='pdf':earlier+=read(source/'baseline-pdf-followup/summary.json')
    earlier_baseline[mode]=earlier
    old=read(source/f'current-baseline-{mode}/summary.json')
    new=read(source/f'verified/{mode}/summary.json')
    assert all(r['unexpected_exit'] is None and r['steady_median'] for r in old+new)
    memory[mode]={'old':old,'new':new}
write(output/'memory-raw.json',memory)
write(output/'earlier-baseline-memory.json',earlier_baseline)
real_pdf={}
for mode in ('single','double'):
    records=read(source/f'verified/pdf-real-{mode}/summary.json')
    assert len(records)==10 and all(r['unexpected_exit'] is None and r['steady_median'] for r in records)
    real_pdf[mode]=records
write(output/'pdf-real-memory.json',real_pdf)
warm={}
for folder in sorted((source/'verified').glob('startup-*')):
    summary=read(folder/'summary.json')
    assert all(s['failures']==0 and s['native_snapshot_ready_ms']['n']==20 for s in summary)
    reports=[read(p) for p in sorted(folder.glob('*.json')) if p.name!='summary.json']
    assert all(r['exit_code']==0 and not r['errors'] for r in reports)
    warm[folder.name]={'summary':summary,'reports':reports}
write(output/'startup-raw.json',warm)
reference=read(source/'sumatra-reference/summary.json')
assert all(r['unexpected_exit'] is None for r in reference)
write(output/'sumatra-reference.json',reference)
scenarios={}
for folder in ('verified-window','verified-chrome','verified-markdown','verified-stress','verified-epub-visual','scale-100','scale-125','scale-200'):
    report=read(source/f'{folder}/fixture-00.json')
    assert report['exit_code']==0 and not report['errors'],folder
    scenarios[folder]=report
assert scenarios['verified-stress']['operations']==730
assert scenarios['verified-chrome']['operations']==14
write(output/'window-scenarios.json',scenarios)
test_log=(root/'artifacts/next-test-release.log').read_text(encoding='utf-8-sig')
atomic_bytes(output/'test-release.log',test_log.replace('\r\n','\n').encode('utf-8'))
assert 'FAILED' not in (output/'test-release.log').read_text(encoding='utf-8-sig')
pictures={
    'ui-pdf-fit.png':('verified-window','first.png'),
    'ui-pdf-zoom.png':('verified-window','feature-03.png'),
    'ui-pdf-tint-night.png':('verified-window','feature-05.png'),
    'ui-pdf-invert.png':('verified-window','feature-06.png'),
    'ui-pdf-spread.png':('verified-window','feature-09.png'),
    'ui-chrome-top.png':('verified-chrome','chrome-01.png'),
    'ui-chrome-bottom.png':('verified-chrome','chrome-02.png'),
    'ui-chrome-fullscreen.png':('verified-chrome','chrome-04.png'),
    'ui-chrome-settings.png':('verified-chrome','chrome-06.png'),
    'ui-control-zoom-wheel.png':('verified-chrome','chrome-09.png'),
    'ui-control-zoom-drag.png':('verified-chrome','chrome-10.png'),
    'ui-control-progress.png':('verified-chrome','chrome-11.png'),
    'ui-control-pan.png':('verified-chrome','chrome-12.png'),
    'ui-immersive.png':('verified-chrome','chrome-14.png'),
    'ui-whole-book-search.png':('verified-window','feature-11.png'),
    'ui-markdown.png':('verified-window','feature-15.png'),
    'ui-txt.png':('verified-window','feature-16.png'),
    'ui-ruby-math.png':('verified-window','feature-19.png'),
    'ui-ruby-math-night.png':('verified-window','feature-20.png'),
    'ui-vertical-limitation.png':('verified-window','feature-21.png'),
    'ui-fixed-canvas.png':('verified-window','feature-22.png'),
    'ui-epub-spread.png':('verified-epub-visual','view-02.png'),
    'ui-epub-settings.png':('verified-epub-visual','view-04.png'),
    'ui-epub-night.png':('verified-epub-visual','view-05.png'),
    'ui-markdown-night.png':('verified-markdown','view-05.png'),
}
image_manifest=[]
for target,(folder,name) in pictures.items():
    path=source/f'{folder}/fixture-00-screenshots/{name}'
    atomic_bytes(output/target,path.read_bytes())
    image_manifest.append({'file':target,'sha256':digest(path),'build':builds['qa'],'capture':'native Slint software renderer; original CC0 fixture'})
write(output/'images.json',image_manifest)
write(output/'manual-input.json',{'build':builds['production'],'method':'Sky real Windows input; original PDF fixture',
    'F11':'passed: title bar and taskbar disappear; document occupies the screen',
    'further_input':'stopped after user intervention; test windows retained',
    'wheel_drag_evidence':'verified-chrome uses native Slint control pointer/scroll dispatch, not OS mouse injection'})
rows=[]
for mode,data in memory.items():
    for sample in sorted({r['sample'] for r in data['old']}):
        before=[r for r in data['old'] if r['sample']==sample]
        after=[r for r in data['new'] if r['sample']==sample]
        metric=lambda records,k:statistics.median(r['steady_median'][k] for r in records)
        a=metric(before,'private_bytes');b=metric(after,'private_bytes')
        rows.append({'mode':mode,'sample':sample,'n_old':len(before),'n_new':len(after),
            'old_private_mib':a/1048576,'new_private_mib':b/1048576,'private_reduction_percent':100*(a-b)/a,
            'old_private_ws_mib':metric(before,'private_working_set')/1048576,'new_private_ws_mib':metric(after,'private_working_set')/1048576,
            'old_ws_mib':metric(before,'working_set')/1048576,'new_ws_mib':metric(after,'working_set')/1048576})
write(output/'summary.json',{'memory':rows,'startup':{k:v['summary'] for k,v in warm.items()},'startup_runs':sum(len(v['reports']) for v in warm.values()),'scenario_operations':730})
print(json.dumps({'memory':rows,'startup_runs':sum(len(v['reports']) for v in warm.values()),'images':len(image_manifest)},indent=2))
