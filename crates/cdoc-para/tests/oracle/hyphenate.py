"""TeX itself as the hyphenation oracle for tests/golden_hyphen.rs.

INITEX loads one pattern file and \\showhyphens each word. 8-bit TeX reads bytes, so the
pattern file and the words are mapped character for character to bytes (ASCII stays, every
other character gets its own byte from 128 up, given a \\lccode of itself) and TeX's answer is
mapped back. The patterns are otherwise exactly the shipped file. Fonts: ecrm1000 has a glyph
at every byte used, so no letter is dropped from a word.

usage: python3 -I hyphenate.py PATTERN_FILE LEFTHYPHENMIN RIGHTHYPHENMIN WORDS_FILE
prints "word<TAB>hy-phen-ated" per word (needs tex on PATH; TeX Live 2024 was used)
"""
import os, re, shutil, subprocess, sys, tempfile

def main(pattern_file, lhm, rhm, words_file):
    here = os.path.dirname(os.path.abspath(__file__))
    source = open(pattern_file, encoding='utf-8').read()
    words = [w for w in open(words_file, encoding='utf-8').read().split('\n') if w]
    extra = sorted({c for c in source + ''.join(words) if ord(c) > 127})
    assert len(extra) <= 120, 'too many non-ASCII characters for one byte each'
    to_byte = {c: 128 + k for k, c in enumerate(extra)}
    from_byte = {v: k for k, v in to_byte.items()}
    enc = lambda s: bytes(to_byte.get(c, ord(c) if ord(c) < 128 else 0) for c in s)
    work = tempfile.mkdtemp()
    try:
        open(os.path.join(work, 'pat.tex'), 'wb').write(enc(source))
        lc = ''.join('\\lccode%d=%d ' % (b, b) for b in from_byte) + "\\lccode`\\'=`\\' "
        drv = ('\\catcode`\\{=1 \\catcode`\\}=2 \\catcode`\\#=6\n' + lc + '\n'
               '\\input pat.tex\n'
               '\\defaulthyphenchar=`\\- \\font\\tenrm=ecrm1000 \\tenrm\n'
               '\\lefthyphenmin=%s \\righthyphenmin=%s \\uchyph=1 \\language=0\n'
               '\\hbadness=0 \\showboxdepth=0 \\showboxbreadth=10000 \\tracingonline=1\n'
               '\\def\\sh#1{\\setbox0\\vbox{\\parfillskip=0pt \\hsize=16383pt \\pretolerance=-1'
               ' \\tolerance=-1 \\hbadness=0 \\hskip0pt #1}}\n') % (lhm, rhm)
        body = b''.join(b'\\sh{' + enc(w) + b'}\n' for w in words)
        open(os.path.join(work, 'drv.tex'), 'wb').write(drv.encode() + body + b'\\end\n')
        env = dict(os.environ, max_print_line='10000')
        subprocess.run(['tex', '-ini', '-interaction=nonstopmode', 'drv.tex'], cwd=work,
                       env=env, capture_output=True)
        log = open(os.path.join(work, 'drv.log'), 'rb').read().decode('latin-1')
        out = []
        for line in log.split('\n'):
            if line.startswith('[] \\tenrm '):
                shown = line[len('[] \\tenrm '):]
                shown = re.sub(r'\^\^([0-9a-f]{2})', lambda m: chr(int(m.group(1), 16)), shown)
                out.append(''.join(from_byte.get(ord(c), c) for c in shown))
        assert len(out) == len(words), (len(out), len(words))
        for w, h in zip(words, out):
            assert h.replace('-', '') == w, (w, h)
            print('%s\t%s' % (w, h))
    finally:
        shutil.rmtree(work)

if __name__ == '__main__':
    main(*sys.argv[1:])
