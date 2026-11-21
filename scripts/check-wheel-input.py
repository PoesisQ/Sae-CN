"""Exercise the reader wheels with Win32 pointer input in an isolated QA window."""
import argparse
import ctypes as ct
from ctypes import wintypes as wt
import json
import math
import os
from pathlib import Path
import subprocess
import time

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--exe", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--trace", type=Path, help="Optional 30 Hz recording coordinates relative to a 64.5 px radius")
parser.add_argument("--epub", action="store_true", help="Check the two-stop reading wheel on the EPUB fixture")
args = parser.parse_args()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
report = out / "native.json"
profile = out / "profile"
profile.mkdir(exist_ok=True)
(profile / "state.json").write_text('{"version":1,"spread":false,"pdf_multi":false}', encoding="utf-8")
env = dict(os.environ, SAE_READER_DATA_DIR=str(profile), SAE_READER_QA_REPORT=str(report),
           SAE_READER_QA_SCENARIO="wheel-input", SAE_READER_QA_SCREENSHOTS=str(out))
user = ct.WinDLL("user32", use_last_error=True)
user.SetProcessDpiAwarenessContext(ct.c_void_p(-4))
user.SetWindowPos.argtypes = (wt.HWND, wt.HWND, ct.c_int, ct.c_int, ct.c_int, ct.c_int, wt.UINT)
user.ClientToScreen.argtypes = (wt.HWND, ct.POINTER(wt.POINT))
user.WindowFromPoint.argtypes = (wt.POINT,)
user.WindowFromPoint.restype = wt.HWND
windows = []
events = []
checks = {}
captures = {}
view_poses = {}
original_cursor = wt.POINT()
user.GetCursorPos(ct.byref(original_cursor))
parked = None
holding = False
startup = subprocess.STARTUPINFO(dwFlags=subprocess.STARTF_USESHOWWINDOW, wShowWindow=4)
fixture = "core-epub3.epub" if args.epub else "core.pdf"
proc = subprocess.Popen([str(args.exe.resolve()), str(root / "tests/fixtures" / fixture)],
                        env=env, cwd=root, startupinfo=startup)


@ct.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
def find_window(window, _):
    pid = wt.DWORD()
    user.GetWindowThreadProcessId(window, ct.byref(pid))
    if pid.value == proc.pid and user.IsWindowVisible(window):
        windows.append(window)
    return True


def probe(label=None):
    label = label or f"sample-{len(events)}-{time.monotonic_ns()}"
    tmp = report.with_suffix(".tmp")
    tmp.write_text(label, encoding="ascii")
    tmp.replace(report.with_suffix(".probe"))
    until = time.monotonic() + 8
    while time.monotonic() < until:
        try:
            obj = json.loads(report.with_suffix(".sample").read_text(encoding="utf-8"))
            if obj["seq"] == label:
                if label.startswith("capture-"):
                    name = label[8:]
                    captures[name] = f"wheel-{obj['sample']}.png"
                    if not name.startswith("dial"):
                        view_poses[name] = {key: obj[key] for key in ("mode_x", "mode_y", "mode_radius", "pitch", "mode_position", "header", "depth", "dpi")}
                return obj
        except (OSError, ValueError):
            pass
        if proc.poll() is not None:
            raise RuntimeError("QA window exited")
        time.sleep(.006)
    raise RuntimeError(f"QA window did not answer {label}")


class MouseInput(ct.Structure):
    _fields_ = [("dx", wt.LONG), ("dy", wt.LONG), ("data", wt.DWORD),
                ("flags", wt.DWORD), ("time", wt.DWORD), ("extra", ct.c_size_t)]


class Input(ct.Structure):
    _fields_ = [("kind", wt.DWORD), ("mouse", MouseInput)]


def mouse_input(flags, data=0):
    item = Input(0, MouseInput(0, 0, data & 0xffffffff, flags, 0, 0))
    if user.SendInput(1, ct.byref(item), ct.sizeof(item)) != 1:
        raise OSError(ct.get_last_error(), "SendInput")


def await_input(description, accepted):
    # A system cursor move can precede delivery to the reader by a frame.
    # Sample gestures only after Winit received this particular input event.
    until = time.monotonic() + 2
    while True:
        pointer = probe()["pointer"]
        if accepted(pointer):
            return
        if time.monotonic() >= until:
            raise RuntimeError(f"Reader did not receive {description}; last input: {pointer}")


def move(x, y):
    global parked
    client = [round(x*dpi), round(y*dpi)]
    parked = wt.POINT(*client)
    user.ClientToScreen(hwnd, ct.byref(parked))
    if not user.SetCursorPos(parked.x, parked.y):
        raise OSError(ct.get_last_error(), "SetCursorPos")
    actual = wt.POINT()
    user.GetCursorPos(ct.byref(actual))
    if (actual.x, actual.y) != (parked.x, parked.y):
        raise RuntimeError(f"Pointer clipped by the desktop at {actual.x},{actual.y}, requested {parked.x},{parked.y}")
    if holding or user.WindowFromPoint(parked) == hwnd:
        await_input(f"client position {client}", lambda p: p["client_px"] == client)


def button(x, y, down):
    global holding
    move(x, y)
    if down and user.WindowFromPoint(parked) != hwnd:
        raise RuntimeError("Test window does not own the press point")
    mouse_input(0x02 if down else 0x04)
    holding = down
    await_input(f"left button down={down}", lambda p: p["left_down"] == down)


def wheel(direction):
    if user.WindowFromPoint(parked) != hwnd:
        raise RuntimeError("Test window does not own the wheel point")
    previous = probe()["pointer"]["wheel_events"]
    mouse_input(0x0800, direction*120)
    await_input("wheel impulse", lambda p: p["wheel_events"] != previous)


def reset_zoom():
    move(preset_x, preset_y)
    button(preset_x, preset_y, True)
    button(preset_x, preset_y, False)
    current = probe()
    assert abs(current["zoom"] - 1) < .001, ("preset", current)


def drag(name, points, arc=False):
    reset_zoom()
    start = probe()
    x, y = points[0]
    button(cx + x, cy + y, True)
    held = probe()
    assert held["pressed"] and abs(held["zoom"] - start["zoom"]) < .005, (name, "press", start, held)
    values = [held["zoom"]]
    violations = []
    for (px, py), (x, y) in zip(points, points[1:]):
        move(cx + x, cy + y)
        sample = probe()
        z = sample["zoom"]
        turn = math.atan2(px*y - py*x, px*x + py*y) if arc else 0
        if arc and abs(turn) > .06 and (z - values[-1]) * math.copysign(1, turn) < -.006:
            violations.append((round(turn, 3), round(values[-1], 3), round(z, 3)))
        values.append(z)
    button(cx + x, cy + y, False)
    last = probe()
    assert not last["pressed"] and last["pinned"] and last["page"] == start["page"], (name, last)
    events.append({"gesture": name, "samples": len(points), "zoom": [round(z, 4) for z in values],
                   "direction_errors": violations})
    assert not violations, (name, violations)
    return last["zoom"]


try:
    first = probe()
    user.EnumWindows(find_window, 0)
    assert windows, "cannot find native reader window"
    hwnd = windows[0]
    dpi = first["dpi"]
    x, y = first["mode_x"], first["header"] - 14
    # Keep physical and Win32 message coordinates aligned under mouse capture.
    # Leave enough space beneath the client for the vertical drag regression.
    if not user.SetWindowPos(hwnd, -1, 40, 20, 0, 0, 0x11):
        raise ct.WinError(ct.get_last_error())
    user.SetForegroundWindow(hwnd)
    parked = wt.POINT(round(x*dpi), round(y*dpi))
    user.ClientToScreen(hwnd, ct.byref(parked))
    user.SetCursorPos(parked.x, parked.y)
    time.sleep(.12)
    move(x, y)
    time.sleep(.25)
    a = probe("capture-drum")
    expanded = a["mode_radius"]
    assert 95.9 <= expanded <= 104.1 and abs(a["depth"] + a["header"] - expanded*.72) < .1 and a["mode"] == 0, a
    gen = a["generation"]
    wheel(-1)
    w = probe()
    assert w["mode"] == 0 and w["generation"] == gen, ("single impulse", w)
    time.sleep(.42)
    wheel(-1)
    wheel(-1)
    assert probe()["mode"] == 1, "two wheel impulses did not engage the next notch"
    time.sleep(.20)
    probe("capture-double" if args.epub else "capture-three-labels")
    move(x-145,16)
    time.sleep(.21)
    rest = probe("capture-rest-double")
    assert rest["mode"] == 1 and abs(rest["mode_radius"] - 88) < .1, rest
    # Reach the compact wheel from the rail, then skim the expanded rim.
    # Moving into the gap before the next button must put it back at rest.
    for pose, height, padding, expected in ((rest,12,4,expanded), (a,22,2,expanded), (a,22,7,88)):
        offset = math.sqrt((pose["mode_radius"]+padding)**2 - (height-pose["mode_y"])**2)
        move(x + offset, height)
        time.sleep(.21)
        pose = probe()
        assert abs(pose["mode_radius"] - expected) < .1 and pose["mode"] == 1, pose
    move(x,y)
    time.sleep(.21)
    if args.epub:
        wheel(-1)
        wheel(-1)
        time.sleep(.5)
        end = probe("capture-epub-end")
        assert end["mode"] == 1 and abs(end["mode_position"] - 1) < .005
        wheel(1)
        wheel(1)
        time.sleep(.5)
        assert probe()["mode"] == 0
        probe("dark")
        time.sleep(.23)
        probe("capture-drum-dark")
        report.with_suffix(".probe").write_text("finish", encoding="ascii")
        assert proc.wait(8) == 0
        assert not json.loads(report.read_text(encoding="utf-8"))["errors"]
        print("Both EPUB end stops verified")
        raise SystemExit(0)
    wheel(-1)
    wheel(-1)
    time.sleep(.5)
    last_stop = probe("capture-multi")
    assert last_stop["mode"] == 2 and abs(last_stop["mode_position"] - 2) < .005
    wheel(1)
    wheel(1)
    assert probe()["mode"] == 1, "could not return from the multi-page end stop"
    wheel(1)
    wheel(1)
    assert probe()["mode"] == 0, "reverse wheel input did not restore single page"
    before = probe()
    button(x, y, True)
    history = []
    travel = expanded * math.radians(a["pitch"])
    for fraction in (.08,.18,.32,.47,.55):
        delta = round(travel * fraction)
        move(x-delta, y)
        partial = probe(f"capture-held-{delta}")
        assert partial["mode_held"] and partial["mode"] == 0 and partial["generation"] == before["generation"]
        history.append(partial["mode_position"])
    assert all(b-a > .04 for a,b in zip([0]+history,history)), ("wheel stopped following the held pointer", history)
    checks["held_positions"] = history
    partial = probe()
    assert partial["mode"] == 0 and partial["generation"] == before["generation"], "partial drag submitted a layout"
    button(x-delta, y, False)
    assert probe()["mode"] == 0, "partial pull did not return"
    button(x, y, True)
    for fraction in (-.32, -.82, -1, -.58):
        dx = round(travel * fraction)
        move(x+dx, y)
    button(x+dx, y, False)
    assert probe()["mode"] == 1, "hysteresis at the saddle failed"
    button(x, y, True)
    for fraction in (-.68, -1.36, -2.65, -1.69):
        dx = round(travel * fraction)
        move(x+dx, y)
        held = probe()
        assert held["mode_held"], ("hard-stop capture", dx, held)
        if fraction == -2.65:
            assert held["mode_position"] > 1.99, ("hard stop", held)
    button(x+dx, y, False)
    assert probe()["mode"] == 1, "reverse from a hard stop contained dead travel"
    checks["view_detents"] = "partial return, engage, hysteresis, hard-stop reversal, two wheel impulses"

    move(first["dock_right"] - 42, first["dock_top"] + 20)
    time.sleep(.28)
    d = probe("capture-dial")
    cx, cy = d["center"]
    r = d["radius"]
    assert 33.8 <= r <= 34.2, d
    preset_x, preset_y, _ = d["presets"]
    for name, (sx, sy), (dx, dy), gain in [
        ("horizontal-rim", (0,-r*.90), (1,0), 2), ("horizontal-preset", (cx-preset_x,preset_y-cy), (-1,0), .5),
        ("vertical-rim", (-r*.90,0), (0,-1), 2), ("vertical-hub", (0,0), (0,1), .5),
    ]:
        z = drag(name, [(sx+dx*t,sy+dy*t) for t in range(0,145,8)])
        assert abs(z - gain) < .025, (name, z, gain)
    for start in [45, 135, 225, 315]:
        for direction, target in [(1, 4), (-1, .25)]:
            name = f"orbit-{start}-{direction}"
            pts = []
            for step in range(49):
                a = math.radians(start + direction * step * 7.5)
                rr = r*.93 * (1+.09*math.sin(2*a))
                pts.append((rr*math.cos(a),rr*math.sin(a)))
            z = drag(name, pts, True)
            assert abs(z - target) < .1, (name, z, target)
    if args.trace:
        samples = json.loads(args.trace.read_text(encoding="utf-8"))
        pts = [(x*r/64.5, y*r/64.5) for _, x, y, _ in samples]
        for sign in (1,-1):
            drag(f"recorded-{sign}", [(x*sign,y) for x,y in pts], True)
            history = events[-1]["zoom"]
            assert max(history) - min(history) > .5
    reset_zoom()
    probe("dark")
    time.sleep(.23)
    probe("capture-dial-dark")
    move(x, y)
    time.sleep(.23)
    probe("capture-drum-dark")
    report.with_suffix(".probe").write_text("finish",encoding="ascii")
    assert proc.wait(8) == 0
    assert not json.loads(report.read_text(encoding="utf-8"))["errors"]
    print(f"{len(events)} native drags, {sum(e['samples'] for e in events)} pointer samples, all checks passed")
finally:
    (out / "gestures.json").write_text(json.dumps({"checks": checks, "captures": captures,
                                                   "view_poses": view_poses, "events": events}, indent=2), encoding="utf-8")
    if holding:
        mouse_input(0x04)
    if parked is not None:
        current = wt.POINT()
        user.GetCursorPos(ct.byref(current))
        if (current.x,current.y) == (parked.x,parked.y):
            user.SetCursorPos(original_cursor.x,original_cursor.y)
    if proc.poll() is None:
        proc.terminate()
        proc.wait()
