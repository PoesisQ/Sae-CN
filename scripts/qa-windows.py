"""Native release QA harness. Only timings/counters leave the input folder.

Run outside the desktop sandbox so that Winit creates a real visible window.
No pre-warming service, working-set trimming, cache clearing or book rewriting.
"""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import time

root=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--exe',type=Path,default=root/'artifacts/qa/sae-reader-qa.exe')
parser.add_argument('--books',type=Path)
parser.add_argument('--runs',type=int,default=1)
parser.add_argument('--scenario',choices=['startup','idle','visual','frontispiece','home-motion','opening','stress','features','chrome','detents','navigation','contents','multipage','instruments','glass','surfaces','page-input'],default='startup')
parser.add_argument('--cover', choices=['blue','plasma','print'])
parser.add_argument('--static', action='store_true')
parser.add_argument('--spread',action='store_true')
parser.add_argument('--multi',action='store_true')
parser.add_argument('--fixture',type=Path)
parser.add_argument('--initial-state', type=Path, help='Seed the isolated profile from a previous fixture run')
parser.add_argument('--blank',action='store_true')
parser.add_argument('--scale',type=float)
parser.add_argument('--design',type=int,default=0)
parser.add_argument('--no-activate',action='store_true',help='Create the window without taking foreground focus on Windows')
parser.add_argument('--output',type=Path,default=root/'artifacts'/'window-qa')
args=parser.parse_args()
startupinfo=None
if args.no_activate and os.name=='nt':
    startupinfo=subprocess.STARTUPINFO(dwFlags=subprocess.STARTF_USESHOWWINDOW,wShowWindow=4) # SW_SHOWNOACTIVATE
args.output.mkdir(parents=True,exist_ok=True)
inputs=[]
if args.books:
    inputs=[(f'epub-{n:02}',p) for n,p in enumerate(sorted(args.books.glob('*.epub')),1)]
elif args.blank:
    inputs=[('blank',None)]
else:
    inputs=[('fixture',args.fixture or root/'tests/fixtures/core-epub3.epub')]
summaries=[]
for alias,path in inputs:
    reports=[]
    for n in range(args.runs):
        name=f'{alias}-{n:02}'
        report=args.output/f'{name}.json'
        report.unlink(missing_ok=True)
        profile=args.output/f'{name}-profile'
        profile.mkdir(exist_ok=True)
        preferences = (args.initial_state.read_text(encoding='utf-8') if args.initial_state else
                       json.dumps({'version':1,'spread':args.spread,'pdf_multi':args.multi}))
        (profile/'state.json').write_text(preferences,encoding='utf-8')
        env=dict(os.environ,SAE_READER_QA_REPORT=str(report.resolve()),SAE_READER_QA_SCENARIO=args.scenario,
                 SAE_READER_DATA_DIR=str(profile.resolve()),SAE_READER_DESIGN=str(args.design))
        if args.scenario in ('visual','frontispiece','home-motion','opening','features','chrome','detents','contents','multipage','instruments','glass','surfaces'):
            env['SAE_READER_QA_SCREENSHOTS']=str((args.output/f'{name}-screenshots').resolve())
        if args.scale:
            env['SLINT_SCALE_FACTOR']=str(args.scale)
        command=[str(args.exe.resolve())]
        if args.scenario == 'home-motion' or args.cover: command.append('--cover='+(args.cover or 'plasma'))
        if args.static: command.append('--motion=static')
        if path:command.append(str(path.resolve()))
        start=time.perf_counter()
        with (args.output/f'{name}.stderr').open('w',encoding='utf-8') as errors:
            result=subprocess.run(command,cwd=root,env=env,stdout=subprocess.DEVNULL,stderr=errors,timeout=190,startupinfo=startupinfo)
        duration=(time.perf_counter()-start)*1000
        value=json.loads(report.read_text(encoding='utf-8')) if report.exists() else {'errors':['no report']}
        value.update(sample=alias,run=n,process_total_ms=duration,exit_code=result.returncode)
        report.write_text(json.dumps(value,indent=2),encoding='utf-8')
        reports.append(value)
        print(json.dumps({'sample':alias,'run':n,'exit':result.returncode,'ready_ms':value.get('native_snapshot_ready_ms'),'errors':value['errors']}),flush=True)
    def aggregate(field):
        values=sorted(float(r[field]) for r in reports if r.get(field) is not None)
        if not values:return None
        return {'n':len(values),'median':statistics.median(values),'p95':values[max(0,__import__('math').ceil(.95*len(values))-1)],'max':max(values)}
    summaries.append({'sample':alias,'scenario':args.scenario,'spread_requested':args.spread,
        'native_snapshot_ready_ms':aggregate('native_snapshot_ready_ms'),
        'first_frame_assigned_ms':aggregate('first_frame_assigned_ms'),
        'process_total_ms':aggregate('process_total_ms'),'failures':sum(r['exit_code']!=0 or bool(r['errors']) for r in reports)})
(args.output/'summary.json').write_text(json.dumps(summaries,indent=2),encoding='utf-8')
raise SystemExit(1 if any(s['failures'] for s in summaries) else 0)
