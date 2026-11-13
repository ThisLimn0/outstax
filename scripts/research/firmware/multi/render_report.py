#!/usr/bin/env python3
"""Render the consolidated INSTAX investigation Markdown as a polished PDF.
Usage: render_report.py [--source path] [--output path] [--qa path]
"""
from __future__ import annotations
import argparse, html, json, re, textwrap, sys
from pathlib import Path
from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.utils import ImageReader
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (BaseDocTemplate, PageTemplate, Frame, Paragraph,
    Spacer, PageBreak, LongTable, TableStyle, KeepTogether, Flowable, Image, CondPageBreak)
from reportlab.platypus.tableofcontents import TableOfContents

sys.dont_write_bytecode = True  # keep __pycache__ out of the project
sys.path.append(str(Path(__file__).resolve().parents[1]))
from _paths import MULTI_EXTERNAL, MULTI_OUT, multi_generated, out_dir
ROOT = MULTI_EXTERNAL
PAGE_W, PAGE_H = 595.276, 841.89
MARGIN = 46
WIDTH = PAGE_W - 2*MARGIN
NAVY = colors.HexColor('#132B45')
TEAL = colors.HexColor('#087F8C')
INK = colors.HexColor('#253B4C')
MUTED = colors.HexColor('#65798A')
PALE = colors.HexColor('#EDF4F7')
FONT_DIR = Path('/usr/share/fonts/truetype/dejavu')
for name, filename in [('DV','DejaVuSans.ttf'),('DV-Bold','DejaVuSans-Bold.ttf'),('DV-Italic','DejaVuSans-Oblique.ttf'),('DV-BoldItalic','DejaVuSans-BoldOblique.ttf'),('DVMono','DejaVuSansMono.ttf')]:
    pdfmetrics.registerFont(TTFont(name, str(FONT_DIR/filename)))
pdfmetrics.registerFontFamily('DV', normal='DV', bold='DV-Bold', italic='DV-Italic', boldItalic='DV-BoldItalic')

ST = {}
ST['body'] = ParagraphStyle('Body', fontName='DV', fontSize=9, leading=13.4, textColor=INK, spaceAfter=7, splitLongWords=True, allowWidows=0, allowOrphans=0)
ST['h1'] = ParagraphStyle('HeadingOne', parent=ST['body'], fontName='DV-Bold', fontSize=20, leading=25, textColor=NAVY, spaceBefore=17, spaceAfter=11, keepWithNext=True)
ST['h2'] = ParagraphStyle('HeadingTwo', parent=ST['body'], fontName='DV-Bold', fontSize=13.2, leading=18, textColor=TEAL, spaceBefore=14, spaceAfter=7, keepWithNext=True)
ST['h3'] = ParagraphStyle('HeadingThree', parent=ST['body'], fontName='DV-Bold', fontSize=10.2, leading=14.5, textColor=NAVY, spaceBefore=10, spaceAfter=5, keepWithNext=True)
ST['h4'] = ParagraphStyle('HeadingFour', parent=ST['h3'], fontSize=9.5, leading=13.5)
ST['small'] = ParagraphStyle('Small', parent=ST['body'], fontSize=7.6, leading=11)
ST['table'] = ParagraphStyle('TableText', parent=ST['body'], fontSize=7.4, leading=10.4, spaceAfter=0, allowWidows=1, allowOrphans=1)
ST['thead'] = ParagraphStyle('TableHead', parent=ST['table'], fontName='DV-Bold', textColor=colors.white)
ST['code'] = ParagraphStyle('Code', parent=ST['body'], fontName='DVMono', fontSize=7, leading=10.2, leftIndent=7, rightIndent=7, spaceBefore=5, spaceAfter=8, backColor=PALE, borderPadding=7)
ST['quote'] = ParagraphStyle('Quote', parent=ST['body'], leftIndent=11, rightIndent=8, textColor=MUTED, borderColor=TEAL, borderWidth=1, borderPadding=7)
ST['bullet'] = ParagraphStyle('Bullet', parent=ST['body'], leftIndent=12, firstLineIndent=-9)
ST['cover_title'] = ParagraphStyle('CoverTitle', parent=ST['h1'], fontSize=33, leading=40, spaceAfter=18)
ST['cover_subtitle'] = ParagraphStyle('CoverSubtitle', parent=ST['body'], fontSize=14, leading=21, textColor=MUTED, spaceAfter=22)


def clean(s):
    return s.replace('\u2011','-').replace('\u2013','-').replace('\u2014',' - ').replace('\u2212','-').replace('\u00a0',' ')


def inline(raw):
    """Escape untrusted Markdown first, then restore a compact safe inline subset."""
    raw = clean(raw.strip())
    slots = []
    def token(txt):
        slots.append(txt)
        return f'ZZINLINE{len(slots)-1}ZZ'
    raw = re.sub(r'`([^`]+)`', lambda m: token('<font name="DVMono" size="8">'+html.escape(m[1])+'</font>'), raw)
    raw = re.sub(r'\[([^\]]+)\]\((https?://[^\s)]+)\)', lambda m: token('<link href="'+html.escape(m[2], quote=True)+'" color="#087F8C">'+html.escape(m[1])+'</link>'), raw)
    raw = html.escape(raw)
    raw = re.sub(r'\*\*(.+?)\*\*', r'<b>\1</b>', raw)
    raw = re.sub(r'(?<!\*)\*([^*]+)\*(?!\*)', r'<i>\1</i>', raw)
    for i, val in enumerate(slots):
        raw = raw.replace(f'ZZINLINE{i}ZZ', val)
    # Paragraph's splitLongWords handles raw URLs without breaking XML entities.
    return raw


class Rule(Flowable):
    def __init__(self, width=WIDTH):
        Flowable.__init__(self); self.width=width; self.height=12
    def draw(self):
        self.canv.setStrokeColor(colors.HexColor('#C9D8E1')); self.canv.line(0,5,self.width,5)


class ReportDoc(BaseDocTemplate):
    def __init__(self, path, title):
        super().__init__(str(path), pagesize=(PAGE_W,PAGE_H), leftMargin=MARGIN, rightMargin=MARGIN, topMargin=49, bottomMargin=47, title=title, author='Technical investigation', pageCompression=1)
        self.report_title = title
        self.headings=[]
        self.addPageTemplates(PageTemplate(id='Report', frames=Frame(MARGIN,47,WIDTH,PAGE_H-96,id='body', leftPadding=0,rightPadding=0,topPadding=0,bottomPadding=0), onPage=self.page_decoration))
    def page_decoration(self, canvas, doc):
        canvas.saveState()
        if doc.page == 1:
            canvas.setFillColor(NAVY); canvas.rect(0,PAGE_H-16,PAGE_W,16,fill=1,stroke=0)
            canvas.setFillColor(TEAL); canvas.rect(0,PAGE_H-22,PAGE_W,6,fill=1,stroke=0)
        else:
            canvas.setFont('DV-Bold',7); canvas.setFillColor(MUTED)
            canvas.drawString(MARGIN,PAGE_H-28,'INSTAX MULTI-MODEL FIRMWARE INVESTIGATION')
            canvas.setStrokeColor(colors.HexColor('#C9D8E1')); canvas.line(MARGIN,PAGE_H-35,PAGE_W-MARGIN,PAGE_H-35)
        canvas.setFont('DV',7); canvas.setFillColor(MUTED)
        canvas.drawString(MARGIN,28,'Evidence, confidence and reproducible methods')
        canvas.drawRightString(PAGE_W-MARGIN,28,str(doc.page))
        canvas.restoreState()
    def afterFlowable(self, flowable):
        if isinstance(flowable, Paragraph) and getattr(flowable,'toc_level',None) is not None:
            text = flowable.getPlainText()
            key = flowable.bookmark_key
            self.canv.bookmarkPage(key)
            self.canv.addOutlineEntry(text,key,level=flowable.toc_level,closed=False)
            self.notify('TOCEntry',(flowable.toc_level,text,self.page,key))
            self.headings.append({'title':text,'page':self.page,'level':flowable.toc_level})


def split_table(line):
    line = line.strip().strip('|')
    return [x.strip().replace('\\|','|') for x in re.split(r'(?<!\\)\|',line)]


def table_block(lines):
    rows = [split_table(x) for x in lines]
    if len(rows)>1 and all(re.fullmatch(r':?-{2,}:?',x.strip()) for x in rows[1]):
        rows.pop(1)
    cols=max(map(len,rows))
    rows=[r+['']*(cols-len(r)) for r in rows]
    # Balanced sizing based on capped content length prevents a long URL from
    # starving columns containing ordinary prose.
    weights=[]
    for c in range(cols):
        lens=[min(len(re.sub(r'\[[^\]]+\]\([^)]*\)', 'link', r[c])),160) for r in rows]
        weights.append(max(7, min(30, sum(sorted(lens,reverse=True)[:8])/min(len(lens),8)**0.75)) ** .6)
    widths=[WIDTH*w/sum(weights) for w in weights]
    minimum=47 if cols>5 else 65
    for _ in range(cols):
        small=[i for i,w in enumerate(widths) if w<minimum]
        if not small: break
        excess=sum(minimum-widths[i] for i in small)
        large=[i for i in range(cols) if i not in small]
        if not large: break
        for i in small: widths[i]=minimum
        total=sum(widths[i] for i in large)
        for i in large: widths[i]-=excess*widths[i]/total
    data=[[Paragraph(inline(cell), ST['thead' if ri==0 else 'table']) for cell in row] for ri,row in enumerate(rows)]
    table=LongTable(data,colWidths=widths,repeatRows=1,hAlign='LEFT',splitByRow=1,splitInRow=0,spaceBefore=5,spaceAfter=11)
    table.setStyle(TableStyle([('BACKGROUND',(0,0),(-1,0),NAVY),('ROWBACKGROUNDS',(0,1),(-1,-1),[colors.white,PALE]),('VALIGN',(0,0),(-1,-1),'TOP'),('LEFTPADDING',(0,0),(-1,-1),6),('RIGHTPADDING',(0,0),(-1,-1),6),('TOPPADDING',(0,0),(-1,-1),6),('BOTTOMPADDING',(0,0),(-1,-1),6),('LINEBELOW',(0,0),(-1,0),.5,TEAL),('LINEBELOW',(0,-1),(-1,-1),.5,colors.HexColor('#C9D8E1'))]))
    return table


def parse_markdown(text, base_dir=ROOT):
    lines=clean(text).splitlines(); story=[]; i=0; heading_id=0
    # Title is represented on the designed cover, not repeated as a chapter.
    first_h1=True
    while i<len(lines):
        line=lines[i]; strip=line.strip()
        if not strip: i+=1; continue
        image_match=re.fullmatch(r'!\[([^\]]*)\]\(([^)]+)\)',strip)
        if image_match:
            image_path=Path(image_match[2]); image_path=image_path if image_path.is_absolute() else base_dir/image_path
            if image_path.is_file():
                iw,ih=ImageReader(str(image_path)).getSize(); scale=min(WIDTH/iw,470/ih,1)
                story.append(Image(str(image_path),width=iw*scale,height=ih*scale,hAlign='LEFT'))
                if image_match[1]: story.append(Paragraph(inline(image_match[1]),ST['small']))
            else:
                raise FileNotFoundError(f'Markdown figure is missing: {image_path}')
            i+=1; continue
        if strip.startswith('```'):
            code=[]; i+=1
            while i<len(lines) and not lines[i].strip().startswith('```'):
                code.append(lines[i]); i+=1
            i+=1
            for chunk in [code[x:x+42] for x in range(0,len(code),42)]:
                wrapped=[]
                for cl in chunk:
                    wrapped.extend(textwrap.wrap(cl,width=100,replace_whitespace=False,drop_whitespace=False,subsequent_indent='  ') or [' '])
                story.append(Paragraph('<br/>'.join(html.escape(x).replace(' ','&#160;') for x in wrapped),ST['code']))
            continue
        if re.match(r'^\s*\|',line) and i+1<len(lines) and re.search(r'\|\s*:?-{2,}',lines[i+1]):
            block=[]
            while i<len(lines) and lines[i].strip().startswith('|'):
                block.append(lines[i]); i+=1
            story.append(table_block(block)); continue
        match=re.match(r'^(#{1,6})\s+(.+?)\s*#*$',strip)
        if match:
            level=len(match[1]); title=match[2]
            if level==1 and first_h1:
                first_h1=False; i+=1
                while i<len(lines) and not re.match(r'^#{1,6}\s',lines[i].strip()): i+=1
                continue
            # Chapter-level headings begin cleanly after substantive preceding text.
            if level<=2 and story and len(story)>2:
                story.append(Spacer(1,5))
            if level==1 and story:
                story.append(CondPageBreak(235))
            effective=min(level,4)
            p=Paragraph(inline(title),ST[f'h{effective}'])
            if level<=2:
                p.toc_level=level-1
                p.bookmark_key=f'section-{heading_id}'; heading_id+=1
            story.append(p); i+=1; continue
        if re.fullmatch(r'[-*_]{3,}',strip): story.append(Rule()); i+=1; continue
        if strip.startswith('>'):
            quote=[]
            while i<len(lines) and lines[i].strip().startswith('>'):
                quote.append(re.sub(r'^\s*>\s?','',lines[i])); i+=1
            story.append(Paragraph(inline(' '.join(quote)),ST['quote'])); continue
        string_match=re.match(r'^- `(0x[0-9a-fA-F]+)`: `(.*)`$',strip)
        if string_match:
            catalog=[]
            while i<len(lines):
                entry=re.match(r'^- `(0x[0-9a-fA-F]+)`: `(.*)`$',lines[i].strip())
                if not entry: break
                catalog.append((entry[1],entry[2])); i+=1
            data=[[Paragraph(x,ST['thead']) for x in ['Address','Visible string','Address','Visible string']]]
            for ci in range(0,len(catalog),2):
                pair=catalog[ci:ci+2]
                if len(pair)==1: pair.append(('',''))
                data.append([Paragraph(inline('`'+x+'`') if x else '',ST['table']) for ent in pair for x in ent])
            tab=LongTable(data,colWidths=[76,WIDTH/2-76,76,WIDTH/2-76],repeatRows=1,hAlign='LEFT',splitByRow=1,splitInRow=0,spaceAfter=10)
            tab.setStyle(TableStyle([('BACKGROUND',(0,0),(-1,0),NAVY),('ROWBACKGROUNDS',(0,1),(-1,-1),[colors.white,PALE]),('VALIGN',(0,0),(-1,-1),'TOP'),('LEFTPADDING',(0,0),(-1,-1),4),('RIGHTPADDING',(0,0),(-1,-1),4),('TOPPADDING',(0,0),(-1,-1),3),('BOTTOMPADDING',(0,0),(-1,-1),3)]))
            story.append(tab); continue
        bullet=re.match(r'^\s*(?:[-*+]\s+|(\d+)[.)]\s+)(.*)',line)
        if bullet:
            label=(bullet[1]+'. ') if bullet[1] else '• '
            text=bullet[2]; i+=1
            while i<len(lines) and lines[i].startswith('  ') and lines[i].strip() and not re.match(r'^\s*[-*+]\s+',lines[i]):
                text+=' '+lines[i].strip(); i+=1
            story.append(Paragraph(inline(label+text),ST['bullet'])); continue
        para=[strip]; i+=1
        while i<len(lines) and lines[i].strip() and not re.match(r'^\s*(?:#{1,6}\s|```|\||>|[-*+]\s|\d+[.)]\s)',lines[i]) and not re.fullmatch(r'[-*_]{3,}',lines[i].strip()):
            para.append(lines[i].strip()); i+=1
        story.append(Paragraph(inline(' '.join(para)),ST['body']))
    return story


def build(source,output,qa):
    text=source.read_text(encoding='utf-8'); title=re.search(r'^#\s+(.+)',text,re.M)[1]
    out_dir(output.parent)
    doc=ReportDoc(output,title.replace('\n',' | '))
    story=[Spacer(1,51),Paragraph('TECHNICAL RESEARCH REPORT',ParagraphStyle('Eyebrow',parent=ST['small'],fontName='DV-Bold',textColor=TEAL,spaceAfter=18)),Paragraph(title.replace('\n','<br/>'),ST['cover_title']),Paragraph('Link WIDE 2 · SQUARE Link · mini Link 2 / 3 Renesas RX · WIDE Evo',ST['cover_subtitle']),Rule(),Spacer(1,20),Paragraph('A consolidated, source-traceable examination of firmware packaging, architecture evidence, image-processing paths and the limits of offline reverse engineering.',ST['body']),Spacer(1,23),Paragraph('Reading the evidence',ST['h2']),Paragraph('<b>Confirmed</b> - directly reproducible from the supplied bytes, official documentation or explicit public source evidence.<br/><b>Strong inference</b> - multiple independent observations support the interpretation.<br/><b>Working hypothesis</b> - plausible, with a clearly stated test still needed.<br/><b>Unresolved</b> - the available evidence does not settle the question.',ST['body']),Spacer(1,18),Paragraph('Confidence labels describe the strength of each claim. Shared formats or strings do not alone establish shared hardware, full algorithm recovery or cross-model compatibility.',ST['small']),Spacer(1,13),Paragraph('Prepared 12 November 2026',ST['small']),PageBreak(),Paragraph('Contents',ST['h1'])]
    toc=TableOfContents(); toc.levelStyles=[ParagraphStyle('TOC0',fontName='DV-Bold',fontSize=9.2,leading=13.2,leftIndent=0,firstLineIndent=0,spaceBefore=6,textColor=NAVY),ParagraphStyle('TOC1',fontName='DV',fontSize=8.1,leading=11.6,leftIndent=12,firstLineIndent=0,spaceBefore=3,textColor=MUTED)]; toc.dotsMinLevel=0
    story += [toc,PageBreak()]+parse_markdown(text, source.parent)
    doc.multiBuild(story)
    import pdfplumber
    with pdfplumber.open(output) as pdf:
        pages=[{'page':i+1,'characters':len(p.extract_text() or ''),'width':float(p.width),'height':float(p.height)} for i,p in enumerate(pdf.pages)]
    result={'source':str(source.relative_to(ROOT)) if source.is_relative_to(ROOT) else str(source),'output':str(output.relative_to(ROOT)) if output.is_relative_to(ROOT) else str(output),'pages':len(pages),'page_metrics':pages,'headings':doc.headings[-len(set(x['title'] for x in doc.headings)):],'visual_validation':'pending','renderer':'ReportLab / DejaVu / A4 / repeating wrapped tables','notes':['Inspect all-page contact sheets and full-size dense pages before delivery.']}
    out_dir(qa.parent); qa.write_text(json.dumps(result,indent=2))
    print(json.dumps({'output':str(output),'pages':len(pages),'qa':str(qa)}))

if __name__=='__main__':
    ap=argparse.ArgumentParser(); ap.add_argument('--source',type=Path); ap.add_argument('--output',type=Path,default=MULTI_OUT/'output/pdf/instax-multi-firmware-report.pdf'); ap.add_argument('--qa',type=Path,default=MULTI_OUT/'report/multi/pdf-qa.json'); args=ap.parse_args(); build(args.source or multi_generated('report/multi/REPORT.md'),args.output,args.qa)
