# Test 19 — Enhanced Markdown Features

ทดสอบฟีเจอร์ Markdown ขั้นสูง: syntax highlighting, containers, alerts, color boxes

---

## 1. Code Block — TypeScript

```typescript
import { PDFCreator, MarkdownParser } from 'lynpdf'

interface Config {
  pageSize: 'A4' | 'A3' | 'letter'
  margin: number
}

async function generateReport(config: Config) {
  const creator = new PDFCreator()
  const md = '# Report'
  const html = MarkdownParser.toHTML(md)
  return creator.createPDFBuffer(html, '')
}
```

---

## 2. Code Block — Python

```python
import pandas as pd
from typing import List, Dict

def analyze(records: List[Dict]) -> pd.DataFrame:
    """วิเคราะห์ข้อมูล"""
    df = pd.DataFrame(records)
    df['total'] = df['price'] * df['quantity']
    summary = df.groupby('category').agg({
        'total': ['sum', 'mean']
    })
    return summary.round(2)
```

  ### 2.1 Python indentation preservation probe

  ```python
  def run_pipeline(items):
    for item in items:
      if item.get("active"):
        print("    prepare")
        print("    execute")
      else:
        print("    skip")
  ```

---

## 3. Code Block — HTML

```html
<!DOCTYPE html>
<html lang="th">
<head>
  <style>
    .card {
      border-radius: 8px;
      padding: 16px;
      background: #ffffff;
    }
  </style>
</head>
<body>
  <div class="card">Hello</div>
</body>
</html>
```

---

## 4. Code Block — JSON

```json
{
  "name": "lynpdf",
  "version": "2.1.0",
  "features": ["html-to-pdf", "markdown"],
  "config": {
    "pageSize": "A4",
    "margin": 50
  }
}
```

---

## 5. Code Block — SQL

```sql
SELECT 
    c.name AS category,
    COUNT(p.id) AS product_count,
    AVG(p.price) AS avg_price
FROM categories c
JOIN products p ON c.id = p.category_id
WHERE p.active = TRUE
GROUP BY c.name
HAVING COUNT(p.id) > 5
ORDER BY avg_price DESC
LIMIT 10;
```

---

## 6. Code Block — Shell

```bash
#!/bin/bash
set -euo pipefail

echo "Building LynPDF..."
bun install
bun run build
bun test
echo "Done!"
```

---

## 7. Alert Containers

:::info
นี่คือข้อมูลสำคัญ — **Info container** ใช้แจ้งข้อมูลทั่วไป
:::

:::tip เคล็ดลับ
ใช้ `MarkdownParser.toHTMLAsync()` เมื่อต้องการ render Mermaid
:::

:::warning ระวัง!
อย่าลืมตั้งค่า `@page { margin }` เมื่อใช้ custom CSS
:::

:::danger อันตราย
ห้ามใช้ `eval()` กับ user input
:::

:::note หมายเหตุ
รองรับภาษาไทย ภาษาอังกฤษ และ emoji ได้ครบ 🇹🇭
:::

:::caution ข้อควรระวัง
ไฟล์ฟอนต์ OTF บางตัวอาจมี ligature ที่ต้องตรวจสอบ
:::

:::important
Container นี้ใช้สำหรับข้อมูลที่ **สำคัญมาก**
:::

---

## 8. GitHub-Style Alerts

> [!NOTE]
> GitHub-style alerts ใช้ syntax `> [!NOTE]` ภายใน blockquote

> [!TIP]
> ใช้ `--verbose` flag เพื่อดู progress log

> [!WARNING]
> Mermaid rendering ต้องติดตั้ง `@mermaid-js/mermaid-cli` แยก

> [!CAUTION]
> ระวังขนาดไฟล์ฟอนต์ — ฟอนต์ขนาดใหญ่อาจทำให้ PDF ใหญ่เกินไป

> [!IMPORTANT]
> ต้องใช้ Bun ≥ 1.0 เท่านั้น

---

## 9. Cards & Color Boxes

:::card LynPDF Features
- ✅ ภาษาไทย
- ✅ Color emoji (Twemoji)
- ✅ Flexbox layout (Yoga)
- ✅ Syntax highlighting
:::

:::box-blue Technical Info
ข้อมูลเชิงเทคนิค
:::

:::box-green Success
การดำเนินการสำเร็จ ✅
:::

:::box-red Error
เกิดข้อผิดพลาด — โปรดตรวจสอบ input
:::

:::box-yellow Warning
คำเตือน — ข้อมูลอาจไม่ครบถ้วน
:::

:::box-purple Special
ข้อมูลสำคัญพิเศษ
:::

:::box-gray Note
หมายเหตุ — ข้อมูลเพิ่มเติม
:::

:::box-orange In Progress
กำลังดำเนินการ
:::

---

## 10. สรุป

| Feature | Status | Plugin |
|---------|--------|--------|
| Syntax Highlighting | ✅ | highlight.js |
| Containers | ✅ | markdown-it-container |
| GitHub Alerts | ✅ | Custom core rule |
| Cards | ✅ | markdown-it-container |
| Color Boxes | ✅ | markdown-it-container |
| Footnotes | ✅ | markdown-it-footnote |
| Task Lists | ✅ | markdown-it-task-lists |

> LynPDF 2.2.0 — Enhanced Markdown Edition 🎉
