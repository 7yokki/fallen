use crate::ast::*;
use crate::lexer::{Tok, Token};
use crate::types::Type;
use std::collections::HashSet;

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
    classes: HashSet<String>,
}

type PResult<T> = Result<T, String>;

impl Parser {
    pub fn new(toks: Vec<Token>) -> Self {
        Parser {
            toks,
            pos: 0,
            classes: HashSet::new(),
        }
    }

    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn peek_at(&self, n: usize) -> &Tok {
        if self.pos + n < self.toks.len() {
            &self.toks[self.pos + n].tok
        } else {
            &Tok::Eof
        }
    }

    fn line(&self) -> usize {
        self.toks[self.pos].line
    }

    fn advance(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn is_punct(&self, p: &str) -> bool {
        matches!(self.peek(), Tok::Punct(s) if s == p)
    }

    fn is_punct_at(&self, n: usize, p: &str) -> bool {
        matches!(self.peek_at(n), Tok::Punct(s) if s == p)
    }

    fn eat_punct(&mut self, p: &str) -> bool {
        if self.is_punct(p) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_punct(&mut self, p: &str) -> PResult<()> {
        if self.eat_punct(p) {
            Ok(())
        } else {
            Err(format!(
                "satır {}: '{}' bekleniyordu, bulunan: {:?}",
                self.line(),
                p,
                self.peek()
            ))
        }
    }

    fn expect_ident(&mut self) -> PResult<String> {
        match self.peek().clone() {
            Tok::Ident(s) => {
                self.advance();
                Ok(s)
            }
            other => Err(format!(
                "satır {}: tanımlayıcı bekleniyordu, bulunan: {:?}",
                self.line(),
                other
            )),
        }
    }

    fn ident_is(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Ident(x) if x == s)
    }

    fn is_qualifier(s: &str) -> bool {
        matches!(
            s,
            "const" | "static" | "volatile" | "register" | "inline" | "extern" | "auto"
        )
    }

    fn builtin_type(s: &str) -> Option<Type> {
        Some(match s {
            "void" => Type::Void,
            "char" => Type::Char,
            "short" => Type::Short,
            "int" => Type::Int,
            "long" => Type::Long,
            "float" | "double" => Type::Int,
            "_Bool" | "bool" => Type::UChar,
            "pid_t" => Type::Int,
            "size_t" => Type::ULong,
            "ssize_t" => Type::Long,
            "int8_t" => Type::SChar,
            "uint8_t" => Type::UChar,
            "int16_t" => Type::Short,
            "uint16_t" => Type::UShort,
            "int32_t" => Type::Int,
            "uint32_t" => Type::UInt,
            "int64_t" => Type::Long,
            "uint64_t" => Type::ULong,
            "FILE" => Type::Void,
            _ => return None,
        })
    }

    fn is_type_word(s: &str) -> bool {
        matches!(
            s,
            "void"
                | "char"
                | "short"
                | "int"
                | "long"
                | "float"
                | "double"
                | "_Bool"
                | "bool"
                | "signed"
                | "unsigned"
                | "struct"
                | "enum"
                | "union"
        ) || Self::builtin_type(s).is_some()
    }

    fn is_type_start(&self) -> bool {
        match self.peek() {
            Tok::Ident(s) => {
                if Self::is_qualifier(s) {
                    return true;
                }
                if Self::is_type_word(s) {
                    return true;
                }
                self.classes.contains(s)
            }
            _ => false,
        }
    }

    fn looks_like_decl(&self) -> bool {
        match self.peek() {
            Tok::Ident(s) => {
                if Self::is_qualifier(s) {
                    return true;
                }
                if Self::is_type_word(s) {
                    return true;
                }
                if self.classes.contains(s) {
                    return matches!(self.peek_at(1), Tok::Ident(_))
                        || matches!(self.peek_at(1), Tok::Punct(p) if p == "*" || p == "&");
                }
                false
            }
            _ => false,
        }
    }

    fn skip_qualifiers(&mut self) {
        while let Tok::Ident(s) = self.peek() {
            if Self::is_qualifier(s) {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn parse_base_type(&mut self) -> PResult<Type> {
        self.skip_qualifiers();

        let mut is_unsigned = false;
        let mut is_signed = false;
        if self.ident_is("unsigned") {
            is_unsigned = true;
            self.advance();
        } else if self.ident_is("signed") {
            is_signed = true;
            self.advance();
        }
        if self.ident_is("struct") || self.ident_is("enum") || self.ident_is("union") {
            self.advance();
            let name = self.expect_ident()?;
            let _ = self.parse_base_type()?;
            return Ok(Type::Class(name));
        }

        let base = if self.ident_is("char") {
            self.advance();
            Type::Char
        } else if self.ident_is("short") {
            self.advance();
            if self.ident_is("int") {
                self.advance();
            }
            Type::Short
        } else if self.ident_is("int") {
            self.advance();
            Type::Int
        } else if self.ident_is("long") {
            self.advance();
            if self.ident_is("long") {
                self.advance();
            }
            if self.ident_is("int") {
                self.advance();
            }
            Type::Long
        } else if self.ident_is("void") {
            self.advance();
            Type::Void
        } else if self.ident_is("float") || self.ident_is("double") {
            self.advance();
            Type::Int
        } else if self.ident_is("_Bool") || self.ident_is("bool") {
            self.advance();
            Type::UChar
        } else if is_unsigned || is_signed {
            Type::Int
        } else {
            let name = self.expect_ident()?;
            if let Some(t) = Self::builtin_type(&name) {
                t
            } else if self.classes.contains(&name) {
                Type::Class(name)
            } else {
                return Err(format!(
                    "satır {}: bilinmeyen tip '{}'",
                    self.line(),
                    name
                ));
            }
        };

        let base = if is_unsigned {
            match base {
                Type::Char => Type::UChar,
                Type::Short => Type::UShort,
                Type::Int => Type::UInt,
                Type::Long => Type::ULong,
                other => other,
            }
        } else if is_signed {
            match base {
                Type::Char => Type::SChar,
                Type::Short => Type::Short,
                Type::Int => Type::Int,
                Type::Long => Type::Long,
                other => other,
            }
        } else {
            base
        };
        Ok(base)
    }

    fn parse_pointer(&mut self, mut ty: Type) -> Type {
        loop {
            self.skip_qualifiers();
            if self.is_punct("*") {
                self.advance();
                ty = Type::Ptr(Box::new(ty));
            } else {
                break;
            }
        }
        ty
    }

    fn parse_array_suffix(&mut self, ty: Type) -> PResult<Type> {
        let mut dims: Vec<usize> = Vec::new();
        loop {
            if self.is_punct("[") {
                self.advance();
                let n = if self.is_punct("]") {
                    0
                } else {
                    if self.is_punct("-") {
                        return Err(format!(
                            "satır {}: dizi boyutu negatif olamaz",
                            self.line()
                        ));
                    }
                    match self.advance() {
                        Tok::Int(v) if v >= 0 => v as usize,
                        other => {
                            return Err(format!(
                                "satır {}: dizi boyutu sabit olmalı, bulunan: {:?}",
                                self.line(),
                                other
                            ))
                        }
                    }
                };
                self.expect_punct("]")?;
                dims.push(n);
            } else {
                break;
            }
        }

        let mut ty = ty;
        for n in dims.iter().rev() {
            if *n == 0 {
                ty = Type::Ptr(Box::new(ty));
            } else {
                ty = Type::Array(Box::new(ty), *n);
            }
        }
        Ok(ty)
    }

    fn decay_param_type(ty: Type) -> Type {
        match ty {
            Type::Array(elem, _) => Type::Ptr(elem),
            other => other,
        }
    }

    fn parse_type_name(&mut self) -> PResult<Type> {
        let base = self.parse_base_type()?;
        let ty = self.parse_pointer(base);
        Ok(ty)
    }

    fn parse_declarator(&mut self, base: Type) -> PResult<(String, Type)> {
        let ty = self.parse_pointer(base);
        let name = self.expect_ident()?;
        let ty = self.parse_array_suffix(ty)?;
        Ok((name, ty))
    }

    pub fn parse_program(&mut self) -> PResult<Program> {
        let mut items = Vec::new();
        while !matches!(self.peek(), Tok::Eof) {
            if self.is_punct(";") {
                self.advance();
                continue;
            }
            if self.ident_is("class") {
                items.push(Item::Class(self.parse_class()?));
                continue;
            }
            self.parse_global(&mut items)?;
        }
        Ok(Program { items })
    }

    fn parse_class(&mut self) -> PResult<ClassDecl> {
        let line = self.line();
        self.advance();
        let name = self.expect_ident()?;
        self.classes.insert(name.clone());
        self.expect_punct("{")?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut is_private = true;
        while !self.is_punct("}") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(format!("satır {}: sınıf gövdesi kapatılmadı", line));
            }
            if self.is_punct(";") {
                self.advance();
                continue;
            }
            if (self.ident_is("private")
                || self.ident_is("public")
                || self.ident_is("protected"))
                && self.is_punct_at(1, ":")
            {
                is_private = self.ident_is("private") || self.ident_is("protected");
                self.advance();
                self.advance();
                continue;
            }
            if self.is_punct("~") {
                return Err(format!(
                    "satır {}: yıkıcı metotlar (destructor) henüz desteklenmiyor",
                    self.line()
                ));
            }
            if self.ident_is(&name) && self.is_punct_at(1, "(") {
                self.advance();
                let params = self.parse_params()?;
                let body = self.parse_block()?;
                methods.push(FuncDecl {
                    name: name.clone(),
                    ret: Type::Void,
                    params,
                    body,
                    owner: Some(name.clone()),
                    is_ctor: true,
                    is_method: true,
                    is_private,
                    is_prototype: false,
                    frame_size: 0,
                    this_offset: None,
                    line,
                });
                continue;
            }
            let base = self.parse_base_type()?;
            let mut ty = self.parse_pointer(base.clone());
            let mname = self.expect_ident()?;
            ty = self.parse_array_suffix(ty)?;
            if self.is_punct("(") {
                let params = self.parse_params()?;
                let body = self.parse_block()?;
                methods.push(FuncDecl {
                    name: mname,
                    ret: ty,
                    params,
                    body,
                    owner: Some(name.clone()),
                    is_ctor: false,
                    is_method: true,
                    is_private,
                    is_prototype: false,
                    frame_size: 0,
                    this_offset: None,
                    line,
                });
            } else {
                fields.push(FieldDecl {
                    name: mname,
                    ty,
                    is_private,
                    line,
                });
                while self.eat_punct(",") {
                    let (fname, fty) = self.parse_declarator(base.clone())?;
                    fields.push(FieldDecl {
                        name: fname,
                        ty: fty,
                        is_private,
                        line,
                    });
                }
                self.expect_punct(";")?;
            }
        }
        self.expect_punct("}")?;
        self.eat_punct(";");
        Ok(ClassDecl {
            name,
            fields,
            methods,
            line,
        })
    }

    fn parse_params(&mut self) -> PResult<Vec<VarDecl>> {
        self.expect_punct("(")?;
        let mut params = Vec::new();
        if self.eat_punct(")") {
            return Ok(params);
        }
        if self.ident_is("void") && self.is_punct_at(1, ")") {
            self.advance();
            self.advance();
            return Ok(params);
        }
        let mut idx = 0;
        loop {
            let line = self.line();
            let base = self.parse_base_type()?;
            let mut ty = self.parse_pointer(base);
            let name = if let Tok::Ident(s) = self.peek().clone() {
                if !Self::is_type_word(&s) && !self.classes.contains(&s) {
                    self.advance();
                    s
                } else {
                    format!("__arg{}", idx)
                }
            } else {
                format!("__arg{}", idx)
            };
            ty = self.parse_array_suffix(ty)?;
            let ty = Self::decay_param_type(ty);
            params.push(VarDecl {
                name,
                ty,
                init: None,
                ctor_args: Vec::new(),
                offset: None,
                line,
            });
            idx += 1;
            if self.eat_punct(",") {
                continue;
            }
            self.expect_punct(")")?;
            break;
        }
        Ok(params)
    }

    fn parse_global(&mut self, items: &mut Vec<Item>) -> PResult<()> {
        let line = self.line();
        let base = self.parse_base_type()?;
        let (name, ty) = self.parse_declarator(base.clone())?;
        if self.is_punct("(") {
            let params = self.parse_params()?;
            if self.eat_punct(";") {
                items.push(Item::Func(FuncDecl {
                    name,
                    ret: ty,
                    params,
                    body: Stmt::Empty,
                    owner: None,
                    is_ctor: false,
                    is_method: false,
                    is_private: false,
                    is_prototype: true,
                    frame_size: 0,
                    this_offset: None,
                    line,
                }));
                return Ok(());
            }
            let body = self.parse_block()?;
            items.push(Item::Func(FuncDecl {
                name,
                ret: ty,
                params,
                body,
                owner: None,
                is_ctor: false,
                is_method: false,
                is_private: false,
                is_prototype: false,
                frame_size: 0,
                this_offset: None,
                line,
            }));
            return Ok(());
        }
        let mut name = name;
        let mut ty = ty;
        loop {
            let init = if self.eat_punct("=") {
                Some(self.parse_assign()?)
            } else {
                None
            };
            items.push(Item::Global(VarDecl {
                name: name.clone(),
                ty: ty.clone(),
                init,
                ctor_args: Vec::new(),
                offset: None,
                line,
            }));
            if !self.eat_punct(",") {
                break;
            }
            let (n, t) = self.parse_declarator(base.clone())?;
            name = n;
            ty = t;
        }
        self.expect_punct(";")?;
        Ok(())
    }

    fn parse_block(&mut self) -> PResult<Stmt> {
        self.expect_punct("{")?;
        let mut stmts = Vec::new();
        while !self.is_punct("}") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(format!("satır {}: blok kapatılmadı", self.line()));
            }
            stmts.push(self.parse_stmt()?);
        }
        self.expect_punct("}")?;
        Ok(Stmt::Block(stmts))
    }

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        if self.is_punct("{") {
            return self.parse_block();
        }
        if self.eat_punct(";") {
            return Ok(Stmt::Empty);
        }
        if self.ident_is("if") {
            self.advance();
            self.expect_punct("(")?;
            let cond = self.parse_expr()?;
            self.expect_punct(")")?;
            let then = Box::new(self.parse_stmt()?);
            let els = if self.ident_is("else") {
                self.advance();
                Some(Box::new(self.parse_stmt()?))
            } else {
                None
            };
            return Ok(Stmt::If(cond, then, els));
        }
        if self.ident_is("while") {
            self.advance();
            self.expect_punct("(")?;
            let cond = self.parse_expr()?;
            self.expect_punct(")")?;
            let body = Box::new(self.parse_stmt()?);
            return Ok(Stmt::While(cond, body));
        }
        if self.ident_is("do") {
            self.advance();
            let body = Box::new(self.parse_stmt()?);
            if !self.ident_is("while") {
                return Err(format!("satır {}: 'while' bekleniyordu", self.line()));
            }
            self.advance();
            self.expect_punct("(")?;
            let cond = self.parse_expr()?;
            self.expect_punct(")")?;
            self.expect_punct(";")?;
            return Ok(Stmt::DoWhile(body, cond));
        }
        if self.ident_is("for") {
            self.advance();
            self.expect_punct("(")?;
            let init = if self.eat_punct(";") {
                None
            } else if self.looks_like_decl() {
                Some(Box::new(self.parse_local_decl()?))
            } else {
                let e = self.parse_expr()?;
                self.expect_punct(";")?;
                Some(Box::new(Stmt::Expr(e)))
            };
            let cond = if self.is_punct(";") {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.expect_punct(";")?;
            let inc = if self.is_punct(")") {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.expect_punct(")")?;
            let body = Box::new(self.parse_stmt()?);
            return Ok(Stmt::For(init, cond, inc, body));
        }
        if self.ident_is("return") {
            self.advance();
            if self.eat_punct(";") {
                return Ok(Stmt::Return(None));
            }
            let e = self.parse_expr()?;
            self.expect_punct(";")?;
            return Ok(Stmt::Return(Some(e)));
        }
        if self.ident_is("switch") {
            return self.parse_switch();
        }
        if self.ident_is("break") {
            let line = self.line();
            self.advance();
            self.expect_punct(";")?;
            return Ok(Stmt::Break(line));
        }
        if self.ident_is("continue") {
            let line = self.line();
            self.advance();
            self.expect_punct(";")?;
            return Ok(Stmt::Continue(line));
        }
        if self.looks_like_decl() {
            return self.parse_local_decl();
        }
        let e = self.parse_expr()?;
        self.expect_punct(";")?;
        Ok(Stmt::Expr(e))
    }

    fn parse_switch(&mut self) -> PResult<Stmt> {
        self.advance(); // consume 'switch'
        self.expect_punct("(")?;
        let expr = self.parse_expr()?;
        self.expect_punct(")")?;
        self.expect_punct("{")?;
        let mut cases: Vec<SwitchCase> = Vec::new();
        while !self.is_punct("}") {
            if matches!(self.peek(), Tok::Eof) {
                return Err(format!("satır {}: switch gövdesi kapatılmadı", self.line()));
            }
            // Parse case or default label
            let value: Option<i64>;
            if self.ident_is("case") {
                self.advance();
                // Parse the case value (may be negative)
                let neg = self.eat_punct("-");
                let v = match self.advance() {
                    Tok::Int(n) => n,
                    Tok::Char(n) => n,
                    other => {
                        return Err(format!(
                            "satır {}: case değeri tam sayı olmalı, bulunan: {:?}",
                            self.line(),
                            other
                        ))
                    }
                };
                let v = if neg { -v } else { v };
                value = Some(v);
                self.expect_punct(":")?;
            } else if self.ident_is("default") {
                self.advance();
                self.expect_punct(":")?;
                value = None;
            } else {
                return Err(format!(
                    "satır {}: switch içinde 'case' veya 'default' bekleniyordu",
                    self.line()
                ));
            }
            // Collect stmts until next case/default/}
            let mut body: Vec<Stmt> = Vec::new();
            while !self.is_punct("}")
                && !self.ident_is("case")
                && !self.ident_is("default")
            {
                if matches!(self.peek(), Tok::Eof) {
                    return Err(format!("satır {}: switch gövdesi kapatılmadı", self.line()));
                }
                body.push(self.parse_stmt()?);
            }
            cases.push(SwitchCase { value, body });
        }
        self.expect_punct("}")?;
        Ok(Stmt::Switch(expr, cases))
    }

    fn parse_local_decl(&mut self) -> PResult<Stmt> {
        let line = self.line();
        let base = self.parse_base_type()?;
        let mut decls = Vec::new();
        loop {
            let (name, ty) = self.parse_declarator(base.clone())?;
            let init = if self.eat_punct("=") {
                Some(self.parse_assign()?)
            } else {
                None
            };
            let ctor_args = self.parse_ctor_args(&ty)?;
            decls.push(Stmt::Var(VarDecl {
                name,
                ty,
                init,
                ctor_args,
                offset: None,
                line,
            }));
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(";")?;
        if decls.len() == 1 {
            Ok(decls.pop().unwrap())
        } else {
            Ok(Stmt::Group(decls))
        }
    }

    fn parse_ctor_args(&mut self, ty: &Type) -> PResult<Vec<Expr>> {
        let mut args = Vec::new();
        if matches!(ty, Type::Class(_)) && self.is_punct("(") {
            self.advance();
            if !self.is_punct(")") {
                loop {
                    args.push(self.parse_assign()?);
                    if self.eat_punct(",") {
                        continue;
                    }
                    break;
                }
            }
            self.expect_punct(")")?;
        }
        Ok(args)
    }

    fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_assign()
    }

    fn assign_op(&self) -> Option<(&'static str, Option<BinOp>)> {
        let p = match self.peek() {
            Tok::Punct(s) => s.as_str(),
            _ => return None,
        };
        Some(match p {
            "=" => ("=", None),
            "+=" => ("+=", Some(BinOp::Add)),
            "-=" => ("-=", Some(BinOp::Sub)),
            "*=" => ("*=", Some(BinOp::Mul)),
            "/=" => ("/=", Some(BinOp::Div)),
            "%=" => ("%=", Some(BinOp::Mod)),
            "&=" => ("&=", Some(BinOp::BitAnd)),
            "|=" => ("|=", Some(BinOp::BitOr)),
            "^=" => ("^=", Some(BinOp::BitXor)),
            "<<=" => ("<<=", Some(BinOp::Shl)),
            ">>=" => (">>=", Some(BinOp::Shr)),
            _ => return None,
        })
    }

    fn parse_assign(&mut self) -> PResult<Expr> {
        let line = self.line();
        let lhs = self.parse_ternary()?;
        if let Some((op, bop)) = self.assign_op() {
            let op = op.to_string();
            self.advance();
            let rhs = self.parse_assign()?;
            let kind = if op == "=" {
                ExprKind::Assign(Box::new(lhs), Box::new(rhs))
            } else {
                ExprKind::CompoundAssign(bop.unwrap(), Box::new(lhs), Box::new(rhs))
            };
            return Ok(Expr::new(kind, line));
        }
        Ok(lhs)
    }

    fn parse_ternary(&mut self) -> PResult<Expr> {
        let line = self.line();
        let cond = self.parse_bin(1)?;
        if self.eat_punct("?") {
            let then = self.parse_assign()?;
            self.expect_punct(":")?;
            let els = self.parse_assign()?;
            return Ok(Expr::new(
                ExprKind::Ternary(Box::new(cond), Box::new(then), Box::new(els)),
                line,
            ));
        }
        Ok(cond)
    }

    fn bin_op(&self) -> Option<(BinOp, u8)> {
        let p = match self.peek() {
            Tok::Punct(s) => s.as_str(),
            _ => return None,
        };
        Some(match p {
            "||" => (BinOp::LogOr, 1),
            "&&" => (BinOp::LogAnd, 2),
            "|" => (BinOp::BitOr, 3),
            "^" => (BinOp::BitXor, 4),
            "&" => (BinOp::BitAnd, 5),
            "==" => (BinOp::Eq, 6),
            "!=" => (BinOp::Ne, 6),
            "<" => (BinOp::Lt, 7),
            ">" => (BinOp::Gt, 7),
            "<=" => (BinOp::Le, 7),
            ">=" => (BinOp::Ge, 7),
            "<<" => (BinOp::Shl, 8),
            ">>" => (BinOp::Shr, 8),
            "+" => (BinOp::Add, 9),
            "-" => (BinOp::Sub, 9),
            "*" => (BinOp::Mul, 10),
            "/" => (BinOp::Div, 10),
            "%" => (BinOp::Mod, 10),
            _ => return None,
        })
    }

    fn parse_bin(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let (op, prec) = match self.bin_op() {
                Some(v) => v,
                None => break,
            };
            if prec < min_prec {
                break;
            }
            let line = self.line();
            self.advance();
            let rhs = self.parse_bin(prec + 1)?;
            lhs = Expr::new(ExprKind::Bin(op, Box::new(lhs), Box::new(rhs)), line);
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        let line = self.line();
        if self.is_punct("(") {
            let save = self.pos;
            self.advance();
            if self.is_type_start() {
                if let Ok(t) = self.parse_type_name() {
                    if self.eat_punct(")") {
                        let e = self.parse_unary()?;
                        return Ok(Expr::new(ExprKind::Cast(t, Box::new(e)), line));
                    }
                }
            }
            self.pos = save;
        }
        if self.ident_is("sizeof") {
            self.advance();
            if self.is_punct("(") {
                let save = self.pos;
                self.advance();
                if self.is_type_start() {
                    if let Ok(t) = self.parse_type_name() {
                        if self.eat_punct(")") {
                            return Ok(Expr::new(ExprKind::SizeofType(t), line));
                        }
                    }
                }
                self.pos = save;
            }
            let e = self.parse_unary()?;
            return Ok(Expr::new(ExprKind::SizeofExpr(Box::new(e)), line));
        }
        if self.is_punct("++") || self.is_punct("--") {
            let inc = self.is_punct("++");
            self.advance();
            let target = self.parse_unary()?;
            return Ok(Expr::new(
                ExprKind::IncDec {
                    pre: true,
                    inc,
                    target: Box::new(target),
                },
                line,
            ));
        }
        if self.is_punct("-") {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::new(ExprKind::Un(UnOp::Neg, Box::new(e)), line));
        }
        if self.is_punct("+") {
            self.advance();
            return self.parse_unary();
        }
        if self.is_punct("!") {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::new(ExprKind::Un(UnOp::Not, Box::new(e)), line));
        }
        if self.is_punct("~") {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::new(ExprKind::Un(UnOp::BitNot, Box::new(e)), line));
        }
        if self.is_punct("*") {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::new(ExprKind::Un(UnOp::Deref, Box::new(e)), line));
        }
        if self.is_punct("&") {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr::new(ExprKind::Un(UnOp::AddrOf, Box::new(e)), line));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut e = self.parse_primary()?;
        loop {
            let line = self.line();
            if self.eat_punct("[") {
                let idx = self.parse_expr()?;
                self.expect_punct("]")?;
                e = Expr::new(ExprKind::Index(Box::new(e), Box::new(idx)), line);
            } else if self.is_punct("(") {
                self.advance();
                let mut args = Vec::new();
                if !self.is_punct(")") {
                    loop {
                        args.push(self.parse_assign()?);
                        if self.eat_punct(",") {
                            continue;
                        }
                        break;
                    }
                }
                self.expect_punct(")")?;
                e = self.make_call(e, args, line);
            } else if self.is_punct(".") || self.is_punct("->") {
                let arrow = self.is_punct("->");
                self.advance();
                let field = self.expect_ident()?;
                e = Expr::new(
                    ExprKind::Member {
                        base: Box::new(e),
                        field,
                        arrow,
                    },
                    line,
                );
            } else if self.is_punct("++") || self.is_punct("--") {
                let inc = self.is_punct("++");
                self.advance();
                e = Expr::new(
                    ExprKind::IncDec {
                        pre: false,
                        inc,
                        target: Box::new(e),
                    },
                    line,
                );
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn make_call(&self, callee: Expr, args: Vec<Expr>, line: usize) -> Expr {
        if let ExprKind::Ident(name) = &callee.kind {
            if name == "WIFEXITED" && args.len() == 1 {
                let mut it = args.into_iter();
                let a = it.next().unwrap();
                return Expr::new(
                    ExprKind::Call(
                        Box::new(Expr::new(
                            ExprKind::Ident("__fallen_wifexited".into()),
                            line,
                        )),
                        vec![a],
                    ),
                    line,
                );
            }
            if name == "WIFSIGNALED" && args.len() == 1 {
                let mut it = args.into_iter();
                let a = it.next().unwrap();
                return Expr::new(
                    ExprKind::Call(
                        Box::new(Expr::new(
                            ExprKind::Ident("__fallen_wifsignaled".into()),
                            line,
                        )),
                        vec![a],
                    ),
                    line,
                );
            }
        }
        Expr::new(ExprKind::Call(Box::new(callee), args), line)
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let line = self.line();
        match self.peek().clone() {
            Tok::Int(v) => {
                self.advance();
                Ok(Expr::new(ExprKind::Int(v), line))
            }
            Tok::Char(v) => {
                self.advance();
                Ok(Expr::new(ExprKind::Int(v), line))
            }
            Tok::Str(bytes) => {
                self.advance();
                Ok(Expr::new(ExprKind::Str(bytes), line))
            }
            Tok::Ident(s) => {
                self.advance();
                match s.as_str() {
                    "this" => Ok(Expr::new(ExprKind::This, line)),
                    "NULL" | "nullptr" => Ok(Expr::new(ExprKind::Int(0), line)),
                    "WUNTRACED" => Ok(Expr::new(ExprKind::Int(2), line)),
                    "true" => Ok(Expr::new(ExprKind::Int(1), line)),
                    "false" => Ok(Expr::new(ExprKind::Int(0), line)),
                    _ => Ok(Expr::new(ExprKind::Ident(s), line)),
                }
            }
            Tok::Punct(p) if p == "(" => {
                self.advance();
                let e = self.parse_expr()?;
                self.expect_punct(")")?;
                Ok(e)
            }
            other => Err(format!(
                "satır {}: ifade bekleniyordu, bulunan: {:?}",
                line, other
            )),
        }
    }
}
