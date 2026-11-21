"""Check the mode wheel silhouette and zoom disc in native Windows snapshots."""
import argparse
import json
import math
from pathlib import Path
from statistics import median

from PIL import Image


def profile(image, center, axle_y, radius, scale):
    deviations = []
    # Look for the meniscus against the neighbouring backdrop on a radial
    # scan. Transmitted page colour varies across the lens and between pages.
    for fraction in (-.90,-.74,-.48,-.24,.24,.48,.74,.90):
        ny = math.sqrt(1-fraction*fraction)
        def pixel(r):
            return image.getpixel((round(center+fraction*r),round(axle_y+ny*r)))
        def gradient(d):
            a, b = pixel(radius+d-scale), pixel(radius+d+scale)
            return max(abs(x-y) for x,y in zip(a,b))
        candidates = range(-math.ceil(2.5*scale),math.ceil(2.5*scale)+1)
        d = max(candidates,key=gradient)
        assert gradient(d) >= 8, (fraction,d,gradient(d))
        deviations.append(d)
    return deviations


def tick_angle_error(image, cx, cy, angle, length, ink, scale):
    # Fit the major axis of the painted ink inside each isolated tick. A row of
    # upright strokes at the correct coordinates must still fail this check.
    vx, vy = math.sin(angle), math.cos(angle)
    limit = (length/2 + 1.1)*scale
    backdrop = [image.getpixel((round(cx + sign*3.4*scale*vy), round(cy - sign*3.4*scale*vx))) for sign in (-1,1)]
    base = [(a+b)/2 for a,b in zip(*backdrop)]
    delta = [target-bg for target,bg in zip(ink,base)]
    norm = sum(channel**2 for channel in delta)
    assert norm > 1600, ("tick contrast", ink, base)
    points = []
    center_coverage = 0.
    for y in range(max(0, math.floor(cy-limit)), math.ceil(cy+limit)+1):
        for x in range(math.floor(cx-limit), math.ceil(cx+limit)+1):
            distance = math.hypot(x-cx,y-cy)
            if distance > limit:
                continue
            rgb = image.getpixel((x,y))
            coverage = sum((v-b)*d for v,b,d in zip(rgb,base,delta))/norm
            if distance <= 1.6*scale:
                center_coverage = max(center_coverage, coverage)
            if coverage > .25:
                points.append((x,y,min(1.,coverage)))
    # At 100% a 1px stroke can straddle two pixels; neither will have the
    # solid tick colour. Test local ink coverage against its own backdrop.
    if center_coverage < .4:
        return None
    assert len(points) >= 3, ("missing stroke", cx,cy)
    total = sum(p[2] for p in points)
    mx = sum(x*w for x,y,w in points)/total
    my = sum(y*w for x,y,w in points)/total
    xx = sum((x-mx)**2*w for x,y,w in points)
    yy = sum((y-my)**2*w for x,y,w in points)
    xy = sum((x-mx)*(y-my)*w for x,y,w in points)
    direction = .5*math.atan2(2*xy, xx-yy)
    agreement = abs(math.cos(direction)*vx + math.sin(direction)*vy)
    return math.degrees(math.acos(min(1.,agreement)))


def radial_marks(image, pose, scale, count, name):
    radius = pose["mode_radius"]
    center, axle_y = pose["mode_x"], pose["mode_y"]
    ink = (213,223,215) if "dark" in name else (61,80,77)
    angle_errors = []
    for tooth in range(-12, 25):
        degrees = (tooth / 6 - pose["mode_position"]) * pose["pitch"]
        angle = math.radians(degrees)
        length = 7 if tooth % 3 == 0 else 4
        if axle_y + (radius - 5 - length) * math.cos(angle) < 2:
            continue
        track = radius - 5 - length/2
        cx = (center + track * math.sin(angle)) * scale - .5
        cy = (axle_y + track * math.cos(angle)) * scale - .5
        error = tick_angle_error(image,cx,cy,angle,length,ink,scale)
        assert (error is not None) == (0 <= tooth <= (count - 1)*6), (name,tooth,degrees,error)
        if error is not None:
            assert error <= (17 if length == 7 else 27), (name,tooth,degrees,error)
            angle_errors.append(error)
    return {"ticks": len(angle_errors), "maximum_direction_error_deg": round(max(angle_errors),2)}


def tick_span(folder):
    data = json.loads((folder / "gestures.json").read_text(encoding="utf-8"))
    count = 2 if "epub-end" in data["captures"] else 3
    samples = {}
    for name, pose in data["view_poses"].items():
        with Image.open(folder / data["captures"][name]) as source:
            image = source.convert("RGB")
        radius, axle_y = pose["mode_radius"], pose["mode_y"]
        assert abs((radius + axle_y)/(2*radius)-.36) < .001
        assert abs(pose["header"] + pose["depth"] - radius - axle_y) < .1
        samples[name] = radial_marks(image, pose, pose["dpi"], count, name)
    assert "drum" in samples and "drum-dark" in samples, "Missing light or dark scale capture"
    return samples


def disc(image, cx, cy, scale):
    radius = 34 * scale
    radii = []
    # Sample the lip on all sides, including the diagonals where a square
    # has no edge. Three neighbouring rays keep a tick or the index from
    # dominating, and a narrow radial band excludes the soft contact shadow.
    tolerance = math.ceil(2 * scale)
    distances = range(-tolerance, tolerance + 1)
    for ray in range(16):
        angle = ray * math.tau / 16
        rays = [(math.cos(angle + off), math.sin(angle + off)) for off in (-.055, 0, .055)]
        def color(dx, dy, offset):
            return image.getpixel((round(cx + dx * (radius + offset) - .5),
                                   round(cy + dy * (radius + offset) - .5)))
        strengths = []
        for offset in distances:
            gradients = []
            for dx, dy in rays:
                a, b = color(dx, dy, offset - .6 * scale), color(dx, dy, offset + .6 * scale)
                gradients.append(max(abs(x - y) for x, y in zip(a, b)))
            strengths.append(median(gradients))
        peak = max(strengths)
        assert peak >= 8, ("Missing circular rim", ray, peak)
        edges = [(d, g) for d, g in zip(distances, strengths) if g >= .7 * peak]
        radii.append(radius + sum(d*g for d, g in edges) / sum(g for _, g in edges))
    width, height = radii[0] + radii[8], radii[4] + radii[12]
    assert max(abs(width - height), abs(width - 2*radius), abs(height - 2*radius)) <= 2.5 * scale
    return [round(width), round(height)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path, help="instruments/fixture-00.json")
    parser.add_argument("--wheel-samples", type=Path, nargs="+", help="check-wheel-input.py output directories")
    args = parser.parse_args()
    report = json.loads(args.report.read_text(encoding="utf-8"))
    assert report["scenario"] == "instruments" and not report["errors"]
    scale = report["dpi"]
    images = args.report.with_name(args.report.stem + "-screenshots")
    with Image.open(images / "instrument-04.png") as source:
        image = source.convert("RGB")
    pose = next(s for s in report["samples"] if s["step"] == 4)
    assert 95.9 <= pose["mode_radius"] <= 104.1
    assert abs(pose["mode_y"]/pose["mode_radius"] + .28) < .001
    outline = profile(image, pose["mode_x"] * scale, pose["mode_y"] * scale, pose["mode_radius"] * scale, scale)
    # The instruments sequence stops at the middle detent on this frame.
    marks = radial_marks(image, dict(pose, mode_position=1, pitch=36), scale, 3, "middle")
    with Image.open(images / "instrument-19.png") as source:
        image = source.convert("RGB")
    dock_right = (image.width + 516 * scale) / 2
    dimensions = disc(image, dock_right - 50 * scale, image.height - 60 * scale, scale)
    counts = [tick_span(folder) for folder in args.wheel_samples or []]
    print(json.dumps({"scale": scale, "circular_edge_error_px": outline, "middle_detent": marks,
                      "zoom_face_physical_px": dimensions, "ticks_on_visible_arc": counts}))


if __name__ == "__main__":
    main()
