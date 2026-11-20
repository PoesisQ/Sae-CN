"""Trace the client area of a reader process we start, through two fullscreen round trips."""
import argparse
import ctypes as ct
from ctypes import wintypes as wt
import json
import os
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--exe', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
user = ct.WinDLL('user32', use_last_error=True)
user.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
enum_proc = ct.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
user.EnumWindows.argtypes = [enum_proc, wt.LPARAM]
user.GetWindowThreadProcessId.argtypes = [wt.HWND, ct.POINTER(wt.DWORD)]
user.IsWindowVisible.argtypes = [wt.HWND]
user.GetWindowTextW.argtypes = [wt.HWND, wt.LPWSTR, ct.c_int]
user.IsZoomed.argtypes = [wt.HWND]
user.GetClientRect.argtypes = [wt.HWND, ct.POINTER(wt.RECT)]
user.PostMessageW.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]
user.MonitorFromWindow.argtypes = [wt.HWND, wt.DWORD]
user.MonitorFromWindow.restype = wt.HANDLE

class MonitorInfo(ct.Structure):
    _fields_ = [('size', wt.DWORD), ('monitor', wt.RECT), ('work', wt.RECT), ('flags', wt.DWORD)]

user.GetMonitorInfoW.argtypes = [wt.HANDLE, ct.POINTER(MonitorInfo)]
env = dict(os.environ, SAE_READER_DATA_DIR=str((args.output / 'profile').resolve()))
env.pop('SAE_READER_QA_REPORT', None)
startup = subprocess.STARTUPINFO(dwFlags=subprocess.STARTF_USESHOWWINDOW, wShowWindow=4)
book = Path(__file__).resolve().parents[1] / 'tests/fixtures/core.pdf'
process = subprocess.Popen([str(args.exe.resolve()), str(book)], env=env, startupinfo=startup)
hwnd = None

@enum_proc
def find_window(candidate, unused):
    global hwnd
    pid = wt.DWORD()
    user.GetWindowThreadProcessId(candidate, ct.byref(pid))
    if pid.value == process.pid and user.IsWindowVisible(candidate):
        title = ct.create_unicode_buffer(256)
        rect = wt.RECT()
        user.GetWindowTextW(candidate, title, len(title))
        if title.value.endswith('Sae Reader') and user.GetClientRect(candidate, ct.byref(rect)) and rect.right > 300:
            hwnd = candidate
            return False
    return True

def viewport():
    rect = wt.RECT()
    if not user.GetClientRect(hwnd, ct.byref(rect)):
        raise ct.WinError(ct.get_last_error())
    return [rect.right, rect.bottom]

def trace(action):
    start = time.perf_counter()
    sizes = [{'ms': 0, 'size': viewport()}]
    action()
    while time.perf_counter() - start < 0.75:
        size = viewport()
        if size != sizes[-1]['size']:
            sizes.append({'ms': round((time.perf_counter() - start) * 1000, 2), 'size': size})
        # Yield without the Windows timer quantum during the brief transition.
        time.sleep(0 if time.perf_counter() - start < 0.12 else 0.004)
    return sizes

def f11():
    scan = user.MapVirtualKeyW(0x7A, 0) << 16
    user.PostMessageW(hwnd, 0x0100, 0x7A, scan | 1)
    user.PostMessageW(hwnd, 0x0101, 0x7A, scan | 0xC0000001)

try:
    until = time.perf_counter() + 8
    while not hwnd and time.perf_counter() < until:
        user.EnumWindows(find_window, 0)
        time.sleep(0.02)
    if not hwnd:
        raise RuntimeError('Reader window unavailable')
    time.sleep(0.6)
    monitor = MonitorInfo(size=ct.sizeof(MonitorInfo))
    if not user.GetMonitorInfoW(user.MonitorFromWindow(hwnd, 2), ct.byref(monitor)):
        raise ct.WinError(ct.get_last_error())
    full = [monitor.monitor.right - monitor.monitor.left, monitor.monitor.bottom - monitor.monitor.top]
    initial = viewport()
    rounds = {}
    for name, enter in [('maximize', lambda: user.PostMessageW(hwnd, 0x0112, 0xF030, 0)), ('F11', f11)]:
        entering, leaving = trace(enter), trace(f11)
        # A restored-window flash is a second ordinary-sized client area AFTER
        # the first near-full-size event. Exclude the expected decoration change.
        was_large = False
        shrank = False
        for sample in entering:
            large = all(n >= target * 0.90 for n, target in zip(sample['size'], full))
            shrank |= was_large and not large
            was_large |= large
        rounds[name] = dict(enter=entering, exit=leaving, shrank_after_maximize=shrank,
                            fills_screen=entering[-1]['size'] == full,
                            restored=leaving[-1]['size'] == initial and not user.IsZoomed(hwnd))
    result = dict(initial=initial, monitor=full, rounds=rounds)
    (args.output / 'trace.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result), flush=True)
    if any(r['shrank_after_maximize'] or not r['fills_screen'] or not r['restored'] for r in rounds.values()):
        raise SystemExit(1)
finally:
    if hwnd:
        user.PostMessageW(hwnd, 0x0010, 0, 0)
    try:
        process.wait(timeout=4)
    except subprocess.TimeoutExpired:
        process.terminate()
        process.wait()
