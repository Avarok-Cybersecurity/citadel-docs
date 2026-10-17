//! Reading a TeX pattern file: `\patterns{...}` and `\hyphenation{...}` groups, `%` comments,
//! `\message{...}` (ignored) and `\endinput`. Anything else is an error, not a guess.

use super::trie::Pattern;
use super::HyphenMins;
use crate::error::PatternError;

/// An exception: the word's letters and the letter counts before each hyphen.
pub(crate) type Exception = (Vec<char>, Vec<usize>);

/// The file's contents.
pub(crate) struct Parsed {
    /// Patterns with the line each came from.
    pub patterns: Vec<(Pattern, usize)>,
    pub exceptions: Vec<Exception>,
}

struct Scanner<'s> {
    chars: std::iter::Peekable<std::str::Chars<'s>>,
    line: usize,
}

impl Scanner<'_> {
    /// The next character outside comments.
    fn next(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        match c {
            '\n' => self.line += 1,
            '%' => {
                for c in self.chars.by_ref() {
                    if c == '\n' {
                        self.line += 1;
                        return Some('\n');
                    }
                }
                return Some('\n');
            }
            _ => {}
        }
        Some(c)
    }

    fn control_word(&mut self) -> String {
        let mut name = String::new();
        while let Some(&c) = self.chars.peek() {
            if !c.is_ascii_alphabetic() {
                break;
            }
            name.push(c);
            self.chars.next();
        }
        name
    }

    /// Whitespace-separated tokens up to the closing `}`, with their lines.
    fn group(&mut self, group: &'static str) -> Result<Vec<(String, usize)>, PatternError> {
        let mut tokens = Vec::new();
        let mut token = String::new();
        loop {
            let Some(c) = self.next() else {
                return Err(PatternError::Unclosed { group });
            };
            if c.is_whitespace() || c == '}' {
                if !token.is_empty() {
                    let line = self.line - usize::from(c == '\n');
                    tokens.push((std::mem::take(&mut token), line));
                }
                if c == '}' {
                    return Ok(tokens);
                }
            } else {
                token.push(c);
            }
        }
    }

    fn open(&mut self, group: &'static str) -> Result<(), PatternError> {
        loop {
            match self.next() {
                Some('{') => return Ok(()),
                Some(c) if c.is_whitespace() => {}
                _ => return Err(PatternError::Unclosed { group }),
            }
        }
    }
}

fn pattern(token: &str, line: usize) -> Result<Pattern, PatternError> {
    let bad = || PatternError::Malformed {
        line,
        pattern: token.to_string(),
    };
    let mut letters = Vec::new();
    let mut values = vec![0u8];
    let mut digit_here = false;
    for c in token.chars() {
        if let Some(d) = c.to_digit(10) {
            if digit_here {
                return Err(bad());
            }
            *values.last_mut().ok_or_else(bad)? = u8::try_from(d).map_err(|_| bad())?;
            digit_here = true;
        } else if c == '\\' || c == '{' || c == '}' || c == '-' {
            return Err(bad());
        } else {
            letters.push(c);
            values.push(0);
            digit_here = false;
        }
    }
    let inner_dot = letters.len() > 2 && letters[1..letters.len() - 1].contains(&'.');
    if letters.is_empty() || letters == ['.'] || inner_dot {
        return Err(bad());
    }
    Ok(Pattern { letters, values })
}

fn exception(token: &str, line: usize) -> Result<Exception, PatternError> {
    let mut letters = Vec::new();
    let mut hyphens = Vec::new();
    for c in token.chars() {
        if c == '-' {
            hyphens.push(letters.len());
        } else if c.is_ascii_digit() || c == '.' || c == '\\' || c == '{' {
            return Err(PatternError::Malformed {
                line,
                pattern: token.to_string(),
            });
        } else {
            letters.push(super::lower(c));
        }
    }
    Ok((letters, hyphens))
}

/// Parses a pattern file.
pub(crate) fn parse(source: &str) -> Result<Parsed, PatternError> {
    let mut s = Scanner {
        chars: source.chars().peekable(),
        line: 1,
    };
    let mut parsed = Parsed {
        patterns: Vec::new(),
        exceptions: Vec::new(),
    };
    let mut seen_patterns = false;
    while let Some(c) = s.next() {
        if c.is_whitespace() {
            continue;
        }
        if c != '\\' {
            return Err(PatternError::Malformed {
                line: s.line,
                pattern: c.to_string(),
            });
        }
        let name = s.control_word();
        match name.as_str() {
            "patterns" => {
                s.open("patterns")?;
                seen_patterns = true;
                for (token, line) in s.group("patterns")? {
                    parsed.patterns.push((pattern(&token, line)?, line));
                }
            }
            "hyphenation" => {
                s.open("hyphenation")?;
                for (token, line) in s.group("hyphenation")? {
                    parsed.exceptions.push(exception(&token, line)?);
                }
            }
            "message" => {
                s.open("message")?;
                let mut depth = 1;
                while depth > 0 {
                    match s.next() {
                        Some('{') => depth += 1,
                        Some('}') => depth -= 1,
                        Some(_) => {}
                        None => return Err(PatternError::Unclosed { group: "message" }),
                    }
                }
            }
            "endinput" => break,
            _ => return Err(PatternError::Unsupported { line: s.line, name }),
        }
    }
    if seen_patterns {
        Ok(parsed)
    } else {
        Err(PatternError::NoPatterns)
    }
}

/// The `hyphenmins: typesetting:` values from a hyph-utf8 header.
pub(crate) fn header_mins(source: &str) -> Result<HyphenMins, PatternError> {
    let mut lines = source
        .lines()
        .take_while(|l| l.starts_with('%'))
        .map(|l| l.trim_start_matches('%').trim());
    lines
        .find(|l| *l == "hyphenmins:")
        .ok_or(PatternError::NoHyphenMins)?;
    lines
        .find(|l| *l == "typesetting:")
        .ok_or(PatternError::NoHyphenMins)?;
    let mut value = |key: &str| -> Result<usize, PatternError> {
        let l = lines.next().ok_or(PatternError::NoHyphenMins)?;
        l.strip_prefix(key)
            .and_then(|v| v.trim().parse().ok())
            .ok_or(PatternError::NoHyphenMins)
    };
    let left = value("left:")?;
    let right = value("right:")?;
    HyphenMins::new(left, right)
}
