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

root=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--exe',type=Path,default=root/'artifacts/production/SaeReader.exe')
parser.add_argument('--name',default='SaeReader-0.1.0-preview.1')
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
inventory={'application':'AGPL-3.0-or-later','packages':packages,'note':'Full locked source superset, including unused optional/platform crates; default_windows_build distinguishes the normal Windows build.'}
(runtime/'dependency-inventory.json').write_text(json.dumps(inventory,indent=2,ensure_ascii=False),encoding='utf-8')
notices='''Sae Reader 0.1.0 preview

Application: AGPL-3.0-or-later (LICENSE). The matching source ZIP is delivered
alongside this executable, including modified MuPDF 1.27.2 / mupdf-rs 0.8.0,
Cargo.lock, all 580 locked registry packages and their original license files.
Slint is used under its GPLv3 open-source option. Other components retain the
terms in dependency-inventory.json and licenses/. Native MuPDF third-party
notices are included recursively in licenses/mupdf-sys-0.8.0 and in source.
License texts omitted from 18 published Cargo packages are supplemented from
upstream; URLs, commits, exceptions and checksums: licenses/supplemental-sources.json.

Windows fonts are loaded from the user's Windows installation and are not
redistributed. The included test fixtures are original CC0 content. This package
does not contain user ebooks, test profiles, SumatraPDF, Rust, MSVC or libclang.
The application icon was supplied by the owner as AI-generated artwork for this
application. Its original, provenance and size-conversion script are in the
corresponding source assets/ directory; it is not a CC0 test fixture.
'''
(runtime/'THIRD-PARTY-NOTICES.txt').write_text(notices,encoding='utf-8')
readme='''Sae Reader · Windows x64 预览版

双击 SaeReader.exe，或把 EPUB/PDF 路径作为命令行参数打开。
滚轮整页切换；顶部可选单页/双页（窄窗口自动暂用单页）。
Ctrl+O 打开，左右方向键 / PageUp / PageDown 翻页，Ctrl+F 当前页查找，
Ctrl+C 复制所选文字，F10 显隐工具栏，F11 全屏。
排版面板可改字体、字号、行距、段距、缩进、宽度、四边距和主题；
PDF 为固定版式，单独缩放。正常关闭后保存设置及每本书的逻辑位置。

默认数据：%LOCALAPPDATA%\\SaeReader\\state.json。
便携数据：以 SaeReader.exe --portable 启动，写入程序目录的 data/。
卸载：关闭后删除程序目录；如需删除个人历史，再自行删除上述数据目录。

此版实际支持 EPUB/PDF。TXT/MD、跨章节全文搜索尚未提供。
Ruby、MathML、竖排和固定版式不能笼统宣称完整兼容。详见 docs/。
本次修复及实测结果见 docs/feedback-20261026.md 和 performance-report.md。

Windows 10/11 x64。当前主机已实测；尚未做无开发环境的干净虚拟机验收。
依赖 Windows 系统 DLL 和 Microsoft Visual C++ v14 x64 运行库；
若提示缺少 VCRUNTIME140.dll / VCRUNTIME140_1.dll，需使用微软官方运行库。
本包不附带 Microsoft 的运行库 DLL，不要求安装 Rust 或开发工具才能运行。

源码、许可、图标与更新说明：
https://github.com/PoesisQ/Sae-CN/releases
请下载与本版本对应的 *-source.zip；完整许可已包含在本 Windows 包中。
'''
(runtime/'使用说明.txt').write_text(readme,encoding='utf-8')
shutil.copy2(root/'README.md',runtime/'README.md')
shutil.copy2(root/'CHANGELOG.md',runtime/'CHANGELOG.md')
(runtime/'assets').mkdir()
shutil.copy2(root/'assets/window-icon.png',runtime/'assets/window-icon.png')
shutil.copytree(root/'docs',runtime/'docs',dirs_exist_ok=True)
source_zip=dist/f'{args.name}-source.zip'
top=['Cargo.toml','Cargo.lock','build.rs','rust-toolchain.toml','README.md','CHANGELOG.md','LICENSE','.gitignore','.gitattributes','Windows_Reader_Codex_Engineering_Brief_v1_20261025.md']
folders=['src','ui','assets','scripts','tests','docs','vendor','legal']
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
        if previous.read(previous_prefix+'Cargo.lock')!=(root/'Cargo.lock').read_bytes():raise RuntimeError('Cannot reuse dependencies from a different Cargo.lock')
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
