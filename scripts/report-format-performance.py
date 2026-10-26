"""Render the 0.3 report from the collected, build-verified anonymous evidence."""
import json
from pathlib import Path

root=Path(__file__).resolve().parents[1]
evidence=root/'docs/evidence/iteration-0.3'
read=lambda name:json.loads((evidence/name).read_text(encoding='utf-8'))
memory=read('memory-summary.json');startup=read('startup-summary.json');builds=read('builds.json')
raw=read('measurements-raw.json')
mem={(r['version'],r['case']):r for r in memory['memory']}
warm={(r['version'],r['case']):r for r in startup['startup']}
paired=[x['case'] for x in memory['inputs'] if x['paired']]
added=[x['case'] for x in memory['inputs'] if not x['paired']]
labels={'blank':'空白','pdf-single':'PDF 单页','pdf-double':'PDF 双页',
        'azw3-single':'AZW3 实书单页','azw3-double':'AZW3 实书双页',
        'mobi-hybrid':'MOBI HUFF 混合样本','fb2-single':'FB2 原创样本'}
for n in range(1,4):
    for mode,text in [('single','单页'),('double','双页')]:labels[f'epub-{n:02}-{mode}']=f'epub-{n:02} {text}'
mib=lambda x:f'{x/1048576:.2f}'
memory_rows=[]
for case in paired:
    a=mem['previous',case]['result'];b=mem['current',case]['result']
    memory_rows.append(f"| {labels[case]} | {mib(a['private_bytes'])} | {mib(b['private_bytes'])} | {(b['private_bytes']-a['private_bytes'])/1048576:+.2f} | {mib(a['private_working_set'])} / {mib(b['private_working_set'])} | {mib(a['working_set'])} / {mib(b['working_set'])} |")
added_rows=[]
for case in added:
    r=mem['current',case]['result']
    added_rows.append(f"| {labels[case]} | {mib(r['private_bytes'])} | {mib(r['private_working_set'])} | {mib(r['working_set'])} | {mib(r['peak_private_bytes'])} |")
warm_rows=[]
for version,case in warm:
    if version!='current':continue
    a=warm.get(('previous',case));b=warm[version,case]['result']
    old=f"{a['result']['median']:.2f}" if a else '—'
    warm_rows.append(f"| {labels[case]} | {old} | {b['median']:.2f} | {b['p95']:.2f} | {b['max']:.2f} |")
cpu_rows=[]
for case in paired+added:
    a=mem.get(('previous',case));b=mem['current',case]['result']
    old=f"{a['result']['idle_one_core_percent']/32:.4f}%" if a else '—'
    cpu_rows.append(f"| {labels[case]} | {old} | {b['idle_one_core_percent']/32:.4f}% |")
previous=builds['production']['previous'];current=builds['production']['current']
current_reports=[d for case in raw['final']['startup'].values() for d in case['reports']]
assert len(current_reports)==160 and all(not d['errors'] and d['exit_code']==0 for d in current_reports)
geometry_mismatches=[]
for d in current_reports:
    if not d['document']:continue
    first=d['frames'][0];width,height=d['viewport']
    if first['display'][0]>width+2 or first['display'][1]>height+2 or first['dpi']!=d['dpi']:
        geometry_mismatches.append((d['sample'],d['run']))
assert not geometry_mismatches,'First rendered page did not match the final native viewport'
stress=read('window-scenarios.json')['stress']
report=f'''# 0.3 最终 Windows 构建性能实测

新增 AZW3/MOBI/FB2 后，普通 EPUB/PDF 的稳态私有内存仍处于上一版的量级。移除固定
启动等待前后分别测量，旧记录保留；没有隐藏图像、降低清晰度、裁剪工作集或常驻预热。
本报告不宣称 G5 全项验收。

## 机器与口径

2026-10-27，Windows 11 10.0.26200，Core i9-14900HX（24 核/32 逻辑处理器），约 16GB
物理内存。实际 Windows DPI 150%，默认客户区 1180×840 逻辑像素。当前桌面的后台程序、
存储负载和杀毒状态未隔离，存储型号/卷映射未重新独立核对。测量期间暂停编译。

- 普通发行程序由外部 Win32 计数器采样；每项 3 个独立进程、每次 8 秒，取 3 秒后的
  各采样点中位数，再取三轮中位数。采样间隔 250ms；所列「采样峰值」是采样所见最大
  Private Bytes，可能漏过更短峰值，原始系统峰值计数也保留。只结束采样器自己创建的子进程。
- 首屏使用独立 QA 构建，每项 20 个新进程；从 main 到真实页面赋值和原生软件渲染快照，
  **不包含 main 前加载，也不是 OS 合成器可见/可交互时间**。p95 使用 nearest-rank。
  未清空 OS 缓存，没有真正冷启动结果。
- 完整初测包含旧版 0.2 与启动优化前 0.3 的 66 次内存、260 次暖首屏。最终程序又测
  39 次内存、160 次暖首屏；主表采用原始 0.2 基线和最终 0.3。原始完整批次合计
  105 次内存、420 次暖首屏，全部保留；最终二进制不混用中间构建。
- 初测中 PDF 首屏中位数曾为旧版 273.00ms、新版 302.03ms；其原生工作线程中位数
  分别约 18.74/18.94ms。检查后移除每次固定的 100ms 等待：先核对真实客户区与 Slint
  视区/有效 DPI，已就绪便打开；未就绪保留单次延迟。后续也修复 FB2 页边背景，
  因而不能将两批所有差值精确归因于这一个等待。

构建与原始数据：[builds.json](evidence/iteration-0.3/builds.json)、
[逐点数据](evidence/iteration-0.3/measurements-raw.json)。书籍只读，公开记录使用匿名编号。

## 普通发行程序内存

单位 MiB。Private 为私有提交内存，Private WS 为驻留私有页，Total WS 含共享/映射页。
字体映射仍占物理 RAM；缓存预算不是整个进程内存上限。

| 场景 | Private 0.2 | Private 0.3 | 差值 | Private WS 旧 / 新 | Total WS 旧 / 新 |
|---|---:|---:|---:|---:|---:|
{chr(10).join(memory_rows)}

差值包括当前桌面状态与分配器波动，不把小幅变化包装成新的大比例内存优化。

| 新格式 | Private | Private WS | Total WS | 采样私有峰值 |
|---|---:|---:|---:|---:|
{chr(10).join(added_rows)}

AZW3 是本地 3,605,858 B 的 KF8 HUFF/CDIC 实书；MOBI 是固定上游输入；FB2 是原创样本。
未用这些少量样本外推所有 Kindle/FB2，解析预算和格式边界见 [格式支持](format-support.md)。
初测 epub-02 单页曾在约 252ms 的一个采样点达到 46.42 MiB，随后回到约 30.8 MiB；
该瞬时点保留在优化前数据，未作为稳态也未删除。

## 暖首屏

单位 ms；每个格子的当前版本样本数为 20。没有旧版支持的新格式以「—」表示。

| 场景 | 0.2 median | 0.3 median | 0.3 p95 | 0.3 max |
|---|---:|---:|---:|---:|
{chr(10).join(warm_rows)}

最终 160 次均正常退出、无 QA 错误。首张页面的 DPI 与最终视区一致，显示宽高没有越过
最终视区；检查没有用低分辨率临时页面当作达标首屏。300ms 仍只是挑战目标，且当前
快照口径不能替代可交互首帧验收。

## 空闲 CPU 与连续操作

单位为整机百分比：进程 CPU 时间除以墙钟时间及 32 个逻辑处理器。Windows CPU 计数
有 15.625ms 粒度，短观察中的 0 不表示永久绝对零。保留每轮结果，不删波动较高的样本。

| 场景 | 0.2 整机 CPU | 0.3 整机 CPU |
|---|---:|---:|
{chr(10).join(cpu_rows)}

旋钮仅输入后有限动画，渲染仅输入后单次节流；格式转换按需执行，没有新空闲轮询。
最终 [730 项连续操作](evidence/iteration-0.3/window-scenarios.json) 的私有内存从
{stress['baseline']['private_bytes']:,} B 到 {stress['end']['private_bytes']:,} B，句柄
{stress['baseline']['handles']} → {stress['end']['handles']}，最大位图缓存 {stress['max_cache_bytes']:,} B。
这是短时压力记录，不替代长期泄漏、隐藏窗口和所有文档压力验收。

## 体积与发行

普通 EXE 为 **{current['bytes']:,} B（{mib(current['bytes'])} MiB）**，旧版为
{previous['bytes']:,} B；增加 {(current['bytes']-previous['bytes'])/1024:.0f} KiB
（{100*(current['bytes']/previous['bytes']-1):.2f}%）。DLL 导入集合与 0.2 相同。
libmobi 静态编译，复用已有 zlib，没有 Python、Calibre、浏览器或额外转换进程依赖。

Windows ZIP 只带可阅读说明和完整许可；历史原始数据/截图放入完整对应源码 ZIP，
减少下载重复历史图片的开销。这是发行包整理，不冒充运行内存下降；精确 ZIP 大小和
SHA256 见 Release 清单。所有 580 个锁定 registry 包和本地原生修补仍在完整源码内。

最终 production SHA256：`{current['sha256']}`。
真正冷启动、干净 Windows VM、跨显示器 DPI、完整无障碍和统一参照阅读器验收仍未完成。
上一轮四格式、10 份 PDF 与 Sumatra 参照保留在 [0.2 报告](performance-0.2.md)。
'''
(root/'docs/performance-0.3.md').write_bytes(report.encode('utf-8'))
print(json.dumps({'memory_main_runs':sum(r['n'] for r in memory['memory']),
                  'startup_main_runs':sum(r['n'] for r in startup['startup']),
                  'final_geometry_mismatches':len(geometry_mismatches),'production':current},indent=2))
