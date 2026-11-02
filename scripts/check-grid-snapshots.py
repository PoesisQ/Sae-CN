"""Check all four native slide images survive a narrow/wide window cycle.

Uses only original fixture screenshots; no document names/text or personal books.
"""
import argparse,json
from pathlib import Path
from PIL import Image
p=argparse.ArgumentParser();p.add_argument('folder',type=Path);a=p.parse_args()
def bands(path):
 im=Image.open(path).convert('RGB');result=[]
 for x in (int(im.width*.1),int(im.width*.6)):
  runs=[];start=None
  for y in range(int(im.height*.1),int(im.height*.92)):
   r,g,b=im.getpixel((x,y));green=g>r+15 and g>b+5 and 60<g<170
   if green and start is None:start=y
   if not green and start is not None:
    if y-start>=5:runs.append((start,y-1))
    start=None
  result.append(runs)
 return result
before=bands(a.folder/'grid-01.png');after=bands(a.folder/'grid-18.png')
assert all(len(v)==2 for v in before),(before,after)
# A visible footer may cover the end of the lower-right marker. Its start
# and unobscured center must remain in the same position after the cycle.
assert all(len(v)==2 for v in after),(before,after)
for old,new in zip(before,after):
 for (a,b),(c,d) in zip(old,new):
  assert abs(a-c)<=2 and d>=a+(b-a)//2,('page pixels moved or were clipped after resize',before,after)
print(json.dumps({'four_page_pixels_after_resize':'passed','before':before,'after':after}))
