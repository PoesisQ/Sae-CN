"""Original CC0 fixtures for the on-demand Rust adapters."""
from pathlib import Path
root=Path(__file__).resolve().parents[1]/'tests/fixtures'
text='编码验证：中文段落与空行。\n\n    保留四个空格的缩进。\n'+'可搜索的原文 SEARCH-TEXT。\n'*3000
(root/'text-utf8.txt').write_bytes(text.encode('utf-8'))
(root/'text-utf16le.txt').write_bytes(b'\xff\xfe'+text.encode('utf-16le'))
(root/'text-gb18030.txt').write_bytes(text.encode('gb18030'))
(root/'reading.md').write_bytes('''# 编码验证 · Markdown 阅读

正文包含 **加粗**、*斜体* 和 [外部链接](https://example.org)。

## 列表、代码与表格

1. 第一项
2. 第二项
   - 嵌套列表

> 一段引用。文字与图片都应保留。

```rust
fn main() {
    println!("Reading text, images and tables stays native.");
}
```

| Feature | Expected |
| --- | --- |
| Table | TABLE-CELL |
| Image | Original illustration |

![原创绿色插图](illustration.png)

---

[回到标题](#编码验证-·-markdown-阅读)

<script>Raw HTML is visible text, never executable.</script>
'''.encode('utf-8'))

# Also keep the standalone fixture refresh reproducible.
import hashlib, json
manifest_path=root/'manifest.json'
manifest=json.loads(manifest_path.read_text(encoding='utf-8')) if manifest_path.exists() else {}
for name in ('text-utf8.txt','text-utf16le.txt','text-gb18030.txt','reading.md'):
    data=(root/name).read_bytes()
    manifest[name]={'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)}
manifest_path.write_bytes((json.dumps(dict(sorted(manifest.items())),indent=2)+'\n').encode('utf-8'))
