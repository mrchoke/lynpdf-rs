# LynPDF Markdown Test

ทดสอบการแปลง Markdown → PDF ด้วย LynPDF

---

## หัวข้อระดับ 2

### หัวข้อระดับ 3

#### หัวข้อระดับ 4

## ข้อความพื้นฐาน

ข้อความ **ตัวหนา** และ *ตัวเอียง* และ ~~ขีดฆ่า~~ พร้อม `inline code` ในประโยคเดียวกัน

[ลิงก์ไปยัง GitHub](https://github.com)

## รายการ (Lists)

### Unordered List

- รายการที่ 1
- รายการที่ 2
  - รายการย่อย A
  - รายการย่อย B
- รายการที่ 3

### Ordered List

1. ขั้นตอนที่ 1: ติดตั้ง Bun
2. ขั้นตอนที่ 2: สร้างโปรเจกต์
3. ขั้นตอนที่ 3: เขียนโค้ด

### Task List

- [x] ติดตั้ง dependencies
- [x] เขียน MarkdownParser
- [ ] เพิ่ม math support
- [ ] เขียนเอกสาร

## ตาราง (Table)

| ฟีเจอร์ | สถานะ | หมายเหตุ |
|---------|--------|----------|
| Headings | ✓ | h1–h4 |
| Bold / Italic | ✓ | **strong**, *em* |
| Tables | ✓ | GFM tables |
| Task Lists | ✓ | checkbox |
| Footnotes | ✓ | markdown-it-footnote |
| Code Blocks | ✓ | fenced + inline |
| Thai Text | ✓ | ภาษาไทยครบ |

## Code Block

```typescript
import { PDFCreator } from 'lynpdf'

const creator = new PDFCreator()
const md = '# Hello from Markdown!'

await creator.createPDFFromMarkdown(md, '', 'output.pdf')
console.log('Done!')
```

## Blockquote

> "ถ้าเราจะเพิ่ม Markdown to PDF โดยใช้ pipeline เดียวกัน ก็ทำได้เลย"
>
> — LynPDF Design Philosophy

## Footnotes

LynPDF ใช้ markdown-it[^1] สำหรับแปลง Markdown เป็น HTML จากนั้นใช้ pipeline เดิม[^2] ในการสร้าง PDF

[^1]: markdown-it เป็น Markdown parser ที่รองรับ plugins มากมาย รวมถึง footnotes, task lists, และ GFM tables
[^2]: Pipeline: HTML → parse5 → Yoga Layout → PDFKit → PDF

## ข้อความภาษาไทยยาว

ประเทศไทยเป็นประเทศที่มีความหลากหลายทางวัฒนธรรมและธรรมชาติ ตั้งแต่ภูเขาสูงทางภาคเหนือ ไปจนถึงชายหาดที่สวยงามทางภาคใต้ อาหารไทยเป็นที่รู้จักไปทั่วโลกด้วยรสชาติที่เป็นเอกลักษณ์ ผสมผสานระหว่างรสเปรี้ยว หวาน เค็ม และเผ็ด วัฒนธรรมไทยมีความลึกซึ้งและงดงาม สถาปัตยกรรมไทยมีเอกลักษณ์เฉพาะตัวที่โดดเด่น

---

*สร้างด้วย LynPDF — Markdown → PDF* 🐱
