"""Observe the production process externally; no child polling/QA feature needed.

The profiler terminates only the child it created after the measurement. This is
not a graceful-close test (native worker handle tests and window-close QA are separate).
"""
import argparse
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import statistics
import subprocess
import time

class Memory(ctypes.Structure):
    _fields_=[('cb',wintypes.DWORD),('faults',wintypes.DWORD)]+[(n,ctypes.c_size_t) for n in ('peak_ws','working_set','peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile','private_bytes','private_ws','shared_commit')]
kernel=ctypes.WinDLL('kernel32',use_last_error=True)
psapi=ctypes.WinDLL('psapi',use_last_error=True)
kernel.OpenProcess.argtypes=[wintypes.DWORD,wintypes.BOOL,wintypes.DWORD];kernel.OpenProcess.restype=wintypes.HANDLE
kernel.CloseHandle.argtypes=[wintypes.HANDLE]
kernel.GetProcessTimes.argtypes=[wintypes.HANDLE]+[ctypes.POINTER(ctypes.c_uint64)]*4
kernel.GetProcessHandleCount.argtypes=[wintypes.HANDLE,ctypes.POINTER(wintypes.DWORD)]
kernel.GetExitCodeProcess.argtypes=[wintypes.HANDLE,ctypes.POINTER(wintypes.DWORD)]
psapi.GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.c_void_p,wintypes.DWORD]
root=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--exe',type=Path,default=root/'target/x86_64-pc-windows-msvc/release/sae-reader.exe')
parser.add_argument('--books',type=Path)
parser.add_argument('--format',choices=['epub','pdf','azw3','mobi','fb2'],default='epub')
parser.add_argument('--fixture',type=Path)
parser.add_argument('--spread',action='store_true')
parser.add_argument('--seconds',type=float,default=8)
parser.add_argument('--runs',type=int,default=1)
parser.add_argument('--sumatra',action='store_true')
parser.add_argument('--pid',type=int,help='Observe an already running process without changing or terminating it')
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
if args.pid and (args.runs!=1 or args.books or args.fixture):parser.error('--pid requires one run and no input files')
if args.books:
    inputs=[(f'{args.format}-{n:02}',p) for n,p in enumerate(sorted(args.books.glob('*.'+args.format)),1)]
elif args.fixture:inputs=[('fixture',args.fixture)]
else:inputs=[('blank',None)]
reports=[]
for alias,path in inputs:
    for run in range(args.runs):
        profile=args.output/f'{alias}-{run:02}-profile';profile.mkdir(exist_ok=True)
        (profile/'state.json').write_text(json.dumps({'version':1,'spread':args.spread}),encoding='utf-8')
        env=dict(os.environ,SAE_READER_DATA_DIR=str(profile.resolve()))
        env.pop('SAE_READER_QA_REPORT',None)
        command=[str(args.exe.resolve())]
        if args.sumatra:command+=['-new-window','-appdata',str(profile.resolve()),'-view','single page','-zoom','fit page']
        if path:command.append(str(path.resolve()))
        process=None if args.pid else subprocess.Popen(command,cwd=root,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        handle=kernel.OpenProcess(0x410,False,args.pid if args.pid else process.pid);samples=[];start=time.perf_counter()
        def exit_code():
            if process:return process.poll()
            code=wintypes.DWORD()
            if kernel.GetExitCodeProcess(handle,ctypes.byref(code)) and code.value!=259:return code.value
            return None
        while exit_code() is None and time.perf_counter()-start<args.seconds:
            memory=Memory();memory.cb=ctypes.sizeof(memory)
            if handle and psapi.GetProcessMemoryInfo(handle,ctypes.byref(memory),memory.cb):
                creation=ctypes.c_uint64();exit_time=ctypes.c_uint64();sys=ctypes.c_uint64();user=ctypes.c_uint64();handles=wintypes.DWORD()
                kernel.GetProcessTimes(handle,ctypes.byref(creation),ctypes.byref(exit_time),ctypes.byref(sys),ctypes.byref(user))
                kernel.GetProcessHandleCount(handle,ctypes.byref(handles))
                samples.append({'ms':(time.perf_counter()-start)*1000,'private_bytes':memory.private_bytes,
                    'private_working_set':memory.private_ws,'working_set':memory.working_set,'peak_working_set':memory.peak_ws,
                    'peak_pagefile':memory.peak_pagefile,'cpu_ms':(sys.value+user.value)/10000,'handles':handles.value})
            time.sleep(.25)
        unexpected_exit=exit_code()
        if unexpected_exit is None and process:process.terminate();process.wait(timeout=10)
        if handle:kernel.CloseHandle(handle)
        steady=[s for s in samples if s['ms']>=3000]
        result={'sample':alias,'run':run,'spread_requested':args.spread,'instrumentation':'external Win32 process counters; no QA timer',
            'unexpected_exit':unexpected_exit,'terminated_by_profiler':unexpected_exit is None and process is not None,'attached':bool(args.pid),'samples':samples,
            'steady_median':{k:statistics.median(s[k] for s in steady) for k in ['private_bytes','private_working_set','working_set','handles']} if steady else None}
        if len(steady)>1:
            wall=steady[-1]['ms']-steady[0]['ms'];cpu=steady[-1]['cpu_ms']-steady[0]['cpu_ms']
            result.update(idle_wall_ms=wall,idle_cpu_ms=cpu,idle_one_core_percent=100*cpu/wall)
        reports.append(result)
        print(json.dumps({k:v for k,v in result.items() if k!='samples'}),flush=True)
(args.output/'summary.json').write_text(json.dumps(reports,indent=2),encoding='utf-8')
