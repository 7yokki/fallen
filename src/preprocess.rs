use crate::lexer::{Lexer, Tok, Token};
use std::collections::HashMap;

#[derive(Clone)]
struct Macro {
    params: Option<Vec<String>>,
    body: Vec<Token>,
}

pub fn preprocess(src: &str) -> Result<Vec<Token>, String> {
    let mut macros: HashMap<String, Macro> = HashMap::new();
    let mut cleaned = String::with_capacity(src.len());

    for line in src.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            if let Some(rest) = trimmed.strip_prefix("#define") {
                if rest.starts_with(char::is_whitespace) {
                    parse_define(rest.trim_start(), &mut macros)?;
                }
            }
            cleaned.push('\n');
            continue;
        }
        cleaned.push_str(line);
    }

    let toks = Lexer::new(&cleaned)
        .tokenize()
        .map_err(|e| e.to_string())?;
    expand_tokens(&toks, &macros, 0)
}

fn parse_define(text: &str, macros: &mut HashMap<String, Macro>) -> Result<(), String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] as char).is_ascii_alphanumeric() || bytes.get(i) == Some(&b'_') {
        i += 1;
    }
    if i == 0 {
        return Ok(());
    }
    let name = text[..i].to_string();
    let mut params = None;
    if bytes.get(i) == Some(&b'(') {
        let close = text[i..]
            .find(')')
            .ok_or_else(|| format!("#define {}: parametre listesi kapatılmadı", name))?;
        let inside = &text[i + 1..i + close];
        let list: Vec<String> = if inside.trim().is_empty() {
            Vec::new()
        } else {
            inside
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        params = Some(list);
        i += close + 1;
    }
    let body_text = text[i..].trim();
    let body = if body_text.is_empty() {
        Vec::new()
    } else {
        Lexer::new(body_text)
            .tokenize()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|t| !matches!(t.tok, Tok::Eof))
            .collect()
    };
    macros.insert(name, Macro { params, body });
    Ok(())
}

fn expand_tokens(
    toks: &[Token],
    macros: &HashMap<String, Macro>,
    depth: usize,
) -> Result<Vec<Token>, String> {
    if depth > 64 {
        return Err("önişlemci: makro genişletme sınırı aşıldı (özyineleme?)".into());
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i].tok {
            Tok::Eof => {
                out.push(toks[i].clone());
                i += 1;
            }
            Tok::Ident(name) => {
                if let Some(m) = macros.get(name) {
                    if let Some(params) = &m.params {
                        let is_call = matches!(toks.get(i + 1).map(|t| &t.tok), Some(Tok::Punct(p)) if p == "(");
                        if is_call {
                            let mut j = i + 2;
                            let mut args: Vec<Vec<Token>> = Vec::new();
                            let mut cur: Vec<Token> = Vec::new();
                            let mut paren = 0i32;
                            let mut closed = false;
                            while j < toks.len() {
                                match &toks[j].tok {
                                    Tok::Eof => break,
                                    Tok::Punct(p) if p == "(" => {
                                        paren += 1;
                                        cur.push(toks[j].clone());
                                    }
                                    Tok::Punct(p) if p == ")" => {
                                        if paren == 0 {
                                            closed = true;
                                            break;
                                        }
                                        paren -= 1;
                                        cur.push(toks[j].clone());
                                    }
                                    Tok::Punct(p) if p == "," && paren == 0 => {
                                        args.push(std::mem::take(&mut cur));
                                    }
                                    _ => cur.push(toks[j].clone()),
                                }
                                j += 1;
                            }
                            if !closed {
                                return Err(format!("önişlemci: '{}' makro çağrısı kapatılmadı", name));
                            }
                            if !cur.is_empty() || !args.is_empty() {
                                args.push(cur);
                            }
                            if args.len() == 1 && args[0].is_empty() && params.is_empty() {
                                args.clear();
                            }
                            if args.len() != params.len() {
                                return Err(format!(
                                    "önişlemci: '{}' makrosu {} argüman bekliyor, {} verildi",
                                    name,
                                    params.len(),
                                    args.len()
                                ));
                            }
                            let mut substituted = Vec::new();
                            for bt in &m.body {
                                if let Tok::Ident(bn) = &bt.tok {
                                    if let Some(pos) = params.iter().position(|p| p == bn) {
                                        for t in &args[pos] {
                                            substituted.push(Token {
                                                tok: t.tok.clone(),
                                                line: toks[i].line,
                                            });
                                        }
                                        continue;
                                    }
                                }
                                substituted.push(Token {
                                    tok: bt.tok.clone(),
                                    line: toks[i].line,
                                });
                            }
                            let expanded = expand_tokens(&substituted, macros, depth + 1)?;
                            out.extend(expanded.into_iter().filter(|t| !matches!(t.tok, Tok::Eof)));
                            i = j + 1;
                        } else {
                            out.push(toks[i].clone());
                            i += 1;
                        }
                    } else {
                        let expanded = expand_tokens(&m.body, macros, depth + 1)?;
                        for mut t in expanded {
                            if !matches!(t.tok, Tok::Eof) {
                                t.line = toks[i].line;
                                out.push(t);
                            }
                        }
                        i += 1;
                    }
                } else {
                    out.push(toks[i].clone());
                    i += 1;
                }
            }
            _ => {
                out.push(toks[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}
