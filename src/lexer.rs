use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Str(Vec<u8>),
    Char(i64),
    Punct(String),
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
}

#[derive(Debug)]
pub struct LexError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lexer hatası (satır {}): {}", self.line, self.msg)
    }
}

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.src.get(self.pos + n).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_whitespace() => {
                    self.bump();
                }
                Some(b'/') if self.peek_at(1) == Some(b'/') => {
                    while let Some(c) = self.bump() {
                        if c == b'\n' {
                            break;
                        }
                    }
                }
                Some(b'/') if self.peek_at(1) == Some(b'*') => {
                    self.bump();
                    self.bump();
                    while let Some(c) = self.bump() {
                        if c == b'*' && self.peek() == Some(b'/') {
                            self.bump();
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut out = Vec::new();
        loop {
            self.skip_ws_and_comments();
            let line = self.line;
            let c = match self.peek() {
                Some(c) => c,
                None => break,
            };

            if c == b'#' {
                while let Some(ch) = self.bump() {
                    if ch == b'\n' {
                        break;
                    }
                }
                continue;
            }

            if c.is_ascii_alphabetic() || c == b'_' {
                let start = self.pos;
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_alphanumeric() || ch == b'_' {
                        self.bump();
                    } else {
                        break;
                    }
                }
                let s = String::from_utf8_lossy(&self.src[start..self.pos]).to_string();
                out.push(Token {
                    tok: Tok::Ident(s),
                    line,
                });
                continue;
            }

            if c.is_ascii_digit() {
                let v = self.lex_number()?;
                out.push(Token {
                    tok: Tok::Int(v),
                    line,
                });
                continue;
            }

            if c == b'"' {
                self.bump();
                let bytes = self.lex_string()?;
                out.push(Token {
                    tok: Tok::Str(bytes),
                    line,
                });
                continue;
            }

            if c == b'\'' {
                self.bump();
                let v = self.lex_char()?;
                out.push(Token {
                    tok: Tok::Char(v),
                    line,
                });
                continue;
            }

            let p = self.lex_punct()?;
            out.push(Token {
                tok: Tok::Punct(p),
                line,
            });
        }
        out.push(Token {
            tok: Tok::Eof,
            line: self.line,
        });
        Ok(out)
    }

    fn skip_int_suffix(&mut self) {
        while let Some(c) = self.peek() {
            if matches!(c, b'u' | b'U' | b'l' | b'L') {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn lex_number(&mut self) -> Result<i64, LexError> {
        let start = self.pos;
        if self.peek() == Some(b'0')
            && matches!(self.peek_at(1), Some(b'x') | Some(b'X'))
        {
            self.bump();
            self.bump();
            let hstart = self.pos;
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() {
                    self.bump();
                } else {
                    break;
                }
            }
            let s = std::str::from_utf8(&self.src[hstart..self.pos]).unwrap_or("0");
            let v = i64::from_str_radix(s, 16).map_err(|_| LexError {
                line: self.line,
                msg: format!("geçersiz onaltılık sayı: {}", s),
            })?;
            self.skip_int_suffix();
            return Ok(v);
        }
        if self.peek() == Some(b'0')
            && matches!(self.peek_at(1), Some(b'b') | Some(b'B'))
        {
            self.bump();
            self.bump();
            let bstart = self.pos;
            while let Some(c) = self.peek() {
                if c == b'0' || c == b'1' {
                    self.bump();
                } else {
                    break;
                }
            }
            let s = std::str::from_utf8(&self.src[bstart..self.pos]).unwrap_or("0");
            let v = i64::from_str_radix(s, 2).map_err(|_| LexError {
                line: self.line,
                msg: format!("geçersiz ikilik sayı: {}", s),
            })?;
            self.skip_int_suffix();
            return Ok(v);
        }
        if self.peek() == Some(b'0')
            && matches!(self.peek_at(1), Some(b'0'..=b'7'))
        {
            self.bump();
            let ostart = self.pos;
            while let Some(c) = self.peek() {
                if matches!(c, b'0'..=b'7') {
                    self.bump();
                } else {
                    break;
                }
            }
            let s = std::str::from_utf8(&self.src[ostart..self.pos]).unwrap_or("0");
            let v = i64::from_str_radix(s, 8).map_err(|_| LexError {
                line: self.line,
                msg: format!("geçersiz sekizlik sayı: {}", s),
            })?;
            self.skip_int_suffix();
            return Ok(v);
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.bump();
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        let v = s.parse::<i64>().map_err(|_| LexError {
            line: self.line,
            msg: format!("geçersiz sayı: {}", s),
        })?;
        self.skip_int_suffix();
        Ok(v)
    }

    fn lex_escape(&mut self) -> Result<u8, LexError> {
        let c = self.bump().ok_or_else(|| LexError {
            line: self.line,
            msg: "kaçış dizisi eksik".into(),
        })?;
        Ok(match c {
            b'n' => b'\n',
            b't' => b'\t',
            b'r' => b'\r',
            b'0'..=b'7' => {
                let mut v = (c - b'0') as u16;
                let mut n = 1;
                while n < 3 {
                    match self.peek() {
                        Some(h) if matches!(h, b'0'..=b'7') => {
                            v = v * 8 + (h - b'0') as u16;
                            self.bump();
                            n += 1;
                        }
                        _ => break,
                    }
                }
                v as u8
            }
            b'a' => 7,
            b'b' => 8,
            b'f' => 12,
            b'v' => 11,
            b'\\' => b'\\',
            b'"' => b'"',
            b'\'' => b'\'',
            b'?' => b'?',
            b'x' => {
                let mut v = 0u8;
                let mut n = 0;
                while n < 2 {
                    match self.peek() {
                        Some(h) if h.is_ascii_hexdigit() => {
                            let d = (h as char).to_digit(16).unwrap() as u8;
                            v = v * 16 + d;
                            self.bump();
                            n += 1;
                        }
                        _ => break,
                    }
                }
                v
            }
            other => other,
        })
    }

    fn lex_string(&mut self) -> Result<Vec<u8>, LexError> {
        let mut out = Vec::new();
        loop {
            match self.bump() {
                None => {
                    return Err(LexError {
                        line: self.line,
                        msg: "kapatılmamış string".into(),
                    })
                }
                Some(b'"') => break,
                Some(b'\\') => out.push(self.lex_escape()?),
                Some(c) => out.push(c),
            }
        }
        Ok(out)
    }

    fn lex_char(&mut self) -> Result<i64, LexError> {
        let v = match self.bump() {
            None => {
                return Err(LexError {
                    line: self.line,
                    msg: "kapatılmamış karakter".into(),
                })
            }
            Some(b'\\') => self.lex_escape()? as i64,
            Some(c) => c as i64,
        };
        if self.peek() != Some(b'\'') {
            return Err(LexError {
                line: self.line,
                msg: "karakter literali tek karakter olmalı".into(),
            });
        }
        self.bump();
        Ok(v)
    }

    fn lex_punct(&mut self) -> Result<String, LexError> {
        const THREE: [&str; 5] = [">>=", "<<=", "...", "->*", "<=>"];
        const TWO: [&str; 21] = [
            "->", "++", "--", "&&", "||", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=",
            "%=", "&=", "|=", "^=", "<<", ">>", "::", "##",
        ];
        let rest = &self.src[self.pos..];
        for op in THREE {
            if rest.starts_with(op.as_bytes()) {
                for _ in 0..op.len() {
                    self.bump();
                }
                return Ok(op.to_string());
            }
        }
        for op in TWO {
            if rest.starts_with(op.as_bytes()) {
                for _ in 0..op.len() {
                    self.bump();
                }
                return Ok(op.to_string());
            }
        }
        let c = self.bump().unwrap();
        match c {
            b'{' | b'}' | b'(' | b')' | b'[' | b']' | b';' | b',' | b'.' | b'?' | b':'
            | b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'~' | b'!' | b'<'
            | b'>' | b'=' => Ok((c as char).to_string()),
            _ => Err(LexError {
                line: self.line,
                msg: format!("beklenmeyen karakter: '{}'", c as char),
            }),
        }
    }
}
