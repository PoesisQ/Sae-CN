"""Deterministic, original CC0 document fixtures. Run with Python + fonttools.

No network access or system-font copying. EPUBs retain actual XHTML, CSS, images,
tables and links. Golden assertions describe content; they do not certify a reader.
"""
from pathlib import Path
import hashlib
import io
import json
import struct
import zipfile
import zlib

ROOT = Path(__file__).resolve().parents[1] / "tests" / "fixtures"
ROOT.mkdir(parents=True, exist_ok=True)


def png():
    w, h = 360, 180
    rows = bytearray()
    for y in range(h):
        rows.append(0)
        for x in range(w):
            rows.extend((80, 108, 96) if 20 < x < 340 and 20 < y < 160 else (248, 246, 241))
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(bytes(rows))) + chunk(b"IEND", b"")


IMAGE = png()
(ROOT / "illustration.png").write_bytes(IMAGE)
SVG = '<svg xmlns="http://www.w3.org/2000/svg" width="360" height="180" viewBox="0 0 360 180"><rect width="360" height="180" fill="#f8f6f1"/><circle cx="90" cy="90" r="60" fill="#506c60"/><path d="M190 140 L250 30 L320 140 Z" fill="#b86e47"/><text x="20" y="175" font-size="14">SVG circle and triangle</text></svg>'


def embedded_font():
    from fontTools.fontBuilder import FontBuilder
    from fontTools.pens.ttGlyphPen import TTGlyphPen
    fb = FontBuilder(1000, isTTF=True)
    glyph_order = [".notdef", "space", "E", "M", "B", "D"]
    fb.setupGlyphOrder(glyph_order)
    fb.setupCharacterMap({32: "space", **{ord(c): c for c in "EMBD"}})
    glyphs = {}
    for name in glyph_order:
        pen = TTGlyphPen(None)
        def box(x0, y0, x1, y1):
            pen.moveTo((x0, y0)); pen.lineTo((x0, y1)); pen.lineTo((x1, y1)); pen.lineTo((x1, y0)); pen.closePath()
        if name not in ("space", ".notdef"):
            box(80, 0, 160, 700)
            if name in "EBD":
                box(160, 620, 490, 700); box(160, 0, 490, 80)
            if name in "EB":
                box(160, 310, 450, 390)
            if name in "BD":
                box(490, 80, 570, 620)
            if name == "M":
                box(490, 0, 570, 700)
                pen.moveTo((160, 700)); pen.lineTo((325, 370)); pen.lineTo((490, 700)); pen.lineTo((490, 530)); pen.lineTo((325, 200)); pen.lineTo((160, 530)); pen.closePath()
        glyphs[name] = pen.glyph()
    fb.setupGlyf(glyphs)
    fb.setupHorizontalMetrics({g: (650, 0) for g in glyph_order})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable({"familyName": "Sae Fixture", "styleName": "Regular", "uniqueFontIdentifier": "SaeFixture-CC0-1", "fullName": "Sae Fixture Regular", "psName": "SaeFixture-Regular", "version": "Version 1.0", "copyright": "Original fixture glyphs. CC0-1.0."})
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    fb.setupPost(); fb.setupMaxp()
    fb.font.recalcTimestamp = False
    fb.font["head"].created = fb.font["head"].modified = 3400000000
    output = io.BytesIO(); fb.save(output)
    return output.getvalue()


FONT = embedded_font()
(ROOT / "fixture-font.ttf").write_bytes(FONT)
CSS = '''@font-face {font-family: Fixture; src: url(font.ttf)}
body {font-family:serif; color:#262a27; margin:24px; line-height:1.5}
h1 {font-size:1.6em} h2 {font-size:1.2em} p {margin:0 0 .6em;text-indent:1.5em}
img,svg {max-width:100%;height:auto} table {border-collapse:collapse;max-width:100%}
th,td {border:1px solid #506c60;padding:5px} blockquote{margin:1em 2em}
.embedded {font-family:Fixture;font-size:32px} code,pre{font-family:monospace}
'''
PARAGRAPH = ('春天的清晨，窗外的树影落在纸上。阅读应保留内容、层次与安静的留白。'
             '中文标点测试：“你好，世界！”（括号与引号）不能消失。'
             ' Quiet reading preserves images, tables, emphasis and meaningful links. '
             'The quick brown fox crosses the page; numbers 0123456789 remain searchable. ')


def xhtml(body, title="Fixture", extra=""):
    return f'<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="zh"><head><title>{title}</title><link rel="stylesheet" href="style.css"/>{extra}</head><body>{body}</body></html>'


def book(name, version="3.0", special=None, fixed=False):
    chapters = []
    for n in range(1, 31):
        body = f'<h1 id="chapter{n}">Chapter {n:02d} · 第{n}章</h1><p id="anchor{n}">ANCHOR-{n:02d} {PARAGRAPH}</p>'
        if n == 1:
            body += '''<h2 id="structures">Content structures</h2><p><em>Italic emphasis</em> and <strong>bold emphasis</strong>, x<sup>2</sup> and H<sub>2</sub>O.</p>
<blockquote>A quiet quotation with its own hierarchy.</blockquote><ol><li>First item</li><li>Second item<ul><li>Nested item</li></ul></li></ol>
<p><img src="image.png" alt="Original green rectangle illustration"/></p><p>Figure 1. A green rectangle on warm paper.</p>
<table><tr><th>Feature</th><th colspan="2">Expected result</th></tr><tr><td rowspan="2">Table spans</td><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>
<p class="embedded">EMBED</p><p><a href="#note1" epub:type="noteref">Footnote [1]</a> · <a href="c02.xhtml#anchor2">Next chapter anchor</a> · <a href="https://example.org/reader-fixture">External link requiring confirmation</a></p>
<aside id="note1" epub:type="footnote"><p>[1] FOOTNOTE-CONTENT. <a href="#structures">Return to source</a></p></aside>'''
        if n == 2:
            body += '<h2>Wide table: every column must remain reachable</h2><table><tr>' + ''.join(f'<th>Column {j}</th>' for j in range(1, 13)) + '</tr><tr>' + ''.join(f'<td>VALUE-{j:02d}</td>' for j in range(1, 13)) + '</tr></table>'
        body += ''.join(f'<p id="p{n}-{j}">{PARAGRAPH} Paragraph {n:02d}-{j:02d}.</p>' for j in range(18))
        if special is not None:
            body = special
        chapters.append(xhtml(body, f"Chapter {n:02d}", '<meta name="viewport" content="width=600,height=800"/>' if fixed else ''))
        if special is not None:
            break
    manifest = ''.join(f'<item id="c{n}" href="c{n:02d}.xhtml" media-type="application/xhtml+xml"/>' for n in range(1, len(chapters)+1))
    spine = ''.join(f'<itemref idref="c{n}"/>' for n in range(1, len(chapters)+1))
    navlinks = ''.join(f'<li><a href="c{n:02d}.xhtml#chapter{n}">Chapter {n:02d}</a>' + ('<ol><li><a href="c01.xhtml#structures">Structures and footnote</a></li></ol>' if n == 1 and special is None else '') + '</li>' for n in range(1, len(chapters)+1))
    nav = xhtml(f'<nav epub:type="toc" id="toc"><h1>Contents</h1><ol>{navlinks}</ol></nav>')
    ncx = '<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="urn:sae:fixture"/></head><docTitle><text>Reader fixture</text></docTitle><navMap>' + ''.join(f'<navPoint id="n{n}" playOrder="{n}"><navLabel><text>Chapter {n:02d}</text></navLabel><content src="c{n:02d}.xhtml#chapter{n}"/></navPoint>' for n in range(1,len(chapters)+1)) + '</navMap></ncx>'
    extra_meta = '<meta property="dcterms:modified">2026-10-25T00:00:00Z</meta>' if version == '3.0' else '<meta name="cover" content="image"/>'
    if fixed:
        extra_meta += '<meta property="rendition:layout">pre-paginated</meta><meta property="rendition:spread">auto</meta>'
    nav_item = '<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>' if version == '3.0' else ''
    opf = f'''<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="{version}" unique-identifier="uid" prefix="rendition: http://www.idpf.org/vocab/rendition/#"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="uid">urn:sae:{name}</dc:identifier><dc:title>Sae Reader · {name}</dc:title><dc:language>zh</dc:language><dc:creator>Original CC0 fixture</dc:creator><dc:rights>CC0-1.0</dc:rights>{extra_meta}</metadata><manifest>{manifest}{nav_item}<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="css" href="style.css" media-type="text/css"/><item id="image" href="image.png" media-type="image/png" {'properties="cover-image"' if version == '3.0' else ''}/><item id="svg" href="figure.svg" media-type="image/svg+xml"/><item id="font" href="font.ttf" media-type="application/vnd.ms-opentype"/></manifest><spine toc="ncx">{spine}</spine></package>'''
    entries = {'mimetype': b'application/epub+zip', 'META-INF/container.xml': b'<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>', 'OEBPS/content.opf': opf.encode(), 'OEBPS/nav.xhtml': nav.encode(), 'OEBPS/toc.ncx': ncx.encode(), 'OEBPS/style.css': CSS.encode(), 'OEBPS/image.png': IMAGE, 'OEBPS/figure.svg': SVG.encode(), 'OEBPS/font.ttf': FONT}
    entries.update({f'OEBPS/c{n:02d}.xhtml': content.encode() for n, content in enumerate(chapters, 1)})
    with zipfile.ZipFile(ROOT / name, 'w') as archive:
        for path, data in entries.items():
            entry = zipfile.ZipInfo(path, (2026, 10, 25, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_STORED if path == 'mimetype' else zipfile.ZIP_DEFLATED
            archive.writestr(entry, data)


book('core-epub2.epub', '2.0')
book('core-epub3.epub')
book('fonts.epub', special='<h1 id="chapter1">Original embedded font</h1><p class="embedded">EMBED</p><p>Compare the distinctive geometric glyphs with the original font fixture.</p>')
book('inline-svg-cover.epub', special='<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100%" height="100%" viewBox="0 0 360 180"><image width="360" height="180" xlink:href="./image.png"/></svg>')
book('advanced.epub', special='''<h1 id="chapter1">Advanced native EPUB probe</h1><img src="figure.svg" alt="SVG circle and triangle"/><p>Ruby: <ruby>漢<rt>かん</rt>字<rt>じ</rt></ruby></p><math xmlns="http://www.w3.org/1998/Math/MathML"><mfrac><mi>a</mi><mi>b</mi></mfrac><mo>+</mo><msup><mi>x</mi><mn>2</mn></msup></math><p dir="rtl">مرحبا بالعالم 123 שלום עולם</p><div style="float:left;width:30%"><img src="image.png" alt="Floating illustration"/></div><p>Text around float. 嵌套与浮动的可见性。</p>''')
book('vertical.epub', special='<div style="writing-mode:vertical-rl"><h1 id="chapter1">縦書きの検証</h1><p>日本語の縦書き。句読点、括弧、「注釈」を確認します。</p><p><ruby>漢字<rt>かんじ</rt></ruby>と画像。</p><img src="image.png" alt="挿絵"/></div>')
book('fixed.epub', special='<h1 id="chapter1" style="position:absolute;left:60px;top:40px">Fixed canvas 600 × 800</h1><img src="image.png" style="position:absolute;left:100px;top:250px;width:360px;height:180px" alt="Fixed position image"/><p style="position:absolute;left:60px;top:650px">This bottom caption must remain visible.</p>', fixed=True)


def pdf():
    objects = [b'', b'<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R >>', b'', b'<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman >>', b'<< /Type /Outlines /First 5 0 R /Last 5 0 R /Count 1 >>', b'<< /Title (Reader fixture) /Parent 4 0 R /Dest [6 0 R /Fit] >>']
    pages = []
    for n in range(1, 31):
        page_id = len(objects); stream_id = page_id + 1
        pages.append(page_id)
        lines = [f'Sae Reader - original PDF fixture - page {n:02d}', 'Text layer, search, selection and links must map to the real page.'] + [f'Paragraph {n:02d}-{j:02d}. The quick brown fox reads quietly. 0123456789.' for j in range(1, 25)]
        commands = 'BT /F1 13 Tf 50 750 Td 22 TL ' + ' T* '.join(f'({line}) Tj' for line in lines) + ' ET\n0.31 0.42 0.38 rg 50 80 360 60 re f\n'
        stream = commands.encode('ascii')
        objects.append(f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 3 0 R >> >> /Contents {stream_id} 0 R >>'.encode())
        objects.append(b'<< /Length ' + str(len(stream)).encode() + b' >>\nstream\n' + stream + b'endstream')
    objects[2] = ('<< /Type /Pages /Count 30 /Kids [' + ' '.join(f'{p} 0 R' for p in pages) + '] >>').encode()
    output = bytearray(b'%PDF-1.7\n%\xe2\xe3\xcf\xd3\n'); offsets = [0]
    for n in range(1, len(objects)):
        offsets.append(len(output)); output.extend(f'{n} 0 obj\n'.encode() + objects[n] + b'\nendobj\n')
    start = len(output); output.extend(f'xref\n0 {len(objects)}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]: output.extend(f'{offset:010d} 00000 n \n'.encode())
    output.extend(f'trailer\n<< /Size {len(objects)} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n'.encode())
    (ROOT / 'core.pdf').write_bytes(output)


pdf()
text = '\n\n'.join(f'第{n}章\n　　{PARAGRAPH}\n\n保留空行。\n    保留四格缩进。' for n in range(1, 101))
for encoding in ('utf-8', 'utf-16', 'gb18030'):
    (ROOT / f'core-{encoding}.txt').write_bytes(text.encode(encoding))
(ROOT / 'invalid-encoding.txt').write_bytes(b'\xff\xfe\x00')
(ROOT / 'core.md').write_text('''# 安静阅读 · Markdown

This **bold** and *italic* text preserves semantic emphasis.

> A quotation should have breathing room.

1. Ordered item
2. Another item
   - Nested list

```rust
fn main() {
    println!("Native text, image and table reading.");
}
```

| Format | Content |
| --- | --- |
| EPUB | 图片、表格、脚注 |
| PDF | Fixed page text layer |

![Original illustration](illustration.png)

[Confirmed external link](https://example.org/reader-fixture)

---

## Second heading

代码块策略：保留空格、等宽字体；长行必须可访问。
''', encoding='utf-8')
with zipfile.ZipFile(ROOT / 'hostile-traversal.epub', 'w') as z:
    z.writestr('mimetype', 'application/epub+zip'); z.writestr('../escape.xhtml', 'Rejected before engine parsing')
with zipfile.ZipFile(ROOT / 'hostile-ratio.epub', 'w', compression=zipfile.ZIP_DEFLATED) as z:
    z.writestr('mimetype', 'application/epub+zip'); z.writestr('bomb.xhtml', b'0' * 2_000_000)
manifest = {p.name: {'sha256': hashlib.sha256(p.read_bytes()).hexdigest(), 'bytes': p.stat().st_size} for p in sorted(ROOT.iterdir()) if p.suffix in ('.epub', '.pdf', '.txt', '.md', '.ttf', '.png') and p.name != 'SOURCES.md'}
(ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
print(f'Generated {len(manifest)} deterministic original fixtures in {ROOT}')
