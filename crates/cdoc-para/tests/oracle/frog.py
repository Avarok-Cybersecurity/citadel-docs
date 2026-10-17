"""TeX itself as the line-breaking oracle for tests/golden_tex.rs.

The Frog King paragraph from Knuth and Plass, "Breaking Paragraphs into Lines" (1981), is
measured in cmr10 by plain TeX (each word fragment's \\wd, the font's interword glue and
hyphen), hyphenated by INITEX with patterns/hyph-en-us.tex (hyphenate.py), and turned into
items. The same items are written as a TeX paragraph of rules (width = the fragment's width,
depth = its ordinal), \\hskip glue and \\discretionary hyphens, and broken by TeX with plain
TeX's parameters plus each case's overrides. \\showbox gives the lines; \\tracingparagraphs the
passes. The result is written as Rust data (tests/golden/frog.rs).

usage: python3 -I frog.py   (needs tex on PATH; TeX Live 2024 was used)
"""
import subprocess, re, sys, os, tempfile, shutil
HERE = os.path.dirname(os.path.abspath(__file__))
TEX = shutil.which('tex')
WORK = tempfile.mkdtemp()
os.chdir(WORK)
wl = sorted({re.sub(r"[^a-z]", "", p.lower()) for w in open(os.path.join(HERE, 'frog.txt')).read().split() for p in w.split('-')})
open('words.txt', 'w').write('\n'.join(wl) + '\n')
tsv = subprocess.run([sys.executable, '-I', os.path.join(HERE, 'hyphenate.py'), os.path.join(HERE, '..', '..', 'patterns', 'hyph-en-us.tex'), '2', '3', 'words.txt'], capture_output=True, text=True, check=True).stdout
hy = dict(l.split('\t') for l in tsv.strip().split('\n'))
text = open(os.path.join(HERE, 'frog.txt')).read().split()
def key(w): return re.sub(r"[^a-z]", "", w.lower())
words = []  # list of list of (frag, join) join: '|' soft, '/' explicit, '' end
for w in text:
    parts = w.split('-') if '-' in w else [w]
    out = []
    for pi, part in enumerate(parts):
        h = hy.get(key(part), key(part))
        cuts = []; n = 0
        for ch in h:
            if ch == '-': cuts.append(n)
            else: n += 1
        frags = []; prev = 0
        letters = 0; idx = 0
        positions = []
        for i, ch in enumerate(part):
            if ch.isalpha():
                if letters in cuts and letters > 0: positions.append(i)
                letters += 1
        prev = 0
        for p in positions:
            frags.append(part[prev:p]); prev = p
        frags.append(part[prev:])
        if pi < len(parts) - 1: frags[-1] += '-'
        out.append(frags)
    enc = '/'.join('|'.join(f) for f in out)
    words.append(enc)
frags = sorted({f for w in words for f in re.split(r'[|/]', w)})
# measure
with open('meas.tex', 'w') as f:
    f.write('\\font\\tenrm=cmr10 \\tenrm\n')
    f.write('\\message{[SPACE=\\number\\fontdimen2\\tenrm,\\number\\fontdimen3\\tenrm,\\number\\fontdimen4\\tenrm]}\n')
    f.write('\\setbox0\\hbox{-}\\message{[HYPHEN=\\number\\wd0]}\n')
    for fr in frags:
        f.write('\\setbox0\\hbox{%s}\\message{[W:%s=\\number\\wd0]}\n' % (fr, fr))
    f.write('\\end\n')
subprocess.run([TEX, '-interaction=nonstopmode', 'meas.tex'], capture_output=True)
log = open('meas.log').read().replace('\n', '')
space = tuple(int(x) for x in re.search(r'\[SPACE=(\d+),(\d+),(\d+)\]', log).groups())
hyphen = int(re.search(r'\[HYPHEN=(\d+)\]', log).group(1))
width = dict((m.group(1), int(m.group(2))) for m in re.finditer(r'\[W:([^=\]]+)=(\d+)\]', log))
assert all(fr in width for fr in frags), [fr for fr in frags if fr not in width]
# items: ('box', w, id) ('glue',) ('soft',) ('ex',) ('pen', p) ('fil',)
items = []; boxid = 0
for i, w in enumerate(words):
    if i: items.append(('glue',))
    for tok in re.split(r'([|/])', w):
        if tok == '|': items.append(('soft',))
        elif tok == '/': items.append(('ex',))
        else:
            boxid += 1; items.append(('box', width[tok], boxid))
items += [('pen', 10000), ('fil',), ('pen', -10000)]
box_index = {it[2]: k for k, it in enumerate(items) if it[0] == 'box'}
def tex_items():
    out = []
    for it in items[:-3]:
        if it[0] == 'box': out.append('\\vrule width %dsp height0pt depth%dpt' % (it[1], it[2]))
        elif it[0] == 'glue': out.append('\\hskip%dsp plus%dsp minus%dsp' % space)
        elif it[0] == 'soft': out.append('\\discretionary{\\vrule width%dsp height0pt depth999pt}{}{}' % hyphen)
        elif it[0] == 'ex': out.append('\\discretionary{}{}{}')
    return '%\n'.join(out)
def run(name, shape, sets):
    with open('kp.tex', 'w') as f:
        f.write('\\showboxdepth=10 \\showboxbreadth=100000 \\tracingparagraphs=1 \\tracingonline=0\n')
        f.write('\\parindent=0pt \\parfillskip=0pt plus1fil \\hbadness=10000 \\hfuzz=0pt\n')
        if len(shape) == 1: f.write('\\setbox1\\vbox{\\hsize=%dsp %s\\noindent\n' % (shape[0], sets))
        else: f.write('\\setbox1\\vbox{\\parshape %d %s %s\\noindent\n' % (len(shape), ' '.join('0pt %dsp' % w for w in shape), sets))
        f.write(tex_items() + '\\par}\n\\showbox1\n\\end\n')
    subprocess.run([TEX, '-interaction=nonstopmode', 'kp.tex'], capture_output=True)
    log = open('kp.log').read()
    if 'Overfull' in log: print('OVERFULL', name, file=sys.stderr); return None
    passes = len(re.findall(r'^@(firstpass|secondpass|emergencypass)', log, re.M)) + (1 if 'pretolerance=-1' in sets else 0)
    lines = []; cur = None
    for l in log.split('\n'):
        m = re.match(r'^\.\\hbox\(.*?\)x[\d.]+(?:, glue set (- )?([\d.]+)(fil)?)?', l)
        if m:
            g = 0.0 if (m.group(2) is None or m.group(3)) else float(m.group(2)) * (-1 if m.group(1) else 1)
            cur = {'ratio': g, 'last': None, 'hy': False}; lines.append(cur); continue
        m = re.match(r'^\.\.\\rule\(0\.0\+([\d.]+)\)', l)
        if m and cur is not None:
            d = int(float(m.group(1)))
            if d == 999: cur['hy'] = True
            else: cur['last'] = d
    breaks = []
    for k, ln in enumerate(lines):
        if k == len(lines) - 1: breaks.append(len(items) - 1); continue
        j = box_index[ln['last']] + 1
        breaks.append(j)
        assert items[j][0] in ('glue', 'soft', 'ex'), (name, j, items[j])
        assert (items[j][0] == 'soft') == ln['hy'], (name, k)
    ratios = [round(ln['ratio'] * 100000) for ln in lines]
    return passes, breaks, ratios
pt = 65536
cases = [
  ('hsize_390pt', [390*pt], '', {}),
  ('hsize_500pt', [500*pt], '', {}),
  ('hsize_300pt', [300*pt], '', {}),
  ('hsize_250pt', [250*pt], '', {}),
  ('hsize_200pt', [200*pt], '', {}),
  ('parshape', [180*pt, 220*pt, 260*pt, 300*pt, 340*pt], '', {}),
  ('looser_300pt', [300*pt], '\\looseness=1', {'looseness': 1}),
  ('looser_390pt', [390*pt], '\\looseness=1', {'looseness': 1}),
  ('looser_250pt', [250*pt], '\\looseness=2', {'looseness': 2}),
  ('tighter_300pt', [300*pt], '\\looseness=-1', {'looseness': -1}),
  ('looser_tolerant_300pt', [300*pt], '\\looseness=1 \\tolerance=2000', {'looseness': 1, 'tolerance': 2000}),
  ('looser_tolerant_200pt', [200*pt], '\\looseness=2 \\tolerance=5000', {'looseness': 2, 'tolerance': 5000}),
  ('tighter_tolerant_250pt', [250*pt], '\\looseness=-1 \\tolerance=2000', {'looseness': -1, 'tolerance': 2000}),
  ('tighter_tolerant_500pt', [500*pt], '\\looseness=-1 \\tolerance=9999', {'looseness': -1, 'tolerance': 9999}),
  ('tighter_390pt', [390*pt], '\\looseness=-1', {'looseness': -1}),
  ('no_first_pass_390pt', [390*pt], '\\pretolerance=-1', {'pretolerance': -1}),
  ('tolerant_150pt', [150*pt], '\\tolerance=1000', {'tolerance': 1000}),
  ('tolerant_160pt', [160*pt], '\\tolerance=1000', {'tolerance': 1000}),
  ('tolerant_175pt', [175*pt], '\\tolerance=1000', {'tolerance': 1000}),
  ('tolerant_180pt', [180*pt], '\\tolerance=500', {'tolerance': 500}),
  ('emergency_130pt', [130*pt], '\\emergencystretch=20pt', {'emergency_stretch': 20*pt}),
  ('emergency_150pt', [150*pt], '\\emergencystretch=10pt', {'emergency_stretch': 10*pt}),
  ('emergency_160pt', [160*pt], '\\emergencystretch=30pt', {'emergency_stretch': 30*pt}),
  ('emergency_175pt', [175*pt], '\\emergencystretch=15pt', {'emergency_stretch': 15*pt}),
]
res = []
for name, shape, sets, over in cases:
    got = run(name, shape, sets)
    if got is None: continue
    p, b, r = got
    res.append((name, shape, over, p, b, r))
    print(name, 'passes', p, 'lines', len(b), file=sys.stderr)
# emit rust
o = []
o.append('//! Generated by tests/oracle/frog.py from TeX 3.141592653 (TeX Live 2024). Do not edit.\n')
o.append('#![allow(clippy::unreadable_literal)] // scaled points, as TeX prints them\n')
o.append('/// cmr10 interword glue in sp: width, stretch, shrink (`\\fontdimen2..4`).')
o.append('pub const SPACE: (i32, i32, i32) = (%d, %d, %d);' % space)
o.append('/// Width of cmr10\'s hyphen in sp.')
o.append('pub const HYPHEN: i32 = %d;' % hyphen)
o.append('/// The Frog King paragraph: `|` a discretionary hyphen (TeX `\\discretionary`), `/` an explicit one.')
o.append('#[rustfmt::skip]')
o.append('pub const WORDS: &[(&str, &[i32])] = &[')
for w in words:
    ws = [width[t] for t in re.split(r'[|/]', w)]
    o.append('    ("%s", &[%s]),' % (w.replace('\\', '\\\\'), ', '.join(map(str, ws))))
o.append('];')
o.append('/// One TeX run: line widths, parameter overrides, and what TeX did.')
o.append('pub struct Case {')
for fld in ['pub name: &\'static str,', 'pub widths: &\'static [i32],', 'pub looseness: i32,', 'pub pretolerance: i32,', 'pub tolerance: i32,', 'pub emergency_stretch: i32,', 'pub passes: u8,', 'pub breaks: &\'static [usize],', 'pub ratios_e5: &\'static [i64],']:
    o.append('    ' + fld)
o.append('}')
o.append('/// The cases.')
o.append('#[rustfmt::skip]')
o.append('pub const CASES: &[Case] = &[')
for name, shape, over, p, b, r in res:
    o.append('    Case { name: "%s", widths: &[%s], looseness: %d, pretolerance: %d, tolerance: %d, emergency_stretch: %d, passes: %d, breaks: &[%s], ratios_e5: &[%s] },' % (
        name, ', '.join(map(str, shape)), over.get('looseness', 0), over.get('pretolerance', 100), over.get('tolerance', 200), over.get('emergency_stretch', 0), p, ', '.join(map(str, b)), ', '.join(map(str, r))))
o.append('];')
open(os.path.join(HERE, '..', 'golden', 'frog.rs'), 'w').write('\n'.join(o) + '\n')
shutil.rmtree(WORK)
