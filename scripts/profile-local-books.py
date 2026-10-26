"""Local-only profiling; no book bytes, paths, titles, images or text in reports."""
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import subprocess
import sys
import time

class Memory(ctypes.Structure):
    _fields_=[('cb',wintypes.DWORD),('faults',wintypes.DWORD)]+[(n,ctypes.c_size_t) for n in ('peak_ws','working_set','peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile','private_bytes','private_ws','shared_commit')]

kernel=ctypes.WinDLL('kernel32',use_last_error=True)
psapi=ctypes.WinDLL('psapi',use_last_error=True)
kernel.OpenProcess.argtypes=[wintypes.DWORD,wintypes.BOOL,wintypes.DWORD];kernel.OpenProcess.restype=wintypes.HANDLE
kernel.CloseHandle.argtypes=[wintypes.HANDLE]
psapi.GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.c_void_p,wintypes.DWORD]
root=Path(__file__).resolve().parents[1]
extension=sys.argv[2] if len(sys.argv)>2 else 'epub'
if extension not in ('epub','pdf'):raise ValueError('Only EPUB/PDF native profiling')
books=sorted(Path(sys.argv[1]).glob(f'*.{extension}')) if len(sys.argv)>1 else []
output=root/'artifacts'/('local-bench' if extension=='epub' else 'local-bench-pdf');output.mkdir(parents=True,exist_ok=True)
reports=[]
for number,path in enumerate(books,1):
    alias=f'{extension}-{number:02}'
    with (output/f'{alias}.jsonl').open('w',encoding='utf-8') as log,(output/f'{alias}.stderr').open('w',encoding='utf-8') as errors:
        start=time.perf_counter()
        process=subprocess.Popen([str(root/'target/x86_64-pc-windows-msvc/release/load-bench.exe'),str(path)],stdout=log,stderr=errors,creationflags=subprocess.CREATE_NO_WINDOW)
        handle=kernel.OpenProcess(0x410,False,process.pid)
        samples=[];timed_out=False
        while process.poll() is None:
            memory=Memory();memory.cb=ctypes.sizeof(memory)
            if handle and psapi.GetProcessMemoryInfo(handle,ctypes.byref(memory),memory.cb):
                samples.append({'ms':(time.perf_counter()-start)*1000,'private_bytes':memory.private_bytes,'private_working_set':memory.private_ws,'working_set':memory.working_set})
            if time.perf_counter()-start>30:
                process.kill();process.wait();timed_out=True;break
            time.sleep(.05)
        if handle:kernel.CloseHandle(handle)
    report={'sample':alias,'file_bytes':path.stat().st_size,'elapsed_ms':(time.perf_counter()-start)*1000,'exit_code':process.returncode,'timed_out':timed_out,'memory_samples':samples}
    reports.append(report)
    print(json.dumps({k:v for k,v in report.items() if k!='memory_samples'}),flush=True)
(output/'summary.json').write_text(json.dumps(reports,indent=2),encoding='utf-8')
