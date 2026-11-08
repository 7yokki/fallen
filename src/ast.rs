use crate::types::Type;

#[derive(Clone, Debug)]
pub enum Place {
    Local(i64),
    Global(String),
    ExternVar(String),
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(i64),
    Str(Vec<u8>),
    Ident(String),
    This,
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Un(UnOp, Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    CompoundAssign(BinOp, Box<Expr>, Box<Expr>),
    IncDec {
        pre: bool,
        inc: bool,
        target: Box<Expr>,
    },
    Call(Box<Expr>, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Member {
        base: Box<Expr>,
        field: String,
        arrow: bool,
    },
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    SizeofType(Type),
    SizeofExpr(Box<Expr>),
    Cast(Type, Box<Expr>),
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Option<Type>,
    pub place: Option<Place>,
    pub call_label: Option<String>,
    pub line: usize,
}

impl Expr {
    pub fn new(kind: ExprKind, line: usize) -> Self {
        Expr {
            kind,
            ty: None,
            place: None,
            call_label: None,
            line,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    LogAnd,
    LogOr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
    Deref,
    AddrOf,
}

#[derive(Clone, Debug)]
pub struct VarDecl {
    pub name: String,
    pub ty: Type,
    pub init: Option<Expr>,
    pub ctor_args: Vec<Expr>,
    pub offset: Option<i64>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Var(VarDecl),
    Expr(Expr),
    Block(Vec<Stmt>),
    Group(Vec<Stmt>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    For(Option<Box<Stmt>>, Option<Expr>, Option<Expr>, Box<Stmt>),
    Return(Option<Expr>),
    Break(usize),
    Continue(usize),
    Switch(Expr, Vec<SwitchCase>),
    Empty,
}

#[derive(Clone, Debug)]
pub struct SwitchCase {
    pub value: Option<i64>, // None = default
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub struct FuncDecl {
    pub name: String,
    pub ret: Type,
    pub params: Vec<VarDecl>,
    pub body: Stmt,
    pub owner: Option<String>,
    pub is_ctor: bool,
    pub is_method: bool,
    pub is_private: bool,
    pub is_prototype: bool,
    pub frame_size: usize,
    pub this_offset: Option<i64>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct FieldDecl {
    pub name: String,
    pub ty: Type,
    pub is_private: bool,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct ClassDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<FuncDecl>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub enum Item {
    Class(ClassDecl),
    Func(FuncDecl),
    Global(VarDecl),
}

#[derive(Clone, Debug)]
pub struct Program {
    pub items: Vec<Item>,
}
