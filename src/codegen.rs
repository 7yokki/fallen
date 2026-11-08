use crate::ast::*;
use crate::sema::{ClassInfo, FuncSig};
use crate::types::Type;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

const PARAM_REGS: [&str; 6] = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];

struct FuncState {
    depth: i32,
    epilogue: String,
    break_labels: Vec<String>,
    continue_labels: Vec<String>,
    fork_vars: HashSet<String>,
}

enum ConstInit {
    None,
    Int(i64),
    Bytes(Vec<u8>),
}

struct GlobalDef {
    name: String,
    ty: Type,
    size: usize,
    align: usize,
    init: ConstInit,
}

fn const_init(e: &Expr) -> ConstInit {
    match &e.kind {
        ExprKind::Int(v) => ConstInit::Int(*v),
        ExprKind::Str(b) => ConstInit::Bytes(b.clone()),
        ExprKind::Un(UnOp::Neg, inner) => match const_init(inner) {
            ConstInit::Int(v) => ConstInit::Int(-v),
            _ => ConstInit::None,
        },
        _ => ConstInit::None,
    }
}

pub struct Codegen<'a> {
    classes: &'a HashMap<String, ClassInfo>,
    funcs: &'a HashMap<String, FuncSig>,
    out: String,
    rodata: Vec<(String, Vec<u8>)>,
    str_count: usize,
    label_count: usize,
    helpers: HashSet<String>,
    cur: FuncState,
    globals: Vec<GlobalDef>,
}

impl<'a> Codegen<'a> {
    pub fn new(
        classes: &'a HashMap<String, ClassInfo>,
        funcs: &'a HashMap<String, FuncSig>,
    ) -> Self {
        Codegen {
            classes,
            funcs,
            out: String::new(),
            rodata: Vec::new(),
            str_count: 0,
            label_count: 0,
            helpers: HashSet::new(),
            cur: FuncState {
                depth: 0,
                epilogue: String::new(),
                break_labels: Vec::new(),
                continue_labels: Vec::new(),
                fork_vars: HashSet::new(),
            },
            globals: Vec::new(),
        }
    }

    fn emit(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn emitf(&mut self, args: std::fmt::Arguments<'_>) {
        self.out.write_fmt(args).unwrap();
        self.out.push('\n');
    }

    fn new_label(&mut self, tag: &str) -> String {
        self.label_count += 1;
        format!(".L{}_{}", tag, self.label_count)
    }

    fn intern_string(&mut self, bytes: &[u8]) -> String {
        self.str_count += 1;
        let label = format!(".Lstr_{}", self.str_count);
        self.rodata.push((label.clone(), bytes.to_vec()));
        label
    }

    fn size(&self, ty: &Type) -> usize {
        ty.size(self.classes)
    }

    fn align_of(&self, ty: &Type) -> usize {
        ty.align(self.classes)
    }

    fn class_of(&self, ty: &Type) -> Option<&ClassInfo> {
        match ty {
            Type::Class(n) => self.classes.get(n),
            Type::Ptr(t) => match &**t {
                Type::Class(n) => self.classes.get(n),
                _ => None,
            },
            _ => None,
        }
    }

    fn field_offset(&self, ty: &Type, field: &str) -> Option<usize> {
        self.class_of(ty)
            .and_then(|c| c.fields.get(field))
            .map(|f| f.offset)
    }

    fn func_label(f: &FuncDecl) -> String {
        match &f.owner {
            Some(owner) => {
                if f.is_ctor {
                    format!("{}_ctor", owner)
                } else {
                    format!("{}_{}", owner, f.name)
                }
            }
            None => f.name.clone(),
        }
    }

    pub fn generate(mut self, program: &Program) -> Result<String, String> {
        self.emit(".text");
        for item in &program.items {
            match item {
                Item::Func(f) => {
                    if !f.is_prototype {
                        self.gen_function(f)?;
                    }
                }
                Item::Class(c) => {
                    for m in &c.methods {
                        if !m.is_prototype {
                            self.gen_function(m)?;
                        }
                    }
                }
                Item::Global(g) => {
                    let size = self.size(&g.ty).max(1);
                    let align = self.align_of(&g.ty).max(1);
                    let init = match &g.init {
                        Some(e) => const_init(e),
                        None => ConstInit::None,
                    };
                    self.globals.push(GlobalDef {
                        name: g.name.clone(),
                        ty: g.ty.clone(),
                        size,
                        align,
                        init,
                    });
                }
            }
        }

        if self.helpers.contains("bounds") {
            self.emit("__fallen_bounds_fail:");
            self.emit("  mov $1, %rax");
            self.emit("  mov $1, %rdi");
            self.emit("  lea .Lpanic_bounds(%rip), %rsi");
            self.emitf(format_args!(
                "  mov ${}, %rdx",
                BOUNDS_MSG.len()
            ));
            self.emit("  syscall");
            self.emit("  and $-16, %rsp");
            self.emit("  mov $1, %edi");
            self.emit("  call exit");
            self.helpers.insert("panic_msg".into());
        }
        if self.helpers.contains("wifexited") {
            self.emit("__fallen_wifexited:");
            self.emit("  mov %edi, %eax");
            self.emit("  and $0x7f, %eax");
            self.emit("  test %eax, %eax");
            self.emit("  sete %al");
            self.emit("  movzbq %al, %rax");
            self.emit("  ret");
        }
        if self.helpers.contains("wifsignaled") {
            self.emit("__fallen_wifsignaled:");
            self.emit("  mov %edi, %eax");
            self.emit("  and $0x7f, %eax");
            self.emit("  add $1, %eax");
            self.emit("  movsbl %al, %eax");
            self.emit("  sar $1, %eax");
            self.emit("  test %eax, %eax");
            self.emit("  setg %al");
            self.emit("  movzbq %al, %rax");
            self.emit("  ret");
        }

        let globals = std::mem::take(&mut self.globals);
        let initialized: Vec<&GlobalDef> = globals
            .iter()
            .filter(|g| !matches!(g.init, ConstInit::None))
            .collect();
        if !initialized.is_empty() {
            self.emit(".data");
            for g in &initialized {
                self.emitf(format_args!(".globl {}", g.name));
                self.emitf(format_args!(".align {}", g.align));
                self.emitf(format_args!("{}:", g.name));
                match &g.init {
                    ConstInit::Int(v) => match g.size {
                        1 => self.emitf(format_args!("  .byte {}", *v as i8)),
                        2 => self.emitf(format_args!("  .word {}", *v as i16)),
                        4 => self.emitf(format_args!("  .long {}", *v as i32)),
                        _ => self.emitf(format_args!("  .quad {}", v)),
                    },
                    ConstInit::Bytes(bytes) => {
                        if matches!(g.ty, Type::Array(_, _)) {
                            let take = bytes.len().min(g.size);
                            if take > 0 {
                                let mut line = String::from("  .byte ");
                                for (i, b) in bytes.iter().take(take).enumerate() {
                                    if i > 0 {
                                        line.push(',');
                                    }
                                    let _ = write!(line, "{}", b);
                                }
                                self.out.push_str(&line);
                                self.out.push('\n');
                            }
                            if g.size > take {
                                self.emitf(format_args!("  .zero {}", g.size - take));
                            }
                        } else {
                            let label = self.intern_string(bytes);
                            self.emitf(format_args!("  .quad {}", label));
                        }
                    }
                    ConstInit::None => {}
                }
            }
        }
        let uninit: Vec<&GlobalDef> = globals
            .iter()
            .filter(|g| matches!(g.init, ConstInit::None))
            .collect();
        if !uninit.is_empty() {
            self.emit(".bss");
            for g in &uninit {
                self.emitf(format_args!(".globl {}", g.name));
                self.emitf(format_args!(".align {}", g.align));
                self.emitf(format_args!("{}:", g.name));
                self.emitf(format_args!("  .zero {}", g.size));
            }
        }

        let rodata = std::mem::take(&mut self.rodata);
        if !rodata.is_empty() || self.helpers.contains("panic_msg") {
            self.emit(".section .rodata");
            for (label, bytes) in &rodata {
                self.emitf(format_args!("{}:", label));
                let mut line = String::from("  .byte ");
                for (i, b) in bytes.iter().enumerate() {
                    if i > 0 {
                        line.push(',');
                    }
                    let _ = write!(line, "{}", b);
                }
                if bytes.is_empty() {
                    line.push('0');
                }
                self.out.push_str(&line);
                self.out.push('\n');
                self.out.push_str("  .byte 0\n");
            }
            if self.helpers.contains("panic_msg") {
                self.emit(".Lpanic_bounds:");
                let mut line = String::from("  .byte ");
                for (i, b) in BOUNDS_MSG.iter().enumerate() {
                    if i > 0 {
                        line.push(',');
                    }
                    let _ = write!(line, "{}", b);
                }
                self.out.push_str(&line);
                self.out.push('\n');
            }
        }

        self.emit(".section .note.GNU-stack,\"\",@progbits");
        Ok(self.out)
    }

    fn gen_function(&mut self, f: &FuncDecl) -> Result<(), String> {
        let label = Self::func_label(f);
        let epilogue = self.new_label("ret");
        self.cur = FuncState {
            depth: 0,
            epilogue: epilogue.clone(),
            break_labels: Vec::new(),
            continue_labels: Vec::new(),
            fork_vars: HashSet::new(),
        };
        self.emitf(format_args!(".globl {}", label));
        self.emitf(format_args!(".type {}, @function", label));
        self.emitf(format_args!("{}:", label));
        self.emit("  push %rbp");
        self.emit("  mov %rsp, %rbp");
        if f.frame_size > 0 {
            self.emitf(format_args!("  sub ${}, %rsp", f.frame_size));
        }
        self.emit("  xor %eax, %eax");

        let mut reg_idx = 0usize;
        if f.is_method {
            let this_off = f.this_offset.ok_or_else(|| {
                format!("iç hata: {} metodu için this yuvası yok", label)
            })?;
            self.emitf(format_args!("  lea {}(%rbp), %rax", this_off));
            self.emitf(format_args!("  mov {}, (%rax)", PARAM_REGS[reg_idx]));
            reg_idx += 1;
        }
        for p in &f.params {
            if reg_idx >= 6 {
                return Err(format!(
                    "satır {}: 6'dan fazla parametre henüz desteklenmiyor",
                    p.line
                ));
            }
            let off = p.offset.ok_or_else(|| {
                format!("iç hata: '{}' parametresi için yuva yok", p.name)
            })?;
            let ty = &p.ty;
            self.emitf(format_args!("  lea {}(%rbp), %rax", off));
            self.store_at_reg("%rax", reg_idx, ty);
            reg_idx += 1;
        }

        self.gen_stmt(&f.body)?;

        self.emitf(format_args!("{}:", epilogue));
        self.emit("  mov %rbp, %rsp");
        self.emit("  pop %rbp");
        self.emit("  ret");
        self.emitf(format_args!(".size {}, .-{}", label, label));
        Ok(())
    }

    fn store_at_reg(&mut self, addr_reg: &str, reg_idx: usize, ty: &Type) {
        let src = PARAM_REGS[reg_idx];
        let size = self.size(ty);
        match size {
            1 => self.emitf(format_args!("  mov {}, ({})", byte_reg(src), addr_reg)),
            2 => self.emitf(format_args!("  mov {}, ({})", word_reg(src), addr_reg)),
            4 => self.emitf(format_args!("  mov {}, ({})", dword_reg(src), addr_reg)),
            _ => self.emitf(format_args!("  mov {}, ({})", src, addr_reg)),
        }
    }

    fn gen_stmt(&mut self, s: &Stmt) -> Result<(), String> {
        match s {
            Stmt::Var(v) => {
                let off = v.offset.ok_or_else(|| "iç hata: değişken yuvası yok".to_string())?;
                let size = self.size(&v.ty);
                let mut init_done = false;
                if let Some(init) = &v.init {
                    if let (Type::Array(elem, n), ExprKind::Str(bytes)) = (&v.ty, &init.kind) {
                        if **elem == Type::Char {
                            let n = *n;
                            self.zero_at(off, size);
                            for (i, b) in bytes.iter().take(n).enumerate() {
                                self.emitf(format_args!("  movb ${}, {}(%rbp)", b, off + i as i64));
                            }
                            if bytes.len() < n {
                                self.emitf(format_args!("  movb $0, {}(%rbp)", off + bytes.len() as i64));
                            }
                            init_done = true;
                        }
                    }
                    if !init_done {
                        self.gen_expr(init)?;
                        self.emitf(format_args!("  lea {}(%rbp), %rcx", off));
                        self.store_at("%rcx", &v.ty);
                    }
                } else {
                    self.zero_at(off, size);
                }
                if let Some(init) = &v.init {
                    if is_call_named(init, "fork") {
                        self.cur.fork_vars.insert(v.name.clone());
                    }
                }
                if let Type::Class(cname) = &v.ty {
                    let has_ctor = self
                        .classes
                        .get(cname)
                        .map(|c| c.has_ctor)
                        .unwrap_or(false);
                    if has_ctor && v.init.is_none() {
                        self.emitf(format_args!("  lea {}(%rbp), %rax", off));
                        self.push();
                        let mut count = 1usize;
                        for a in &v.ctor_args {
                            self.gen_expr(a)?;
                            self.push();
                            count += 1;
                        }
                        if count > 6 {
                            return Err(format!(
                                "satır {}: yapıcı için 6'dan fazla argüman desteklenmiyor",
                                v.line
                            ));
                        }
                        for i in (0..count).rev() {
                            self.pop_to(PARAM_REGS[i]);
                        }
                        let padded = self.align_call();
                        self.emit("  xor %eax, %eax");
                        self.emitf(format_args!("  call {}_ctor", cname));
                        self.unalign_call(padded);
                    }
                }
            }
            Stmt::Expr(e) => {
                if let ExprKind::Assign(lhs, rhs) = &e.kind {
                    if let ExprKind::Ident(n) = &lhs.kind {
                        if is_call_named(rhs, "fork") {
                            self.cur.fork_vars.insert(n.clone());
                        }
                    }
                }
                self.gen_expr(e)?;
            }
            Stmt::Block(stmts) => {
                for st in stmts {
                    self.gen_stmt(st)?;
                }
            }
            Stmt::Group(stmts) => {
                for st in stmts {
                    self.gen_stmt(st)?;
                }
            }
            Stmt::If(c, t, e) => {
                let child_branch = child_branch_of(c, &self.cur.fork_vars);
                let mut exit_then = false;
                let mut exit_else = false;
                if child_branch == Some(1) && falls_through(t) {
                    exit_then = true;
                }
                if child_branch == Some(2) {
                    if let Some(el) = e {
                        if falls_through(el) {
                            exit_else = true;
                        }
                    }
                }
                let l_else = self.new_label("else");
                let l_end = self.new_label("endif");
                self.gen_expr(c)?;
                self.emit("  test %rax, %rax");
                self.emitf(format_args!("  je {}", l_else));
                self.gen_stmt(t)?;
                if exit_then {
                    self.emit_child_exit();
                }
                self.emitf(format_args!("  jmp {}", l_end));
                self.emitf(format_args!("{}:", l_else));
                if let Some(e) = e {
                    self.gen_stmt(e)?;
                }
                if exit_else {
                    self.emit_child_exit();
                }
                self.emitf(format_args!("{}:", l_end));
            }
            Stmt::While(c, b) => {
                let l_start = self.new_label("while");
                let l_end = self.new_label("endwhile");
                self.cur.break_labels.push(l_end.clone());
                self.cur.continue_labels.push(l_start.clone());
                self.emitf(format_args!("{}:", l_start));
                self.gen_expr(c)?;
                self.emit("  test %rax, %rax");
                self.emitf(format_args!("  je {}", l_end));
                self.gen_stmt(b)?;
                self.emitf(format_args!("  jmp {}", l_start));
                self.emitf(format_args!("{}:", l_end));
                self.cur.break_labels.pop();
                self.cur.continue_labels.pop();
            }
            Stmt::DoWhile(b, c) => {
                let l_start = self.new_label("do");
                let l_end = self.new_label("enddo");
                let l_cont = self.new_label("docond");
                self.cur.break_labels.push(l_end.clone());
                self.cur.continue_labels.push(l_cont.clone());
                self.emitf(format_args!("{}:", l_start));
                self.gen_stmt(b)?;
                self.emitf(format_args!("{}:", l_cont));
                self.gen_expr(c)?;
                self.emit("  test %rax, %rax");
                self.emitf(format_args!("  jne {}", l_start));
                self.emitf(format_args!("{}:", l_end));
                self.cur.break_labels.pop();
                self.cur.continue_labels.pop();
            }
            Stmt::For(init, cond, inc, body) => {
                let l_start = self.new_label("for");
                let l_end = self.new_label("endfor");
                let l_inc = self.new_label("forinc");
                self.cur.break_labels.push(l_end.clone());
                self.cur.continue_labels.push(l_inc.clone());
                if let Some(init) = init {
                    self.gen_stmt(init)?;
                }
                self.emitf(format_args!("{}:", l_start));
                if let Some(cond) = cond {
                    self.gen_expr(cond)?;
                    self.emit("  test %rax, %rax");
                    self.emitf(format_args!("  je {}", l_end));
                }
                self.gen_stmt(body)?;
                self.emitf(format_args!("{}:", l_inc));
                if let Some(inc) = inc {
                    self.gen_expr(inc)?;
                }
                self.emitf(format_args!("  jmp {}", l_start));
                self.emitf(format_args!("{}:", l_end));
                self.cur.break_labels.pop();
                self.cur.continue_labels.pop();
            }
            Stmt::Return(e) => {
                if let Some(e) = e {
                    self.gen_expr(e)?;
                } else {
                    self.emit("  xor %eax, %eax");
                }
                self.emitf(format_args!("  jmp {}", self.cur.epilogue.clone()));
            }
            Stmt::Break(_) => {
                if let Some(l) = self.cur.break_labels.last().cloned() {
                    self.emitf(format_args!("  jmp {}", l));
                } else {
                    return Err("'break' döngü dışında".into());
                }
            }
            Stmt::Continue(_) => {
                if let Some(l) = self.cur.continue_labels.last().cloned() {
                    self.emitf(format_args!("  jmp {}", l));
                } else {
                    return Err("'continue' döngü dışında".into());
                }
            }
            Stmt::Empty => {}
            Stmt::Switch(expr, cases) => {
                // Evaluate the switch expression into %rax, save in %r11
                self.gen_expr(expr)?;
                self.emit("  mov %rax, %r11");

                let end_label = self.new_label("switch_end");
                let case_labels: Vec<String> = cases
                    .iter()
                    .map(|_| self.new_label("case"))
                    .collect();
                let default_index = cases.iter().position(|c| c.value.is_none());

                for (i, case) in cases.iter().enumerate() {
                    if let Some(val) = case.value {
                        self.emitf(format_args!("  cmp ${}, %r11", val));
                        self.emitf(format_args!("  je {}", case_labels[i]));
                    }
                }

                // Jump to default or end
                if let Some(i) = default_index {
                    self.emitf(format_args!("  jmp {}", case_labels[i]));
                } else {
                    self.emitf(format_args!("  jmp {}", end_label));
                }

                // Push break label so inner `break` jumps to end
                self.cur.break_labels.push(end_label.clone());

                for (i, case) in cases.iter().enumerate() {
                    self.emitf(format_args!("{}:", case_labels[i]));
                    for st in &case.body {
                        self.gen_stmt(st)?;
                    }
                }

                self.cur.break_labels.pop();
                self.emitf(format_args!("{}:", end_label));
            }
        }
        Ok(())
    }

    fn zero_at(&mut self, off: i64, size: usize) {
        if size == 0 {
            return;
        }
        if size <= 64 {
            let mut o = 0;
            while o + 8 <= size {
                self.emitf(format_args!("  movq $0, {}(%rbp)", off + o as i64));
                o += 8;
            }
            while o < size {
                self.emitf(format_args!("  movb $0, {}(%rbp)", off + o as i64));
                o += 1;
            }
        } else {
            self.emitf(format_args!("  lea {}(%rbp), %rdi", off));
            self.emit("  xor %eax, %eax");
            self.emitf(format_args!("  mov ${}, %rcx", size));
            self.emit("  rep stosb");
        }
    }

    fn push(&mut self) {
        self.emit("  push %rax");
        self.cur.depth += 1;
    }

    fn pop_to(&mut self, reg: &str) {
        self.emitf(format_args!("  pop {}", reg));
        self.cur.depth -= 1;
    }

    fn emit_child_exit(&mut self) {
        self.emit("  mov $127, %edi");
        self.emit("  call _exit");
    }

    fn align_call(&mut self) -> bool {
        if self.cur.depth % 2 != 0 {
            self.emit("  sub $8, %rsp");
            self.cur.depth += 1;
            true
        } else {
            false
        }
    }

    fn unalign_call(&mut self, padded: bool) {
        if padded {
            self.emit("  add $8, %rsp");
            self.cur.depth -= 1;
        }
    }

    fn load_at(&mut self, addr_reg: &str, ty: &Type) {
        if ty.is_aggregate() {
            return;
        }
        let size = self.size(ty);
        let signed = ty.is_signed();
        match (size, signed) {
            (1, true) => self.emitf(format_args!("  movsbq ({}), %rax", addr_reg)),
            (1, false) => self.emitf(format_args!("  movzbq ({}), %rax", addr_reg)),
            (2, true) => self.emitf(format_args!("  movswq ({}), %rax", addr_reg)),
            (2, false) => self.emitf(format_args!("  movzwq ({}), %rax", addr_reg)),
            (4, true) => self.emitf(format_args!("  movslq ({}), %rax", addr_reg)),
            (4, false) => self.emitf(format_args!("  movl ({}), %eax", addr_reg)),
            _ => self.emitf(format_args!("  mov ({}) , %rax", addr_reg)),
        }
    }

    fn store_at(&mut self, addr_reg: &str, ty: &Type) {
        let size = self.size(ty);
        match size {
            1 => self.emitf(format_args!("  movb %al, ({})", addr_reg)),
            2 => self.emitf(format_args!("  movw %ax, ({})", addr_reg)),
            4 => self.emitf(format_args!("  movl %eax, ({})", addr_reg)),
            _ => self.emitf(format_args!("  movq %rax, ({})", addr_reg)),
        }
    }

    fn gen_expr(&mut self, e: &Expr) -> Result<(), String> {
        let ty = e.ty.clone().unwrap_or(Type::Int);
        match &e.kind {
            ExprKind::Int(v) => {
                if *v >= i32::MIN as i64 && *v <= i32::MAX as i64 {
                    self.emitf(format_args!("  mov ${}, %rax", v));
                } else {
                    self.emitf(format_args!("  movabs ${}, %rax", v));
                }
            }
            ExprKind::Str(bytes) => {
                let label = self.intern_string(bytes);
                self.emitf(format_args!("  lea {}(%rip), %rax", label));
            }
            ExprKind::Ident(_) | ExprKind::This | ExprKind::Member { .. } | ExprKind::Index(_, _) => {
                self.gen_addr(e)?;
                self.load_at("%rax", &ty);
            }
            ExprKind::Un(UnOp::Deref, inner) => {
                self.gen_expr(inner)?;
                self.load_at("%rax", &ty);
            }
            ExprKind::Un(UnOp::AddrOf, inner) => {
                self.gen_addr(inner)?;
            }
            ExprKind::Un(UnOp::Neg, inner) => {
                self.gen_expr(inner)?;
                self.emit("  neg %rax");
            }
            ExprKind::Un(UnOp::Not, inner) => {
                self.gen_expr(inner)?;
                self.emit("  test %rax, %rax");
                self.emit("  sete %al");
                self.emit("  movzbq %al, %rax");
            }
            ExprKind::Un(UnOp::BitNot, inner) => {
                self.gen_expr(inner)?;
                self.emit("  not %rax");
            }
            ExprKind::Assign(lhs, rhs) => {
                self.gen_addr(lhs)?;
                self.push();
                self.gen_expr(rhs)?;
                self.pop_to("%rcx");
                self.store_at("%rcx", &ty);
            }
            ExprKind::CompoundAssign(op, lhs, rhs) => {
                self.gen_addr(lhs)?;
                self.push();
                self.load_at("%rax", &lhs.ty.clone().unwrap_or(ty.clone()));
                self.push();
                self.gen_expr(rhs)?;
                let rt = rhs.ty.clone().unwrap_or(Type::Int);
                self.emit("  mov %rax, %rcx");
                self.pop_to("%rax");
                self.apply_binop(*op, &ty, &rt)?;
                self.pop_to("%rcx");
                self.store_at("%rcx", &ty);
            }
            ExprKind::IncDec { pre, inc, target } => {
                let tty = target.ty.clone().unwrap_or(Type::Int);
                let step = if tty.is_pointer() {
                    tty.pointee().map(|t| self.size(t) as i64).unwrap_or(1)
                } else {
                    1
                };
                self.gen_addr(target)?;
                self.emit("  mov %rax, %rcx");
                self.load_at("%rcx", &tty);
                self.emit("  mov %rax, %rdx");
                if *inc {
                    self.emitf(format_args!("  add ${}, %rax", step));
                } else {
                    self.emitf(format_args!("  sub ${}, %rax", step));
                }
                self.store_at("%rcx", &tty);
                if !*pre {
                    self.emit("  mov %rdx, %rax");
                }
            }
            ExprKind::Bin(op, a, b) => {
                if *op == BinOp::LogAnd || *op == BinOp::LogOr {
                    let l_short = self.new_label("logshort");
                    let l_end = self.new_label("logend");
                    self.gen_expr(a)?;
                    self.emit("  test %rax, %rax");
                    if *op == BinOp::LogAnd {
                        self.emitf(format_args!("  je {}", l_short));
                    } else {
                        self.emitf(format_args!("  jne {}", l_short));
                    }
                    self.gen_expr(b)?;
                    self.emit("  test %rax, %rax");
                    self.emit("  setne %al");
                    self.emit("  movzbq %al, %rax");
                    self.emitf(format_args!("  jmp {}", l_end));
                    self.emitf(format_args!("{}:", l_short));
                    if *op == BinOp::LogAnd {
                        self.emit("  xor %eax, %eax");
                    } else {
                        self.emit("  mov $1, %eax");
                    }
                    self.emitf(format_args!("{}:", l_end));
                } else {
                    self.gen_expr(a)?;
                    self.push();
                    self.gen_expr(b)?;
                    let at = a.ty.clone().unwrap_or(Type::Int);
                    let bt = b.ty.clone().unwrap_or(Type::Int);
                    self.emit("  mov %rax, %rcx");
                    self.pop_to("%rax");
                    self.apply_binop(*op, &at, &bt)?;
                }
            }
            ExprKind::Call(callee, args) => {
                let label = e
                    .call_label
                    .clone()
                    .or_else(|| callee_label_static(callee))
                    .ok_or_else(|| format!("satır {}: çağrı etiketi yok", e.line))?;
                self.gen_call(callee, args, &ty, &label)?;
            }
            ExprKind::Ternary(c, t, f) => {
                let l_else = self.new_label("ternelse");
                let l_end = self.new_label("ternend");
                self.gen_expr(c)?;
                self.emit("  test %rax, %rax");
                self.emitf(format_args!("  je {}", l_else));
                self.gen_expr(t)?;
                self.emitf(format_args!("  jmp {}", l_end));
                self.emitf(format_args!("{}:", l_else));
                self.gen_expr(f)?;
                self.emitf(format_args!("{}:", l_end));
            }
            ExprKind::SizeofType(t) => {
                let s = self.size(t);
                self.emitf(format_args!("  mov ${}, %rax", s));
            }
            ExprKind::SizeofExpr(inner) => {
                let t = inner.ty.clone().unwrap_or(Type::Int);
                let s = self.size(&t);
                self.emitf(format_args!("  mov ${}, %rax", s));
            }
            ExprKind::Cast(t, inner) => {
                self.gen_expr(inner)?;
                match self.size(t) {
                    1 => {
                        if t.is_signed() {
                            self.emit("  movsbq %al, %rax");
                        } else {
                            self.emit("  movzbq %al, %rax");
                        }
                    }
                    2 => {
                        if t.is_signed() {
                            self.emit("  movswq %ax, %rax");
                        } else {
                            self.emit("  movzwq %ax, %rax");
                        }
                    }
                    4 => {
                        if t.is_signed() {
                            self.emit("  movslq %eax, %rax");
                        } else {
                            self.emit("  mov %eax, %eax");
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn apply_binop(&mut self, op: BinOp, at: &Type, bt: &Type) -> Result<(), String> {
        match op {
            BinOp::Add => {
                if at.is_pointer() && bt.is_integer() {
                    let s = at.pointee().map(|t| self.size(t)).unwrap_or(1).max(1) as i64;
                    if s != 1 {
                        self.emitf(format_args!("  imul ${}, %rcx", s));
                    }
                    self.emit("  add %rcx, %rax");
                } else if bt.is_pointer() && at.is_integer() {
                    let s = bt.pointee().map(|t| self.size(t)).unwrap_or(1).max(1) as i64;
                    if s != 1 {
                        self.emitf(format_args!("  imul ${}, %rax", s));
                    }
                    self.emit("  add %rcx, %rax");
                } else {
                    self.emit("  add %rcx, %rax");
                }
            }
            BinOp::Sub => {
                if at.is_pointer() && bt.is_pointer() {
                    let s = at.pointee().map(|t| self.size(t)).unwrap_or(1).max(1) as i64;
                    self.emit("  sub %rcx, %rax");
                    if s != 1 {
                        self.emit("  cqto");
                        self.emitf(format_args!("  mov ${}, %rcx", s));
                        self.emit("  idiv %rcx");
                    }
                } else if at.is_pointer() && bt.is_integer() {
                    let s = at.pointee().map(|t| self.size(t)).unwrap_or(1).max(1) as i64;
                    if s != 1 {
                        self.emitf(format_args!("  imul ${}, %rcx", s));
                    }
                    self.emit("  sub %rcx, %rax");
                } else {
                    self.emit("  sub %rcx, %rax");
                }
            }
            BinOp::Mul => self.emit("  imul %rcx, %rax"),
            BinOp::Div | BinOp::Mod => {
                if at.is_signed() && bt.is_signed() {
                    self.emit("  cqto");
                    self.emit("  idiv %rcx");
                } else {
                    self.emit("  xor %edx, %edx");
                    self.emit("  div %rcx");
                }
                if op == BinOp::Mod {
                    self.emit("  mov %rdx, %rax");
                }
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let unsigned = is_unsigned_type(at) || is_unsigned_type(bt);
                self.emit("  cmp %rcx, %rax");
                let cc = match op {
                    BinOp::Eq => "sete",
                    BinOp::Ne => "setne",
                    BinOp::Lt => {
                        if unsigned { "setb" } else { "setl" }
                    }
                    BinOp::Le => {
                        if unsigned { "setbe" } else { "setle" }
                    }
                    BinOp::Gt => {
                        if unsigned { "seta" } else { "setg" }
                    }
                    BinOp::Ge => {
                        if unsigned { "setae" } else { "setge" }
                    }
                    _ => unreachable!(),
                };
                self.emitf(format_args!("  {} %al", cc));
                self.emit("  movzbq %al, %rax");
            }
            BinOp::BitAnd => self.emit("  and %rcx, %rax"),
            BinOp::BitOr => self.emit("  or %rcx, %rax"),
            BinOp::BitXor => self.emit("  xor %rcx, %rax"),
            BinOp::Shl | BinOp::Shr => {
                if op == BinOp::Shl {
                    self.emit("  shl %cl, %rax");
                } else if at.is_signed() {
                    self.emit("  sar %cl, %rax");
                } else {
                    self.emit("  shr %cl, %rax");
                }
            }
            BinOp::LogAnd | BinOp::LogOr => unreachable!(),
        }
        Ok(())
    }

    fn gen_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        ret_ty: &Type,
        label: &str,
    ) -> Result<(), String> {
        if let ExprKind::Ident(n) = &callee.kind {
            if n == "__fallen_wifexited" {
                self.helpers.insert("wifexited".into());
            } else if n == "__fallen_wifsignaled" {
                self.helpers.insert("wifsignaled".into());
            }
        }

        let mut count = 0usize;
        if let ExprKind::Member { base, .. } = &callee.kind {
            let bt = base.ty.clone().unwrap_or(Type::Int);
            if bt.is_pointer() {
                self.gen_expr(base)?;
            } else {
                self.gen_addr(base)?;
            }
            self.push();
            count += 1;
        }
        for a in args {
            self.gen_expr(a)?;
            self.push();
            count += 1;
        }
        if count > 6 {
            return Err("6'dan fazla argüman henüz desteklenmiyor".into());
        }
        for i in (0..count).rev() {
            self.pop_to(PARAM_REGS[i]);
        }
        let padded = self.align_call();
        self.emit("  xor %eax, %eax");
        self.emitf(format_args!("  call {}", label));
        self.unalign_call(padded);
        match (self.size(ret_ty), ret_ty.is_signed()) {
            (1, true) => self.emit("  movsbq %al, %rax"),
            (1, false) => self.emit("  movzbq %al, %rax"),
            (2, true) => self.emit("  movswq %ax, %rax"),
            (2, false) => self.emit("  movzwq %ax, %rax"),
            (4, true) => self.emit("  movslq %eax, %rax"),
            (4, false) => self.emit("  mov %eax, %eax"),
            _ => {}
        }
        Ok(())
    }

    fn gen_addr(&mut self, e: &Expr) -> Result<(), String> {
        match &e.kind {
            ExprKind::Ident(_) | ExprKind::This => {
                match &e.place {
                    Some(Place::Local(off)) => {
                        self.emitf(format_args!("  lea {}(%rbp), %rax", off));
                    }
                    Some(Place::Global(name)) | Some(Place::ExternVar(name)) => {
                        self.emitf(format_args!("  lea {}(%rip), %rax", name));
                    }
                    None => {
                        return Err(format!("satır {}: çözülemeyen tanımlayıcı", e.line));
                    }
                }
            }
            ExprKind::Member { base, field, .. } => {
                let bt = base.ty.clone().unwrap_or(Type::Int);
                if bt.is_pointer() {
                    self.gen_expr(base)?;
                } else {
                    self.gen_addr(base)?;
                }
                let off = self
                    .field_offset(&bt, field)
                    .ok_or_else(|| format!("satır {}: alan ofseti bulunamadı", e.line))?;
                if off != 0 {
                    self.emitf(format_args!("  add ${}, %rax", off));
                }
            }
            ExprKind::Index(base, idx) => {
                let bt = base.ty.clone().unwrap_or(Type::Int);
                let elem_size = bt.pointee().map(|t| self.size(t)).unwrap_or(1).max(1);
                self.gen_expr(idx)?;
                self.push();
                if bt.is_array() {
                    self.gen_addr(base)?;
                } else {
                    self.gen_expr(base)?;
                }
                self.emit("  mov %rax, %r10");
                self.pop_to("%rcx");
                if let Type::Array(_, n) = &bt {
                    self.helpers.insert("bounds".into());
                    self.emit("  cmp $0, %rcx");
                    self.emit("  jl __fallen_bounds_fail");
                    self.emitf(format_args!("  cmp ${}, %rcx", n));
                    self.emit("  jge __fallen_bounds_fail");
                }
                if elem_size != 1 {
                    self.emitf(format_args!("  imul ${}, %rcx", elem_size));
                }
                self.emit("  add %r10, %rcx");
                self.emit("  mov %rcx, %rax");
            }
            ExprKind::Un(UnOp::Deref, inner) => {
                self.gen_expr(inner)?;
            }
            _ => {
                return Err(format!("satır {}: bu ifade adreslenemez", e.line));
            }
        }
        Ok(())
    }
}

fn is_call_named(e: &Expr, name: &str) -> bool {
    matches!(&e.kind, ExprKind::Call(callee, _) if matches!(&callee.kind, ExprKind::Ident(n) if n == name))
}

fn child_branch_of(cond: &Expr, fork_vars: &HashSet<String>) -> Option<u8> {
    if let ExprKind::Bin(op, a, b) = &cond.kind {
        let ident = match (&a.kind, &b.kind) {
            (ExprKind::Ident(n), ExprKind::Int(0)) => n,
            (ExprKind::Int(0), ExprKind::Ident(n)) => n,
            _ => return None,
        };
        if !fork_vars.contains(ident) {
            return None;
        }
        match op {
            BinOp::Eq => Some(1),
            BinOp::Ne => Some(2),
            _ => None,
        }
    } else {
        None
    }
}

fn falls_through(s: &Stmt) -> bool {
    match s {
        Stmt::Return(_) | Stmt::Break(_) | Stmt::Continue(_) => false,
        Stmt::Block(list) | Stmt::Group(list) => {
            for st in list {
                if !falls_through(st) {
                    return false;
                }
            }
            true
        }
        Stmt::If(_, t, e) => {
            let tf = falls_through(t);
            let ef = match e {
                Some(e) => falls_through(e),
                None => true,
            };
            tf || ef
        }
        Stmt::Switch(_, cases) => {
            if cases.is_empty() {
                return true;
            }
            // Falls through if the last case body falls through
            cases.last().map(|c| {
                c.body.last().map(|s| falls_through(s)).unwrap_or(true)
            }).unwrap_or(true)
        }
        _ => true,
    }
}

fn is_unsigned_type(t: &Type) -> bool {
    matches!(t, Type::UChar | Type::UShort | Type::UInt | Type::ULong)
}

fn callee_label_static(callee: &Expr) -> Option<String> {
    if let ExprKind::Ident(n) = &callee.kind {
        Some(n.clone())
    } else {
        None
    }
}

fn byte_reg(reg: &str) -> &'static str {
    match reg {
        "%rdi" => "%dil",
        "%rsi" => "%sil",
        "%rdx" => "%dl",
        "%rcx" => "%cl",
        "%r8" => "%r8b",
        "%r9" => "%r9b",
        _ => "%al",
    }
}

fn word_reg(reg: &str) -> &'static str {
    match reg {
        "%rdi" => "%di",
        "%rsi" => "%si",
        "%rdx" => "%dx",
        "%rcx" => "%cx",
        "%r8" => "%r8w",
        "%r9" => "%r9w",
        _ => "%ax",
    }
}

fn dword_reg(reg: &str) -> &'static str {
    match reg {
        "%rdi" => "%edi",
        "%rsi" => "%esi",
        "%rdx" => "%edx",
        "%rcx" => "%ecx",
        "%r8" => "%r8d",
        "%r9" => "%r9d",
        _ => "%eax",
    }
}

const BOUNDS_MSG: &[u8] = b"Fallen: dizi sinir ihlali (bellek korumasi)\n";
