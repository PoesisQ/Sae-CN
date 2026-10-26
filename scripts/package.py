"""Bundle this local evaluation build with matching, fully vendored source.

No library books, user profiles, tools, build caches or private QA images are
eligible inputs. Run cargo vendor and cargo metadata before this script.
"""
import argparse
from contextlib import ExitStack
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import zipfile
from urllib.parse import quote

root=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--exe',type=Path,default=root/'artifacts/production/SaeReader-0.3.0-preview.1/SaeReader.exe')
parser.add_argument('--name',default='SaeReader-0.3.0-preview.1')
parser.add_argument('--reuse-registry-from',type=Path,help='Reuse registry source from a checksum-verified prior source ZIP with identical Cargo.lock')
args=parser.parse_args()
if not re.fullmatch(r'[A-Za-z0-9._-]+',args.name):raise ValueError('Invalid package name')
dist=root/'dist';dist.mkdir(exist_ok=True)
runtime=dist/args.name
if runtime.exists():raise RuntimeError('Output folder already exists; choose a new --name so personal portable data can never be included')
runtime.mkdir()
shutil.copy2(args.exe,runtime/'SaeReader.exe')
shutil.copy2(root/'LICENSE',runtime/'LICENSE')
metadata=json.loads((root/'artifacts/cargo-metadata.json').read_text(encoding='utf-8-sig'))
deps=root/'artifacts/distribution-dependencies'
if not deps.is_dir():raise RuntimeError('Run cargo vendor before packaging')
tree=(root/'artifacts/used-dependencies.txt').read_bytes()
tree=tree.decode('utf-16') if tree.startswith(b'\xff\xfe') else tree.decode('utf-8-sig')
used=set(re.findall(r'^([\w-]+) v([^ |]+)',tree,re.M))
packages=[]
licenses=runtime/'licenses';licenses.mkdir(exist_ok=True)
for p in metadata['packages']:
    if p['name']=='sae-reader':continue
    name=f"{p['name']}-{p['version']}"
    folder=(deps/name) if p['source'] else Path(p['manifest_path']).parent
    if not folder.is_dir():raise RuntimeError(f'Missing source: {name}')
    in_build=(p['name'],p['version']) in used
    records=[]
    for f in folder.rglob('*'):
        if not f.is_file() or not re.match(r'^(licen[cs]e|copying|notice|copyright)',f.name,re.I):continue
        rel=f.relative_to(folder)
        records.append(str(rel).replace('\\','/'))
        if in_build:
            target=licenses/name/rel;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(f,target)
    packages.append({'name':p['name'],'version':p['version'],'license':p['license'],
        'license_file':p['license_file'],'default_windows_build':in_build,'notices':records,
        'supplemental_notices':[]})
    supplement=root/'legal/third-party'/name
    if supplement.is_dir():
        packages[-1]['supplemental_notices']=[str(f.relative_to(supplement)).replace('\\','/') for f in supplement.rglob('*') if f.is_file()]
        if in_build:shutil.copytree(supplement,licenses/name,dirs_exist_ok=True)
    if in_build and not records and not packages[-1]['supplemental_notices']:
        raise RuntimeError(f'Missing license text for build dependency: {name}')
shutil.copy2(root/'legal/third-party/sources.json',licenses/'supplemental-sources.json')
native_license=licenses/'libmobi-0.12'
native_license.mkdir()
shutil.copy2(root/'vendor/libmobi/COPYING',native_license/'LGPL-3.0-or-later.txt')
shutil.copy2(root/'vendor/libmobi/UPSTREAM.txt',native_license/'UPSTREAM.txt')
gpl3=next(f for f in (licenses/'slint-1.18.1').rglob('GPL-3.0-only.txt'))
shutil.copy2(gpl3,native_license/'GPL-3.0.txt')
native_components=[{'name':'libmobi','version':'0.12','commit':'906274205c11944b628da1c553b255acb1af7c55',
    'license':'LGPL-3.0-or-later','linkage':'static','source':'vendor/libmobi/src',
    'build':'build.rs; native/kindle.c; native/kindle_alloc.c; encryption disabled; existing MuPDF zlib',
    'notices':['licenses/libmobi-0.12/LGPL-3.0-or-later.txt','licenses/libmobi-0.12/GPL-3.0.txt','licenses/libmobi-0.12/UPSTREAM.txt']}]
inventory={'application':'AGPL-3.0-or-later','packages':packages,'native_components':native_components,'note':'Full locked source superset, including unused optional/platform crates; default_windows_build distinguishes the normal Windows build.'}
(runtime/'dependency-inventory.json').write_text(json.dumps(inventory,indent=2,ensure_ascii=False),encoding='utf-8')
notices='''Sae Reader 0.3.0 preview

Application: AGPL-3.0-or-later (LICENSE). The matching source ZIP is delivered
alongside this executable, including modified MuPDF 1.27.2 / mupdf-rs 0.8.0,
Cargo.lock, all 580 locked registry packages and their original license files.
Slint is used under its GPLv3 open-source option. Other components retain the
terms in dependency-inventory.json and licenses/. Native MuPDF third-party
notices are included recursively in licenses/mupdf-sys-0.8.0 and in source.
License texts omitted from 18 published Cargo packages are supplemented from
upstream; URLs, commits, exceptions and checksums: licenses/supplemental-sources.json.

libmobi 0.12, commit 906274205c11944b628da1c553b255acb1af7c55, is statically
linked under LGPL-3.0-or-later. Its source, our bridge/bounded allocator and the
complete rebuild inputs are in the matching source ZIP (vendor/libmobi/, native/,
build.rs). Encryption, writing tools and libxml2 are unbuilt; zlib is reused from
MuPDF. Rebuild the combined application from those sources with modifications to
libmobi as desired. License texts and upstream provenance: licenses/libmobi-0.12/.

Windows fonts are loaded from the user's Windows installation and are not
redistributed. The included test fixtures are original CC0 content. This package
does not contain user ebooks, test profiles, SumatraPDF, Rust, MSVC or libclang.
The application icon was supplied by the owner as AI-generated artwork for this
application. Its original, provenance and size-conversion script are in the
corresponding source assets/ directory; it is not a CC0 test fixture.
'''
(runtime/'THIRD-PARTY-NOTICES.txt').write_text(notices,encoding='utf-8')
readme='''Sae Reader · Windows x64 预览版

双击 SaeReader.exe，或把 EPUB/PDF/TXT/MD/AZW3/MOBI/FB2 路径作为命令行参数打开。
点击阅读区域左边上一页、右边下一页；滚轮整页切换（PDF 放大后平移）。鼠标靠近上沿展开工具条，选择单页/双页。
Ctrl+O 打开，左右方向键 / PageUp / PageDown 翻页，Ctrl+F 全文查找，
Ctrl+C 复制所选文字，F10 固定/自动收起工具栏，F11 全屏，Esc 退出全屏。
鼠标靠近下沿显示进度拖动条与缩放旋钮。旋钮随滚轮或拖动转动，在整页/铺满视区附近吸附，继续操作可脱离；点击恢复适页。
PDF 放大后 Shift+滚轮横移，中键或右键拖动平移；左键拖动仍可选字。
排版面板可改字体、字号、行距、段距、缩进、宽度、四边距和主题；
PDF 可单页/双页缩放、适页/适宽和平移，原色/随主题着色/反色可选。正常关闭后保存设置及每本书的逻辑位置。

默认数据：%LOCALAPPDATA%\\SaeReader\\state.json。
便携数据：以 SaeReader.exe --portable 启动，写入程序目录的 data/。
卸载：关闭后删除程序目录；如需删除个人历史，再自行删除上述数据目录。

此版支持 EPUB/PDF/TXT/MD/AZW3/MOBI/FB2，不含 DRM。Kindle 解析按需执行，转换树随即释放；FB2 使用原生引擎，缺少 ID 的目录使用临时副本补齐锚点。
部分 Ruby/MathML 使用原生矢量兼容，保留矢量文字选择；竖排和复杂固定版式仍有限制。详见 docs/。
本次修复及实测结果见 docs/iteration-0.3.md 和 docs/performance-report.md。

Windows 10/11 x64。当前主机已实测；尚未做无开发环境的干净虚拟机验收。
依赖 Windows 系统 DLL 和 Microsoft Visual C++ v14 x64 运行库；
若提示缺少 VCRUNTIME140.dll / VCRUNTIME140_1.dll，需使用微软官方运行库。
本包不附带 Microsoft 的运行库 DLL，不要求安装 Rust 或开发工具才能运行。

源码、许可、图标与更新说明：
https://github.com/PoesisQ/Sae-CN/releases
请下载与本版本对应的 *-source.zip；完整许可已包含在本 Windows 包中。
'''
(runtime/'使用说明.txt').write_text(readme,encoding='utf-8')
(runtime/'assets').mkdir()
shutil.copy2(root/'assets/window-icon.png',runtime/'assets/window-icon.png')
# The small runtime contains human-readable documentation. Historical raw
# measurements/screenshots stay in the full corresponding source ZIP and on the
# versioned repository, rather than inflating every executable download.
runtime_markdown=[root/'README.md',root/'CHANGELOG.md']+[
    p for p in (root/'docs').rglob('*.md') if 'evidence' not in p.relative_to(root/'docs').parts]
runtime_paths={p.resolve() for p in runtime_markdown}|{(root/'assets/window-icon.png').resolve()}
def copy_runtime_markdown(path):
    def link(match):
        target=match.group(1)
        if re.match(r'^[a-zA-Z][a-zA-Z0-9+.-]*:|^[/#]',target):return match.group(0)
        relative,separator,fragment=target.partition('#')
        resolved=(path.parent/relative).resolve()
        if not resolved.is_file() or resolved in runtime_paths:return match.group(0)
        try:rel=resolved.relative_to(root).as_posix()
        except ValueError:return match.group(0)
        url='https://github.com/PoesisQ/Sae-CN/blob/v'+args.name.removeprefix('SaeReader-')+'/'+quote(rel)
        return ']('+url+(separator+fragment if separator else '')+')'
    target=runtime/path.relative_to(root);target.parent.mkdir(parents=True,exist_ok=True)
    target.write_text(re.sub(r'\]\(([^()\s]+)\)',link,path.read_text(encoding='utf-8')),encoding='utf-8')
for path in runtime_markdown:copy_runtime_markdown(path)
(runtime/'docs/README.txt').write_text(
    'Raw measurements and original-fixture screenshots are in the matching full source ZIP.\n'
    'Documentation links to those artifacts use the versioned GitHub repository.\n'
    'The reader itself does not download or load these documents.\n',encoding='utf-8')
source_zip=dist/f'{args.name}-source.zip'
top=['AGENTS.md','Cargo.toml','Cargo.lock','build.rs','rust-toolchain.toml','README.md','CHANGELOG.md','LICENSE','.gitignore','.gitattributes','Windows_Reader_Codex_Engineering_Brief_v1_20261025.md']
folders=['src','ui','assets','scripts','tests','docs','vendor','legal','native']
files=[]
for item in top:
    path=root/item
    if path.is_file():files.append((path,Path(item)))
for folder in folders:
    path=root/folder
    if path.is_dir():
        for f in path.rglob('*'):
            if f.is_file() and not any(part in ('.git','__pycache__') for part in f.relative_to(root).parts):files.append((f,f.relative_to(root)))
if not args.reuse_registry_from:
    for f in deps.rglob('*'):
        if f.is_file():files.append((f,Path('dependencies')/f.relative_to(deps)))
config='''[build]
jobs = 4
target = "x86_64-pc-windows-msvc"
[source.crates-io]
replace-with = "vendored-sources"
[source.vendored-sources]
directory = "dependencies"
'''
revision=None
source_tree_clean=None
if (root/'.git').exists():
    revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
    source_tree_clean=not bool(subprocess.check_output(['git','status','--porcelain','--untracked-files=no'],cwd=root,text=True).strip())
class SequentialZipOutput:
    """No backpatch seeks on the slow source drive; ZIP uses data descriptors."""
    def __init__(self, file): self.file = file
    def write(self, data): return self.file.write(data)
    def flush(self): self.file.flush()

with ExitStack() as stack:
    previous=None; previous_entries=[]; previous_prefix=''
    if args.reuse_registry_from:
        previous_name=args.reuse_registry_from.name.removesuffix('-source.zip')
        previous_manifest=json.loads(args.reuse_registry_from.with_name(previous_name+'-manifest.json').read_text(encoding='utf-8'))
        expected=next(f['sha256'] for f in previous_manifest['files'] if f['file']==args.reuse_registry_from.name)
        check=hashlib.sha256()
        with args.reuse_registry_from.open('rb') as file:
            for block in iter(lambda:file.read(1024*1024),b''):check.update(block)
        if check.hexdigest()!=expected:raise RuntimeError('Previous source ZIP checksum mismatch')
        previous=stack.enter_context(zipfile.ZipFile(stack.enter_context(args.reuse_registry_from.open('rb',buffering=1024*1024))))
        previous_prefix=previous_name+'-source/'
        def dependency_lock(data):
            value=data.decode('utf-8-sig').replace('\r\n','\n')
            value=re.sub(r'(name = "sae-reader"\nversion = ")[^"]+("\n)',r'\1APPLICATION_VERSION\2',value)
            # The sole new application edge is build-only cc, already locked.
            # Package versions/checksums and all other edges remain verified.
            value=re.sub(r'(\[\[package\]\]\nname = "sae-reader"\n)(.*?)(?=\n\[\[package\]\]|\Z)',
                         lambda match:match.group(0).replace(' "cc",\n',''),value,flags=re.S)
            # Only this additional edge is permitted; the Winit package itself
            # was already present. All versions/checksums/other edges must match.
            value=re.sub(r'(\[\[package\]\]\nname = "slint"\n)(.*?)(?=\n\[\[package\]\]|\Z)',
                         lambda match:match.group(0).replace(' "i-slint-backend-winit",\n',''),value,flags=re.S)
            return value
        if dependency_lock(previous.read(previous_prefix+'Cargo.lock'))!=dependency_lock((root/'Cargo.lock').read_bytes()):raise RuntimeError('Cannot reuse sources after dependency lock changes')
        previous_entries=[f for f in previous.infolist() if f.filename.startswith(previous_prefix+'dependencies/') and not f.is_dir()]
        previous_crates={f.filename[len(previous_prefix+'dependencies/'):].split('/')[0] for f in previous_entries}
        expected_crates={f"{p['name']}-{p['version']}" for p in metadata['packages'] if p['source']}
        if previous_crates!=expected_crates:raise RuntimeError('Previous dependency source coverage mismatch')
        print(f'Reusing {len(previous_entries)} registry source entries from verified {args.reuse_registry_from.name}',flush=True)
    output=stack.enter_context(source_zip.open('wb',buffering=1024*1024))
    z=stack.enter_context(zipfile.ZipFile(SequentialZipOutput(output),'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6,strict_timestamps=False))
    for index,(file,rel) in enumerate(sorted(files,key=lambda pair:str(pair[1])),1):
        z.write(file,str(Path(args.name+'-source')/rel))
        if index%2000==0:print(f'Current project source entries: {index}/{len(files)}',flush=True)
    for index,entry in enumerate(previous_entries,1):
        info=zipfile.ZipInfo(args.name+'-source/'+entry.filename[len(previous_prefix):],date_time=entry.date_time)
        info.compress_type=zipfile.ZIP_DEFLATED;info.external_attr=entry.external_attr
        z.writestr(info,previous.read(entry))
        if index%5000==0:print(f'Verified registry source entries: {index}/{len(previous_entries)}',flush=True)
    z.writestr(args.name+'-source/.cargo/config.toml',config)
    if revision:z.writestr(args.name+'-source/SOURCE_REVISION.txt',revision+'\n')
    z.writestr(args.name+'-source/SOURCE-BUILD.txt','Matching local source. Install VS2022 v143 C++ / Windows SDK, run scripts/bootstrap.ps1 and scripts/env.ps1, then cargo build --release --locked --offline. All locked registry dependencies are included in dependencies/. See README.md and docs/.\n')
runtime_zip=dist/f'{args.name}-windows-x64.zip'
with runtime_zip.open('wb',buffering=1024*1024) as output:
    with zipfile.ZipFile(SequentialZipOutput(output),'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6,strict_timestamps=False) as z:
        for f in sorted(runtime.rglob('*')):
            if f.is_file():z.write(f,str(Path(args.name)/f.relative_to(runtime)))
def digest(path):
    hash=hashlib.sha256()
    with path.open('rb') as file:
        for block in iter(lambda:file.read(1024*1024),b''):hash.update(block)
    return hash.hexdigest()
manifest={'name':args.name,'source_commit':revision,'source_tree_clean':source_tree_clean,'files':[{'file':p.name,'bytes':p.stat().st_size,'sha256':digest(p)} for p in [runtime/'SaeReader.exe',source_zip,runtime_zip]],'locked_registry_packages':len(list(deps.iterdir()))}
(dist/f'{args.name}-manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
print(json.dumps(manifest,indent=2))
