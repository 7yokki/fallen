use crate::ast::*;
use crate::types::{align_up, Type};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct FieldInfo {
    pub name: String,
    pub ty: Type,
    pub offset: usize,
    pub is_private: bool,
}

#[derive(Clone, Debug)]
pub struct MethodInfo {
    pub params: Vec<Type>,
    pub ret: Type,
    pub label: String,
    pub is_ctor: bool,
    pub is_private: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ClassInfo {
    pub name: String,
    pub size: usize,
    pub align: usize,
    pub fields: HashMap<String, FieldInfo>,
    pub field_order: Vec<FieldInfo>,
    pub methods: HashMap<String, MethodInfo>,
    pub has_ctor: bool,
}

#[derive(Clone, Debug)]
pub struct FuncSig {
    pub ret: Type,
    pub params: Vec<Type>,
    pub variadic: bool,
}

#[derive(Clone, Debug)]
pub struct MemoryReport {
    pub function: String,
    pub frame_size: usize,
    pub locals: usize,
    pub aggregate_bytes: usize,
}

pub struct Sema {
    pub classes: HashMap<String, ClassInfo>,
    pub funcs: HashMap<String, FuncSig>,
    pub externs: HashMap<String, FuncSig>,
    pub globals: HashMap<String, Type>,
    pub report: Vec<MemoryReport>,
}

struct VarSlot {
    ty: Type,
    offset: i64,
}

struct FnCtx {
    scopes: Vec<HashMap<String, VarSlot>>,
    used: usize,
    aggregate_bytes: usize,
    ret: Type,
    owner: Option<String>,
    this_offset: Option<i64>,
    loop_depth: usize,
    switch_depth: usize,
}

type SResult<T> = Result<T, String>;

impl Sema {
    pub fn new() -> Self {
        let mut externs = HashMap::new();
        let int = Type::Int;
        let void = Type::Void;
        let cptr = Type::ptr(Type::Char);
        let vptr = Type::ptr(Type::Void);
        let ulong = Type::ULong;
        let mut add = |name: &str, ret: Type, params: Vec<Type>, variadic: bool| {
            externs.insert(
                name.to_string(),
                FuncSig {
                    ret,
                    params,
                    variadic,
                },
            );
        };
        add("printf", int.clone(), vec![cptr.clone()], true);
        add(
            "fprintf",
            int.clone(),
            vec![vptr.clone(), cptr.clone()],
            true,
        );
        add("perror", void.clone(), vec![cptr.clone()], false);
        add("fflush", int.clone(), vec![vptr.clone()], false);
        add(
            "strtok",
            cptr.clone(),
            vec![cptr.clone(), cptr.clone()],
            false,
        );
        add(
            "strcmp",
            int.clone(),
            vec![cptr.clone(), cptr.clone()],
            false,
        );
        add(
            "strcspn",
            ulong.clone(),
            vec![cptr.clone(), cptr.clone()],
            false,
        );
        add(
            "memset",
            vptr.clone(),
            vec![vptr.clone(), int.clone(), ulong.clone()],
            false,
        );
        add(
            "fgets",
            cptr.clone(),
            vec![cptr.clone(), int.clone(), vptr.clone()],
            false,
        );
        add("chdir", int.clone(), vec![cptr.clone()], false);
        add("fork", int.clone(), vec![], false);
        add(
            "waitpid",
            int.clone(),
            vec![int.clone(), Type::ptr(int.clone()), int.clone()],
            false,
        );
        add(
            "execvp",
            int.clone(),
            vec![cptr.clone(), Type::ptr(cptr.clone())],
            false,
        );
        add("exit", void.clone(), vec![int.clone()], false);
        add("strlen", ulong.clone(), vec![cptr.clone()], false);
        add("puts", int.clone(), vec![cptr.clone()], false);
        add("putchar", int.clone(), vec![int.clone()], false);
        add("getchar", int.clone(), vec![], false);
        add("malloc", vptr.clone(), vec![ulong.clone()], false);
        add("calloc", vptr.clone(), vec![ulong.clone(), ulong.clone()], false);
        add("free", void.clone(), vec![vptr.clone()], false);
        add("memcpy", vptr.clone(), vec![vptr.clone(), vptr.clone(), ulong.clone()], false);
        add("memcmp", int.clone(), vec![vptr.clone(), vptr.clone(), ulong.clone()], false);
        add("atoi", int.clone(), vec![cptr.clone()], false);
        add("atol", Type::Long, vec![cptr.clone()], false);
        add("atof", Type::Int, vec![cptr.clone()], false); // mapped to int (no float yet)
        add("abs", int.clone(), vec![int.clone()], false);
        add("labs", Type::Long, vec![Type::Long], false);
        add("sprintf", int.clone(), vec![cptr.clone(), cptr.clone()], true);
        add("snprintf", int.clone(), vec![cptr.clone(), ulong.clone(), cptr.clone()], true);
        add("sscanf", int.clone(), vec![cptr.clone(), cptr.clone()], true);
        add("scanf", int.clone(), vec![cptr.clone()], true);
        add("fscanf", int.clone(), vec![vptr.clone(), cptr.clone()], true);
        add("strcpy", cptr.clone(), vec![cptr.clone(), cptr.clone()], false);
        add("strncpy", cptr.clone(), vec![cptr.clone(), cptr.clone(), ulong.clone()], false);
        add("strcat", cptr.clone(), vec![cptr.clone(), cptr.clone()], false);
        add("strncat", cptr.clone(), vec![cptr.clone(), cptr.clone(), ulong.clone()], false);
        add("strncmp", int.clone(), vec![cptr.clone(), cptr.clone(), ulong.clone()], false);
        add("strchr", cptr.clone(), vec![cptr.clone(), int.clone()], false);
        add("strrchr", cptr.clone(), vec![cptr.clone(), int.clone()], false);
        add("strstr", cptr.clone(), vec![cptr.clone(), cptr.clone()], false);
        add("strnlen", ulong.clone(), vec![cptr.clone(), ulong.clone()], false);
        add("strerror", cptr.clone(), vec![int.clone()], false);
        add("memmove", vptr.clone(), vec![vptr.clone(), vptr.clone(), ulong.clone()], false);
        add("open", int.clone(), vec![cptr.clone(), int.clone()], true);
        add("close", int.clone(), vec![int.clone()], false);
        add("read", Type::Long, vec![int.clone(), vptr.clone(), ulong.clone()], false);
        add("write", Type::Long, vec![int.clone(), vptr.clone(), ulong.clone()], false);
        add("lseek", Type::Long, vec![int.clone(), Type::Long, int.clone()], false);
        add("getenv", cptr.clone(), vec![cptr.clone()], false);
        add("system", int.clone(), vec![cptr.clone()], false);
        add("time", Type::Long, vec![Type::ptr(Type::Long)], false);
        add("sleep", Type::UInt, vec![Type::UInt], false);
        add("usleep", int.clone(), vec![Type::ULong], false);
        add("rand", int.clone(), vec![], false);
        add("srand", Type::Void, vec![Type::UInt], false);
        add("realloc", vptr.clone(), vec![vptr.clone(), ulong.clone()], false);
        add("fopen", vptr.clone(), vec![cptr.clone(), cptr.clone()], false);
        add("fclose", int.clone(), vec![vptr.clone()], false);
        add("fread", ulong.clone(), vec![vptr.clone(), ulong.clone(), ulong.clone(), vptr.clone()], false);
        add("fwrite", ulong.clone(), vec![vptr.clone(), ulong.clone(), ulong.clone(), vptr.clone()], false);
        add("fseek", int.clone(), vec![vptr.clone(), Type::Long, int.clone()], false);
        add("ftell", Type::Long, vec![vptr.clone()], false);
        add("feof", int.clone(), vec![vptr.clone()], false);
        add("ferror", int.clone(), vec![vptr.clone()], false);
        add("rewind", Type::Void, vec![vptr.clone()], false);
        add("remove", int.clone(), vec![cptr.clone()], false);
        add("rename", int.clone(), vec![cptr.clone(), cptr.clone()], false);
        add("isdigit", int.clone(), vec![int.clone()], false);
        add("isalpha", int.clone(), vec![int.clone()], false);
        add("isalnum", int.clone(), vec![int.clone()], false);
        add("isspace", int.clone(), vec![int.clone()], false);
        add("isupper", int.clone(), vec![int.clone()], false);
        add("islower", int.clone(), vec![int.clone()], false);
        add("toupper", int.clone(), vec![int.clone()], false);
        add("tolower", int.clone(), vec![int.clone()], false);
        add("getpid", int.clone(), vec![], false);
        add("getppid", int.clone(), vec![], false);
        add("getuid", Type::UInt, vec![], false);
        add("getcwd", cptr.clone(), vec![cptr.clone(), ulong.clone()], false);
        add("mkdir", int.clone(), vec![cptr.clone(), Type::UInt], false);
        add("rmdir", int.clone(), vec![cptr.clone()], false);
        add("unlink", int.clone(), vec![cptr.clone()], false);
        add("stat", int.clone(), vec![cptr.clone(), vptr.clone()], false);
        add("access", int.clone(), vec![cptr.clone(), int.clone()], false);
        add("signal", vptr.clone(), vec![int.clone(), vptr.clone()], false);
        add("kill", int.clone(), vec![int.clone(), int.clone()], false);
        add("pipe", int.clone(), vec![Type::ptr(int.clone())], false);
        add("dup", int.clone(), vec![int.clone()], false);
        add("dup2", int.clone(), vec![int.clone(), int.clone()], false);
        add("execv", int.clone(), vec![cptr.clone(), Type::ptr(cptr.clone())], false);
        add("execve", int.clone(), vec![cptr.clone(), Type::ptr(cptr.clone()), Type::ptr(cptr.clone())], false);
        add("_exit", Type::Void, vec![int.clone()], false);
        add("__fallen_wifexited", int.clone(), vec![int.clone()], false);
        add("__fallen_wifsignaled", int.clone(), vec![int.clone()], false);

        Sema {
            classes: HashMap::new(),
            funcs: HashMap::new(),
            externs,
            globals: HashMap::new(),
            report: Vec::new(),
        }
    }

    pub fn analyze(&mut self, program: &mut Program) -> SResult<()> {
        let mut class_decls: HashMap<String, ClassDecl> = HashMap::new();
        for item in &program.items {
            if let Item::Class(c) = item {
                if class_decls.insert(c.name.clone(), c.clone()).is_some() {
                    return Err(format!(
                        "satır {}: sınıf '{}' birden fazla tanımlandı",
                        c.line, c.name
                    ));
                }
            }
        }

        let mut computed: HashMap<String, ClassInfo> = HashMap::new();
        let mut visiting: Vec<String> = Vec::new();
        let names: Vec<String> = class_decls.keys().cloned().collect();
        for n in &names {
            self.layout_class(n, &class_decls, &mut computed, &mut visiting)?;
        }
        self.classes = computed;
        let _ = &class_decls;

        let mut func_defined: HashMap<String, bool> = HashMap::new();
        for item in &program.items {
            match item {
                Item::Func(f) => {
                    if self.globals.contains_key(&f.name) {
                        return Err(format!(
                            "satır {}: '{}' hem fonksiyon hem global olarak tanımlanmış",
                            f.line, f.name
                        ));
                    }
                    if let Some(existing_defined) = func_defined.get(&f.name) {
                        if *existing_defined || f.is_prototype {
                            return Err(format!(
                                "satır {}: '{}' fonksiyonu birden fazla tanımlandı",
                                f.line, f.name
                            ));
                        }
                    } else if self.funcs.contains_key(&f.name) {
                        return Err(format!(
                            "satır {}: '{}' fonksiyonu birden fazla tanımlandı",
                            f.line, f.name
                        ));
                    }
                    func_defined.insert(f.name.clone(), !f.is_prototype);
                    self.funcs.insert(
                        f.name.clone(),
                        FuncSig {
                            ret: f.ret.clone(),
                            params: f.params.iter().map(|p| p.ty.clone()).collect(),
                            variadic: false,
                        },
                    );
                }
                Item::Global(g) => {
                    if self.funcs.contains_key(&g.name) {
                        return Err(format!(
                            "satır {}: '{}' hem global hem fonksiyon olarak tanımlanmış",
                            g.line, g.name
                        ));
                    }
                    if self.globals.insert(g.name.clone(), g.ty.clone()).is_some() {
                        return Err(format!(
                            "satır {}: global '{}' birden fazla tanımlandı",
                            g.line, g.name
                        ));
                    }
                }
                Item::Class(_) => {}
            }
        }

        for item in program.items.iter_mut() {
            match item {
                Item::Func(f) => {
                    if !f.is_prototype {
                        self.analyze_func(f)?;
                    }
                }
                Item::Global(g) => {
                    if let Some(init) = &mut g.init {
                        self.analyze_global_init(init)?;
                        let it = init.ty.clone().unwrap_or(Type::Int);
                        check_assignable(&g.ty, &it).map_err(|m| {
                            format!("satır {}: global '{}': {}", g.line, g.name, m)
                        })?;
                    }
                }
                Item::Class(cd) => {
                    for m in cd.methods.iter_mut() {
                        self.analyze_func(m)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn layout_class(
        &self,
        name: &str,
        decls: &HashMap<String, ClassDecl>,
        computed: &mut HashMap<String, ClassInfo>,
        visiting: &mut Vec<String>,
    ) -> SResult<ClassInfo> {
        if let Some(c) = computed.get(name) {
            return Ok(c.clone());
        }
        if visiting.contains(&name.to_string()) {
            return Err(format!(
                "sınıf '{}' kendisini değer olarak içeriyor (sonsuz boyut)",
                name
            ));
        }
        let decl = decls
            .get(name)
            .ok_or_else(|| format!("bilinmeyen sınıf '{}'", name))?
            .clone();
        visiting.push(name.to_string());

        for f in &decl.fields {
            self.ensure_type_size(&f.ty, decls, computed, visiting)?;
        }

        let mut offset = 0usize;
        let mut max_align = 1usize;
        let mut fields = HashMap::new();
        let mut order = Vec::new();
        for f in &decl.fields {
            let a = f.ty.align(computed);
            let s = f.ty.size(computed);
            offset = align_up(offset, a);
            max_align = max_align.max(a);
            let fi = FieldInfo {
                name: f.name.clone(),
                ty: f.ty.clone(),
                offset,
                is_private: f.is_private,
            };
            if fields.insert(f.name.clone(), fi.clone()).is_some() {
                return Err(format!(
                    "satır {}: '{}' sınıfında '{}' alanı birden fazla tanımlandı",
                    f.line, name, f.name
                ));
            }
            order.push(fi);
            offset += s;
        }
        let size = align_up(offset.max(1), max_align);
        let mut methods = HashMap::new();
        let mut has_ctor = false;
        for m in &decl.methods {
            if methods.contains_key(&m.name) {
                return Err(format!(
                    "satır {}: '{}' sınıfında '{}' metodu/yapıcısı birden fazla tanımlandı",
                    m.line, name, m.name
                ));
            }
            if fields.contains_key(&m.name) {
                return Err(format!(
                    "satır {}: '{}' sınıfında '{}' adı hem alan hem metot olarak kullanılamaz",
                    m.line, name, m.name
                ));
            }
            let label = if m.is_ctor {
                format!("{}_ctor", name)
            } else {
                format!("{}_{}", name, m.name)
            };
            if m.is_ctor {
                has_ctor = true;
            }
            methods.insert(
                m.name.clone(),
                MethodInfo {
                    params: m.params.iter().map(|p| p.ty.clone()).collect(),
                    ret: m.ret.clone(),
                    label,
                    is_ctor: m.is_ctor,
                    is_private: m.is_private,
                },
            );
        }
        visiting.pop();
        let info = ClassInfo {
            name: name.to_string(),
            size,
            align: max_align,
            fields,
            field_order: order,
            methods,
            has_ctor,
        };
        computed.insert(name.to_string(), info.clone());
        Ok(info)
    }

    fn ensure_type_size(
        &self,
        ty: &Type,
        decls: &HashMap<String, ClassDecl>,
        computed: &mut HashMap<String, ClassInfo>,
        visiting: &mut Vec<String>,
    ) -> SResult<()> {
        match ty {
            Type::Class(n) => {
                if !computed.contains_key(n) {
                    self.layout_class(n, decls, computed, visiting)?;
                }
            }
            Type::Array(e, _) => self.ensure_type_size(e, decls, computed, visiting)?,
            Type::Ptr(_) => {}
            _ => {}
        }
        Ok(())
    }

    fn alloc(&self, ctx: &mut FnCtx, ty: &Type) -> i64 {
        let a = ty.align(&self.classes);
        let s = ty.size(&self.classes);
        ctx.used = align_up(ctx.used, a.max(1));
        let off = -((ctx.used + s) as i64);
        ctx.used += s;
        if ty.is_aggregate() {
            ctx.aggregate_bytes += s;
        }
        off
    }

    fn declare(&self, ctx: &mut FnCtx, name: &str, ty: Type, offset: i64) {
        if let Some(scope) = ctx.scopes.last_mut() {
            scope.insert(
                name.to_string(),
                VarSlot {
                    ty,
                    offset,
                },
            );
        }
    }

    fn lookup(&self, ctx: &FnCtx, name: &str) -> Option<(Type, i64)> {
        for scope in ctx.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some((v.ty.clone(), v.offset));
            }
        }
        None
    }

    fn analyze_func(&mut self, f: &mut FuncDecl) -> SResult<()> {
        let mut ctx = FnCtx {
            scopes: vec![HashMap::new()],
            used: 0,
            aggregate_bytes: 0,
            ret: f.ret.clone(),
            owner: f.owner.clone(),
            this_offset: None,
            loop_depth: 0,
            switch_depth: 0,
        };

        if f.is_method {
            let owner = f.owner.clone().unwrap();
            let ty = Type::ptr(Type::Class(owner));
            let off = self.alloc(&mut ctx, &ty);
            ctx.this_offset = Some(off);
            f.this_offset = Some(off);
            self.declare(&mut ctx, "this", ty, off);
        }

        for p in f.params.iter_mut() {
            if ctx.scopes[0].contains_key(&p.name) {
                return Err(format!(
                    "satır {}: '{}' parametresi birden fazla tanımlandı",
                    p.line, p.name
                ));
            }
            let off = self.alloc(&mut ctx, &p.ty);
            p.offset = Some(off);
            self.declare(&mut ctx, p.name.clone().as_str(), p.ty.clone(), off);
        }

        self.analyze_stmt(&mut f.body, &mut ctx)?;
        let is_main = f.owner.is_none() && f.name == "main";
        if !matches!(f.ret, Type::Void) && !is_main && !stmt_always_returns(&f.body) {
            return Err(format!(
                "satır {}: '{}' non-void fonksiyonunun tüm yollarında 'return' yok",
                f.line, f.name
            ));
        }
        let frame = align_up(ctx.used, 16);
        f.frame_size = frame;
        self.report.push(MemoryReport {
            function: if let Some(o) = &f.owner {
                format!("{}::{}", o, f.name)
            } else {
                f.name.clone()
            },
            frame_size: frame,
            locals: ctx.used,
            aggregate_bytes: ctx.aggregate_bytes,
        });
        Ok(())
    }

    fn analyze_stmt(&mut self, s: &mut Stmt, ctx: &mut FnCtx) -> SResult<()> {
        match s {
            Stmt::Var(v) => {
                if let Some(scope) = ctx.scopes.last() {
                    if scope.contains_key(&v.name) {
                        return Err(format!(
                            "satır {}: '{}' aynı kapsamda birden fazla tanımlandı",
                            v.line, v.name
                        ));
                    }
                }
                let off = self.alloc(ctx, &v.ty);
                v.offset = Some(off);
                self.declare(ctx, &v.name, v.ty.clone(), off);
                for a in v.ctor_args.iter_mut() {
                    self.analyze_expr(a, ctx)?;
                }
                if let Type::Class(cname) = &v.ty {
                    if v.init.is_some() {
                        return Err(format!(
                            "satır {}: '{}' sınıfı '=' ile kopyalanarak başlatılamaz; yapıcı argümanları kullanın (ör. {} {}(...))",
                            v.line, cname, cname, v.name
                        ));
                    }
                    if let Some(ci) = self.classes.get(cname) {
                        if let Some(ctor) = ci.methods.values().find(|m| m.is_ctor) {
                            if ctor.is_private && ctx.owner.as_deref() != Some(cname.as_str()) {
                                return Err(format!(
                                    "satır {}: '{}::{}' yapıcısı özel (private)",
                                    v.line, cname, cname
                                ));
                            }
                            if v.ctor_args.len() != ctor.params.len() {
                                return Err(format!(
                                    "satır {}: '{}' yapıcısı {} argüman bekliyor, {} verildi",
                                    v.line,
                                    cname,
                                    ctor.params.len(),
                                    v.ctor_args.len()
                                ));
                            }
                            for (i, (arg, param_ty)) in
                                v.ctor_args.iter().zip(ctor.params.iter()).enumerate()
                            {
                                let at = arg.ty.clone().unwrap_or(Type::Int);
                                check_assignable(param_ty, &at).map_err(|m| {
                                    format!(
                                        "satır {}: '{}' yapıcısı {}'. argüman: {}",
                                        v.line,
                                        cname,
                                        i + 1,
                                        m
                                    )
                                })?;
                            }
                        } else if !v.ctor_args.is_empty() {
                            return Err(format!(
                                "satır {}: '{}' sınıfının yapıcısı yok",
                                v.line, cname
                            ));
                        }
                    }
                }
                if let Some(init) = &mut v.init {
                    self.analyze_expr(init, ctx)?;
                    let it = init.ty.clone().unwrap_or(Type::Int);
                    check_assignable(&v.ty, &it).map_err(|m| {
                        format!("satır {}: değişken '{}': {}", v.line, v.name, m)
                    })?;
                }
            }
            Stmt::Expr(e) => {
                self.analyze_expr(e, ctx)?;
            }
            Stmt::Block(stmts) => {
                ctx.scopes.push(HashMap::new());
                for st in stmts {
                    self.analyze_stmt(st, ctx)?;
                }
                ctx.scopes.pop();
            }
            Stmt::Group(stmts) => {
                for st in stmts {
                    self.analyze_stmt(st, ctx)?;
                }
            }
            Stmt::If(c, t, e) => {
                self.analyze_expr(c, ctx)?;
                self.analyze_stmt(t, ctx)?;
                if let Some(e) = e {
                    self.analyze_stmt(e, ctx)?;
                }
            }
            Stmt::While(c, b) => {
                self.analyze_expr(c, ctx)?;
                ctx.loop_depth += 1;
                self.analyze_stmt(b, ctx)?;
                ctx.loop_depth -= 1;
            }
            Stmt::DoWhile(b, c) => {
                ctx.loop_depth += 1;
                self.analyze_stmt(b, ctx)?;
                ctx.loop_depth -= 1;
                self.analyze_expr(c, ctx)?;
            }
            Stmt::For(i, c, inc, b) => {
                ctx.scopes.push(HashMap::new());
                if let Some(i) = i {
                    self.analyze_stmt(i, ctx)?;
                }
                if let Some(c) = c {
                    self.analyze_expr(c, ctx)?;
                }
                if let Some(inc) = inc {
                    self.analyze_expr(inc, ctx)?;
                }
                ctx.loop_depth += 1;
                self.analyze_stmt(b, ctx)?;
                ctx.loop_depth -= 1;
                ctx.scopes.pop();
            }
            Stmt::Return(e) => {
                let ret = ctx.ret.clone();
                match (&ret, e) {
                    (Type::Void, Some(_)) => {
                        return Err("void fonksiyondan değer döndürülemez".into());
                    }
                    (Type::Void, None) => {}
                    (_, None) => {
                        return Err("non-void fonksiyondan 'return' değeri gerekli".into());
                    }
                    (r, Some(expr)) => {
                        self.analyze_expr(expr, ctx)?;
                        let et = expr.ty.clone().unwrap_or(Type::Int);
                        if r.is_aggregate() {
                            return Err(format!(
                                "satır {}: aggregate (dizi/sınıf) dönüş değeri desteklenmiyor",
                                expr.line
                            ));
                        }
                        check_assignable(r, &et).map_err(|m| {
                            format!("satır {}: dönüş değeri: {}", expr.line, m)
                        })?;
                    }
                }
            }
            Stmt::Break(line) => {
                if ctx.loop_depth == 0 && ctx.switch_depth == 0 {
                    return Err(format!("satır {}: 'break' döngü veya switch dışında kullanılamaz", line));
                }
            }
            Stmt::Continue(line) => {
                if ctx.loop_depth == 0 {
                    return Err(format!("satır {}: 'continue' döngü dışında kullanılamaz", line));
                }
            }
            Stmt::Switch(expr, cases) => {
                self.analyze_expr(expr, ctx)?;
                let et = expr.ty.clone().unwrap_or(Type::Int);
                if !et.is_integer() {
                    return Err(format!(
                        "satır {}: switch ifadesi tam sayı tipinde olmalı",
                        expr.line
                    ));
                }
                let mut seen: HashSet<i64> = HashSet::new();
                let mut has_default = false;
                for case in cases.iter() {
                    if let Some(v) = case.value {
                        if !seen.insert(v) {
                            return Err(format!(
                                "satır {}: switch içinde 'case {}' birden fazla tanımlandı",
                                expr.line, v
                            ));
                        }
                    } else if has_default {
                        return Err(format!(
                            "satır {}: switch içinde birden fazla 'default' olamaz",
                            expr.line
                        ));
                    } else {
                        has_default = true;
                    }
                }
                ctx.switch_depth += 1;
                for case in cases {
                    for st in &mut case.body {
                        self.analyze_stmt(st, ctx)?;
                    }
                }
                ctx.switch_depth -= 1;
            }
            Stmt::Empty => {}
        }
        Ok(())
    }

    fn analyze_global_init(&mut self, e: &mut Expr) -> SResult<()> {
        match &mut e.kind {
            ExprKind::Int(_) => {
                e.ty = Some(Type::Int);
                Ok(())
            }
            ExprKind::Str(b) => {
                let n = b.len() + 1;
                e.ty = Some(Type::Array(Box::new(Type::Char), n));
                Ok(())
            }
            ExprKind::Un(UnOp::Neg, inner) => {
                self.analyze_global_init(inner)?;
                e.ty = Some(Type::Int);
                Ok(())
            }
            _ => Err("global değişken başlatıcısı sabit olmalı".into()),
        }
    }

    fn analyze_expr(&mut self, e: &mut Expr, ctx: &mut FnCtx) -> SResult<()> {
        match &mut e.kind {
            ExprKind::Int(_) => {
                e.ty = Some(Type::Int);
            }
            ExprKind::Str(b) => {
                let n = b.len() + 1;
                e.ty = Some(Type::Array(Box::new(Type::Char), n));
            }
            ExprKind::This => {
                let owner = ctx
                    .owner
                    .clone()
                    .ok_or_else(|| format!("satır {}: 'this' sadece metot içinde kullanılabilir", e.line))?;
                e.ty = Some(Type::ptr(Type::Class(owner)));
                if let Some(off) = ctx.this_offset {
                    e.place = Some(Place::Local(off));
                }
            }
            ExprKind::Ident(name) => {
                if let Some((ty, off)) = self.lookup(ctx, name) {
                    e.ty = Some(ty);
                    e.place = Some(Place::Local(off));
                } else if let Some(ty) = self.globals.get(name) {
                    e.ty = Some(ty.clone());
                    e.place = Some(Place::Global(name.clone()));
                } else if name == "stdin" || name == "stdout" || name == "stderr" {
                    e.ty = Some(Type::ptr(Type::Void));
                    e.place = Some(Place::ExternVar(name.clone()));
                } else {
                    return Err(format!(
                        "satır {}: tanımsız değişken '{}'",
                        e.line, name
                    ));
                }
            }
            ExprKind::Bin(op, a, b) => {
                self.analyze_expr(a, ctx)?;
                self.analyze_expr(b, ctx)?;
                let ta = a.ty.clone().unwrap_or(Type::Int);
                let tb = b.ty.clone().unwrap_or(Type::Int);
                let ty = match op {
                    BinOp::LogAnd
                    | BinOp::LogOr
                    | BinOp::Eq
                    | BinOp::Ne
                    | BinOp::Lt
                    | BinOp::Le
                    | BinOp::Gt
                    | BinOp::Ge => Type::Int,
                    BinOp::Add => {
                        if ta.is_pointer() {
                            ta
                        } else if tb.is_pointer() {
                            tb
                        } else {
                            usual_arith(&ta, &tb)
                        }
                    }
                    BinOp::Sub => {
                        if ta.is_pointer() && tb.is_pointer() {
                            Type::Long
                        } else if ta.is_pointer() {
                            ta
                        } else {
                            usual_arith(&ta, &tb)
                        }
                    }
                    _ => usual_arith(&ta, &tb),
                };
                e.ty = Some(ty);
            }
            ExprKind::Un(op, a) => {
                self.analyze_expr(a, ctx)?;
                let ta = a.ty.clone().unwrap_or(Type::Int);
                match op {
                    UnOp::Deref => {
                        let t = ta.pointee().cloned().ok_or_else(|| {
                            format!("satır {}: pointer olmayan değer çözülemez", e.line)
                        })?;
                        e.ty = Some(t);
                    }
                    UnOp::AddrOf => {
                        e.ty = Some(Type::Ptr(Box::new(ta)));
                    }
                    UnOp::Not => e.ty = Some(Type::Int),
                    UnOp::Neg | UnOp::BitNot => e.ty = Some(usual_arith(&ta, &ta)),
                }
            }
            ExprKind::Assign(l, r) => {
                self.analyze_expr(l, ctx)?;
                self.analyze_expr(r, ctx)?;
                if !is_lvalue(l) {
                    return Err(format!("satır {}: atamanın sol tarafı lvalue olmalı", e.line));
                }
                let lt = l.ty.clone().unwrap_or(Type::Int);
                if lt.is_array() || matches!(lt, Type::Class(_)) {
                    return Err(format!("satır {}: dizi/sınıf değerine atama yapılamaz", e.line));
                }
                let rt = r.ty.clone().unwrap_or(Type::Int);
                check_assignable(&lt, &rt)
                    .map_err(|m| format!("satır {}: atama: {}", e.line, m))?;
                e.ty = Some(lt);
            }
            ExprKind::CompoundAssign(_, l, r) => {
                self.analyze_expr(l, ctx)?;
                self.analyze_expr(r, ctx)?;
                if !is_lvalue(l) {
                    return Err(format!("satır {}: atamanın sol tarafı lvalue olmalı", e.line));
                }
                e.ty = l.ty.clone();
            }
            ExprKind::IncDec { target, .. } => {
                self.analyze_expr(target, ctx)?;
                if !is_lvalue(target) {
                    return Err(format!("satır {}: ++/-- lvalue gerektirir", e.line));
                }
                e.ty = target.ty.clone();
            }
            ExprKind::Call(callee, args) => {
                let (sig, display): (Option<FuncSig>, String) = if let ExprKind::Member {
                    base,
                    field,
                    arrow,
                } = &mut callee.kind
                {
                    self.analyze_expr(base, ctx)?;
                    let base_ty = base.ty.clone().unwrap_or(Type::Int);
                    let cname = class_of(&base_ty).ok_or_else(|| {
                        format!("satır {}: metot çağrısı için sınıf/pointer gerekli", e.line)
                    })?;
                    if base_ty.is_pointer() && !*arrow {
                        return Err(format!(
                            "satır {}: pointer üzerinde '.' kullanılamaz, '->' kullanın",
                            e.line
                        ));
                    }
                    if matches!(base_ty, Type::Class(_)) && *arrow {
                        return Err(format!(
                            "satır {}: değer üzerinde '->' kullanılamaz, '.' kullanın",
                            e.line
                        ));
                    }
                    let info = self
                        .classes
                        .get(&cname)
                        .ok_or_else(|| format!("bilinmeyen sınıf '{}'", cname))?;
                    let m = info.methods.get(field.as_str()).ok_or_else(|| {
                        format!("satır {}: '{}' sınıfında '{}' metodu yok", e.line, cname, field)
                    })?;
                    if m.is_private && ctx.owner.as_deref() != Some(cname.as_str()) {
                        return Err(format!(
                            "satır {}: '{}::{}' özel (private) bir metot",
                            e.line, cname, field
                        ));
                    }
                    e.call_label = Some(m.label.clone());
                    (
                        Some(FuncSig {
                            ret: m.ret.clone(),
                            params: m.params.clone(),
                            variadic: false,
                        }),
                        field.clone(),
                    )
                } else if let ExprKind::Ident(name) = &callee.kind {
                    if self.lookup(ctx, name).is_some() {
                        return Err(format!(
                            "satır {}: '{}' bir değişken, fonksiyon değil",
                            e.line, name
                        ));
                    }
                    if let Some(s) = self.externs.get(name) {
                        e.call_label = Some(name.clone());
                        (Some(s.clone()), name.clone())
                    } else if let Some(s) = self.funcs.get(name) {
                        e.call_label = Some(name.clone());
                        (Some(s.clone()), name.clone())
                    } else {
                        return Err(format!(
                            "satır {}: tanımsız fonksiyon '{}'",
                            e.line, name
                        ));
                    }
                } else {
                    return Err(format!("satır {}: geçersiz fonksiyon çağrısı", e.line));
                };
                for a in args.iter_mut() {
                    self.analyze_expr(a, ctx)?;
                }
                if let Some(s) = sig {
                    if !s.variadic && args.len() != s.params.len() {
                        return Err(format!(
                            "satır {}: '{}' fonksiyonu {} argüman bekliyor, {} verildi",
                            e.line,
                            display,
                            s.params.len(),
                            args.len()
                        ));
                    }
                    if s.variadic && args.len() < s.params.len() {
                        return Err(format!(
                            "satır {}: '{}' fonksiyonu en az {} argüman bekliyor, {} verildi",
                            e.line,
                            display,
                            s.params.len(),
                            args.len()
                        ));
                    }
                    for (i, (arg, param_ty)) in
                        args.iter().zip(s.params.iter()).enumerate()
                    {
                        let at = arg.ty.clone().unwrap_or(Type::Int);
                        check_assignable(param_ty, &at).map_err(|m| {
                            format!(
                                "satır {}: '{}' çağrısı {}'. argüman: {}",
                                e.line,
                                display,
                                i + 1,
                                m
                            )
                        })?;
                    }
                    e.ty = Some(s.ret);
                } else {
                    e.ty = Some(Type::Int);
                }
            }
            ExprKind::Index(base, idx) => {
                self.analyze_expr(base, ctx)?;
                self.analyze_expr(idx, ctx)?;
                let bt = base.ty.clone().unwrap_or(Type::Int);
                let elem = bt.pointee().cloned().ok_or_else(|| {
                    format!("satır {}: dizilim/pointer olmayan değer indekslenemez", e.line)
                })?;
                if let (Type::Array(_, n), ExprKind::Int(k)) = (&bt, &idx.kind) {
                    if *k < 0 || *k as usize >= *n {
                        return Err(format!(
                            "satır {}: sabit indeks {} dizi sınırlarının dışında (0..{})",
                            e.line, k, n
                        ));
                    }
                }
                e.ty = Some(elem);
            }
            ExprKind::Member { base, field, arrow } => {
                self.analyze_expr(base, ctx)?;
                let bt = base.ty.clone().unwrap_or(Type::Int);
                let cname = class_of(&bt).ok_or_else(|| {
                    format!("satır {}: alan erişimi için sınıf gerekli", e.line)
                })?;
                if matches!(bt, Type::Class(_)) && *arrow {
                    return Err(format!(
                        "satır {}: değer üzerinde '->' kullanılamaz, '.' kullanın",
                        e.line
                    ));
                }
                if bt.is_pointer() && !*arrow {
                    return Err(format!(
                        "satır {}: pointer üzerinde '.' kullanılamaz, '->' kullanın",
                        e.line
                    ));
                }
                let info = self
                    .classes
                    .get(&cname)
                    .ok_or_else(|| format!("bilinmeyen sınıf '{}'", cname))?;
                let fi = info.fields.get(field.as_str()).ok_or_else(|| {
                    format!("satır {}: '{}' sınıfında '{}' alanı yok", e.line, cname, field)
                })?;
                if fi.is_private && ctx.owner.as_deref() != Some(cname.as_str()) {
                    return Err(format!(
                        "satır {}: '{}::{}' özel (private) bir alan",
                        e.line, cname, field
                    ));
                }
                e.ty = Some(fi.ty.clone());
            }
            ExprKind::Ternary(c, t, f) => {
                self.analyze_expr(c, ctx)?;
                self.analyze_expr(t, ctx)?;
                self.analyze_expr(f, ctx)?;
                let tt = t.ty.clone().unwrap_or(Type::Int);
                let ft = f.ty.clone().unwrap_or(Type::Int);
                e.ty = Some(if tt == ft { tt } else { usual_arith(&tt, &ft) });
            }
            ExprKind::SizeofType(t) => {
                e.ty = Some(Type::ULong);
                let _ = t;
            }
            ExprKind::SizeofExpr(inner) => {
                self.analyze_expr(inner, ctx)?;
                e.ty = Some(Type::ULong);
            }
            ExprKind::Cast(t, inner) => {
                self.analyze_expr(inner, ctx)?;
                e.ty = Some(t.clone());
            }
        }
        Ok(())
    }
}

fn stmt_always_returns(s: &Stmt) -> bool {
    match s {
        Stmt::Return(_) => true,
        Stmt::Block(list) | Stmt::Group(list) => {
            list.iter().any(|st| stmt_always_returns(st))
        }
        Stmt::If(_, then, Some(other)) => {
            stmt_always_returns(then) && stmt_always_returns(other)
        }
        Stmt::DoWhile(body, _) => stmt_always_returns(body),
        Stmt::Switch(_, cases) => {
            !cases.is_empty()
                && cases.iter().any(|c| c.value.is_none())
                && cases
                    .iter()
                    .all(|c| c.body.iter().any(|st| stmt_always_returns(st)))
        }
        _ => false,
    }
}

fn is_lvalue(e: &Expr) -> bool {
    matches!(
        e.kind,
        ExprKind::Ident(_)
            | ExprKind::Member { .. }
            | ExprKind::Index(_, _)
            | ExprKind::Un(UnOp::Deref, _)
    )
}

fn class_of(ty: &Type) -> Option<String> {
    match ty {
        Type::Class(n) => Some(n.clone()),
        Type::Ptr(t) => match &**t {
            Type::Class(n) => Some(n.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn usual_arith(a: &Type, b: &Type) -> Type {
    if a.is_pointer() {
        return a.clone();
    }
    if b.is_pointer() {
        return b.clone();
    }
    if matches!(a, Type::ULong) || matches!(b, Type::ULong) {
        Type::ULong
    } else if matches!(a, Type::Long) || matches!(b, Type::Long) {
        Type::Long
    } else if matches!(a, Type::UInt) || matches!(b, Type::UInt) {
        Type::UInt
    } else {
        Type::Int
    }
}

fn check_assignable(dst: &Type, src: &Type) -> Result<(), String> {
    if dst == src {
        return Ok(());
    }
    if dst.is_integer() && src.is_integer() {
        return Ok(());
    }
    if dst.is_pointer() && src.is_pointer() {
        return Ok(());
    }
    if dst.is_pointer() && src.is_array() {
        return Ok(());
    }
    if let (Type::Array(da, _), Type::Array(sa, _)) = (dst, src) {
        if da.as_ref() == &Type::Char && sa.as_ref() == &Type::Char {
            return Ok(());
        }
    }
    if dst.is_pointer() && src.is_integer() {
        return Ok(());
    }
    if dst.is_integer() && src.is_pointer() {
        return Ok(());
    }
    if matches!(dst, Type::Class(_)) && matches!(src, Type::Class(_)) {
        return Ok(());
    }
    Err(format!(
        "tip uyuşmazlığı: {:?} değeri {:?} tipine atanamaz",
        src, dst
    ))
}
