"""Small original CC0 Kindle/FB2 regression inputs, no external converter.

The minimal KF8 file intentionally has one flow and no SKEL/FRAG index. Full KF8
indices, fonts and HUFF/CDIC are tested separately against pinned upstream inputs
and opt-in local books; this synthetic file alone is not a compatibility claim.
"""
import base64
import struct
import zipfile
from pathlib import Path

root = Path(__file__).resolve().parents[1] / 'tests/fixtures'
html = ('<html><head><title>Sae Kindle fixture</title></head><body>'
        '<h1 id="start">Kindle 与中文排版</h1>'
        '<p>完整保留文字、图片和链接。SEARCH-KINDLE</p>'
        '<img recindex="00001" width="240" height="160"/>'
        '<table border="1"><tr><td>Table A</td><td>Table B</td></tr></table>'
        '<p><a href="https://example.org">External reference</a></p>'
        + ''.join('<p>Paragraph %03d: native reading and searchable text.</p>' % n for n in range(90))
        + '</body></html>').encode('utf-8')

def record0(version, compression, texts, text_length):
    # Offsets documented in the pinned libmobi MOBIMobiHeader definition.
    header = bytearray(280)
    def word(offset, value): struct.pack_into('>L', header, offset, value)
    struct.pack_into('>HHIHHHH', header, 0, compression, 0, text_length, texts, 4096, 0, 0)
    header[16:20] = b'MOBI'
    for offset, value in [(20, 264), (24, 2), (28, 65001), (32, 123456), (36, version),
                          (80, 1+texts), (84, 280), (88, len(b'Sae Kindle fixture')),
                          (92, 9), (104, 6), (108, 1+texts)]:
        word(offset, value)
    for offset in [40,44,48,52,56,60,64,68,72,76,112,120,164,168,200,208,224,244,248,252,256,260]:
        word(offset, 0xffffffff)
    if version == 8: word(192, 0xffffffff)
    else: struct.pack_into('>HH', header, 192, 1, texts)
    return bytes(header) + b'Sae Kindle fixture'

def build(name, version=6, compression=1):
    content = html.replace(b'recindex="00001"', b'src="kindle:embed:0001?mime=image/png"') if version == 8 else html
    text = [content[n:n+4096] for n in range(0, len(content), 4096)]
    if compression == 2:
        # Valid PalmDOC literal runs also cover bytes >= 128 without ambiguity.
        text = [b''.join(bytes([len(p[n:n+8])]) + p[n:n+8] for n in range(0, len(p), 8)) for p in text]
    records = [record0(version, compression, len(text), len(content)), *text, (root/'illustration.png').read_bytes()]
    pdb = bytearray(78)
    pdb[:18] = b'Sae Kindle fixture'
    pdb[60:68] = b'BOOKMOBI'
    struct.pack_into('>H', pdb, 76, len(records))
    table = bytearray(); offset = 78 + 8 * len(records) + 2
    for n, data in enumerate(records):
        table += struct.pack('>LL', offset, n * 2); offset += len(data)
    (root/name).write_bytes(pdb + table + b'\0\0' + b''.join(records))

build('kindle-plain.mobi')
build('kindle-palmdoc.mobi', compression=2)
build('kindle-minimal.azw3', version=8)
image = base64.b64encode((root/'illustration.png').read_bytes()).decode('ascii')
fb2 = f'''<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
<description><title-info><genre>prose</genre><author><first-name>Sae</first-name><last-name>Fixture</last-name></author><book-title>Sae FictionBook 排版验证</book-title><lang>zh</lang></title-info></description>
<body><title><p>FictionBook 与中文目录</p></title>
<section id="c1"><title><p>第一章 · Native reading</p></title><p>SEARCH-FB2 中文段落与 English text。</p>
<image l:href="#illustration"/><p><strong>Strong emphasis</strong> 与 <emphasis>italic</emphasis>。</p>
<p><a l:href="#note" type="note">注释</a>与<a l:href="https://example.org">外部链接</a>。</p>
<table><tr><td>Table A</td><td>Table B</td></tr></table>
{''.join(f'<p>Paragraph {n:03d}: native text and images.</p>' for n in range(80))}</section>
<section id="c2"><title><p>第二章 · Reading on</p></title><p>SECOND-CHAPTER 全文搜索。</p></section></body>
<body name="notes"><section id="note"><title><p>注释</p></title><p>FOOTNOTE-FB2 内容。</p><p><a l:href="#c1">返回</a></p></section></body>
<binary id="illustration" content-type="image/png">{image}</binary></FictionBook>'''
(root/'reading.fb2').write_bytes(fb2.encode('utf-8'))
labels = ['序章 · 文字与图像', '第一辑 · 阅读札记', 'Morning letters', '第二辑 · 页边留白',
          'Notes and fragments — a longer chapter heading about native reading, typography, and the space around a page',
          '第三辑 · 纸上的时间：当章节标题很长时，也应当完整换行，而不是被滚动条或阅读面板遮挡', 'Afterword']
with zipfile.ZipFile(root/'core-epub3.epub') as source, zipfile.ZipFile(root/'catalog.epub','w') as output:
    for info in source.infolist():
        data = source.read(info)
        if info.filename.endswith('nav.xhtml'):
            text = data.decode('utf-8')
            for n, label in enumerate(labels, 1): text = text.replace(f'>Chapter {n:02}<',f'>{label}<')
            # Many variable-height rows exercise the virtualized viewport, its
            # bottom boundary and scrollbar without modifying the book body.
            extra = ''.join(f'<li><a href="c{n%7+1:02}.xhtml">'
                            + (f'Appendix {n:02} — Sovereignty, responsibility and a longer discussion of shared traditions'
                               if n % 2 else f'附录 {n:02} · 中英文混合标题与层级的显示、滚动和键盘导航检查')
                            + '</a></li>' for n in range(1, 25))
            text = text.replace('</ol></nav>', extra + '</ol></nav>')
            data = text.encode('utf-8')
        if info.filename.endswith('.opf'):
            # This additional original fixture deliberately uses EPUB3 nav,
            # rather than allowing the legacy NCX fallback to mask nav defects.
            assert b' toc="ncx"' in data
            data = data.replace(b' toc="ncx"',b'')
        output.writestr(info,data)
print('generated 5 original CC0 format and catalog fixtures')
