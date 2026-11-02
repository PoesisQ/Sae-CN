"""Generate original CC0 PDF fixtures for adaptive multi-page reading."""
from pathlib import Path
import json,hashlib
root=Path(__file__).resolve().parents[1]/'tests/fixtures'
def pdf(name,sizes):
 objs=[b'',b'<< /Type /Catalog /Pages 2 0 R >>',b'',b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'];pages=[]
 for i,(w,h) in enumerate(sizes):
  p=len(objs);pages.append(p);stream=f'BT /F1 24 Tf 50 {h-70} Td (PAGE-{i+1:02d} Native slide fixture) Tj 0 -65 Td (Selection and search on every visible page.) Tj ET\n0.31 0.42 0.38 rg 50 40 180 45 re f\n'.encode()
  objs.extend([f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Resources << /Font << /F1 3 0 R >> >> /Contents {p+1} 0 R /Annots [{p+2} 0 R] >>'.encode(),b'<< /Length '+str(len(stream)).encode()+b' >>\nstream\n'+stream+b'endstream',b'<< /Type /Annot /Subtype /Link /Rect [50 40 230 85] /Dest [4 0 R /Fit] >>'])
 objs[2]=f'<< /Type /Pages /Count {len(pages)} /Kids ['.encode()+b' '.join(f'{p} 0 R'.encode() for p in pages)+b'] >>'
 out=bytearray(b'%PDF-1.7\n');offsets=[0]
 for n,obj in enumerate(objs[1:],1):offsets.append(len(out));out+=f'{n} 0 obj\n'.encode()+obj+b'\nendobj\n'
 xref=len(out);out+=f'xref\n0 {len(objs)}\n0000000000 65535 f \n'.encode()
 for off in offsets[1:]:out+=f'{off:010d} 00000 n \n'.encode()
 out+=f'trailer\n<< /Size {len(objs)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode();(root/name).write_bytes(out)
pdf('slides.pdf',[(960,540)]*9)
pdf('mixed-pages.pdf',[(960,540)]*4+[(595,842)]*4+[(720,720)])
manifest=json.loads((root/'manifest.json').read_text(encoding='utf-8')) if (root/'manifest.json').exists() else {}
for name in ('slides.pdf','mixed-pages.pdf'):
 data=(root/name).read_bytes();manifest[name]={'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)}
(root/'manifest.json').write_bytes((json.dumps(manifest,indent=2)+'\n').encode())
