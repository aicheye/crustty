// AST (Abstract Syntax Tree) definitions for the C interpreter

/// Unique identifier for AST nodes, used for tracking execution position
pub type NodeId = usize;

/// Source location information for error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// Base types supported by the interpreter
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseType {
    Int,
    Char,
    Void,
    Struct(String), // Struct name
}

/// Type representation with const qualifier, pointers, and arrays
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    pub base: BaseType,
    pub is_const: bool,
    pub pointer_depth: usize, // 0 = not pointer, 1 = *, 2 = **, etc.
    pub array_dims: Vec<Option<usize>>, // None for unsized dimension (function params)
}

impl Type {
    pub fn new(base: BaseType) -> Self {
        Type {
            base,
            is_const: false,
            pointer_depth: 0,
            array_dims: Vec::new(),
        }
    }

    pub fn with_const(mut self) -> Self {
        self.is_const = true;
        self
    }

    pub fn with_pointer(mut self) -> Self {
        self.pointer_depth += 1;
        self
    }

    pub fn with_array(mut self, size: Option<usize>) -> Self {
        self.array_dims.push(size);
        self
    }
}

/// Binary operators
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Logical
    And,
    Or,
    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    BitShl,
    BitShr,
    // Compound assignment
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
}

/// Unary operators
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnOp {
    Neg,     // -x
    Not,     // !x
    BitNot,  // ~x
    PreInc,  // ++x
    PreDec,  // --x
    PostInc, // x++
    PostDec, // x--
    Deref,   // *x
    AddrOf,  // &x
}

/// Function parameter
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub param_type: Type,
}

/// Struct field
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub field_type: Type,
}

/// Struct definition
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<Field>,
}

/// Switch case
#[derive(Debug, Clone)]
pub enum CaseNode {
    Case {
        value: Box<AstNode>,
        statements: Vec<AstNode>,
        location: SourceLocation,
    },
    Default {
        statements: Vec<AstNode>,
        location: SourceLocation,
    },
}

/// AST nodes representing statements and expressions
#[derive(Debug, Clone)]
pub enum AstNode {
    // Top-level declarations
    FunctionDef {
        name: String,
        params: Vec<Param>,
        body: Vec<AstNode>,
        return_type: Type,
        location: SourceLocation,
    },
    StructDef {
        name: String,
        fields: Vec<Field>,
        location: SourceLocation,
    },

    // Statements
    VarDecl {
        name: String,
        var_type: Type,
        init: Option<Box<AstNode>>,
        location: SourceLocation,
    },
    Assignment {
        lhs: Box<AstNode>,
        rhs: Box<AstNode>,
        location: SourceLocation,
    },
    CompoundAssignment {
        lhs: Box<AstNode>,
        op: BinOp,
        rhs: Box<AstNode>,
        location: SourceLocation,
    },
    Return {
        expr: Option<Box<AstNode>>,
        location: SourceLocation,
    },
    If {
        condition: Box<AstNode>,
        then_branch: Vec<AstNode>,
        else_branch: Option<Vec<AstNode>>,
        location: SourceLocation,
    },
    While {
        condition: Box<AstNode>,
        body: Vec<AstNode>,
        location: SourceLocation,
    },
    DoWhile {
        body: Vec<AstNode>,
        condition: Box<AstNode>,
        location: SourceLocation,
    },
    For {
        init: Option<Box<AstNode>>,
        condition: Option<Box<AstNode>>,
        increment: Option<Box<AstNode>>,
        body: Vec<AstNode>,
        location: SourceLocation,
    },
    Switch {
        expr: Box<AstNode>,
        cases: Vec<CaseNode>,
        location: SourceLocation,
    },
    Break {
        location: SourceLocation,
    },
    Continue {
        location: SourceLocation,
    },
    Goto {
        label: String,
        location: SourceLocation,
    },
    Label {
        name: String,
        location: SourceLocation,
    },
    ExpressionStatement {
        expr: Box<AstNode>,
        location: SourceLocation,
    },

    // Expressions
    IntLiteral(i32, SourceLocation),
    CharLiteral(i8, SourceLocation),
    StringLiteral(String, SourceLocation),
    Null {
        location: SourceLocation,
    },
    Variable(String, SourceLocation),
    BinaryOp {
        op: BinOp,
        left: Box<AstNode>,
        right: Box<AstNode>,
        location: SourceLocation,
    },
    UnaryOp {
        op: UnOp,
        operand: Box<AstNode>,
        location: SourceLocation,
    },
    TernaryOp {
        condition: Box<AstNode>,
        true_expr: Box<AstNode>,
        false_expr: Box<AstNode>,
        location: SourceLocation,
    },
    FunctionCall {
        name: String,
        args: Vec<AstNode>,
        location: SourceLocation,
    },
    ArrayAccess {
        array: Box<AstNode>,
        index: Box<AstNode>,
        location: SourceLocation,
    },
    MemberAccess {
        object: Box<AstNode>,
        member: String,
        location: SourceLocation,
    },
    PointerMemberAccess {
        object: Box<AstNode>,
        member: String,
        location: SourceLocation,
    },
    Cast {
        target_type: Type,
        expr: Box<AstNode>,
        location: SourceLocation,
    },
    SizeofType {
        target_type: Type,
        location: SourceLocation,
    },
    SizeofExpr {
        expr: Box<AstNode>,
        location: SourceLocation,
    },
}

impl AstNode {
    /// Get the source location of this node
    pub fn location(&self) -> &SourceLocation {
        match self {
            AstNode::FunctionDef { location, .. } => location,
            AstNode::StructDef { location, .. } => location,
            AstNode::VarDecl { location, .. } => location,
            AstNode::Assignment { location, .. } => location,
            AstNode::CompoundAssignment { location, .. } => location,
            AstNode::Return { location, .. } => location,
            AstNode::If { location, .. } => location,
            AstNode::While { location, .. } => location,
            AstNode::DoWhile { location, .. } => location,
            AstNode::For { location, .. } => location,
            AstNode::Switch { location, .. } => location,
            AstNode::Break { location, .. } => location,
            AstNode::Continue { location, .. } => location,
            AstNode::Goto { location, .. } => location,
            AstNode::Label { location, .. } => location,
            AstNode::ExpressionStatement { location, .. } => location,
            AstNode::IntLiteral(_, loc) => loc,
            AstNode::CharLiteral(_, loc) => loc,
            AstNode::StringLiteral(_, loc) => loc,
            AstNode::Null { location } => location,
            AstNode::Variable(_, loc) => loc,
            AstNode::BinaryOp { location, .. } => location,
            AstNode::UnaryOp { location, .. } => location,
            AstNode::TernaryOp { location, .. } => location,
            AstNode::FunctionCall { location, .. } => location,
            AstNode::ArrayAccess { location, .. } => location,
            AstNode::MemberAccess { location, .. } => location,
            AstNode::PointerMemberAccess { location, .. } => location,
            AstNode::Cast { location, .. } => location,
            AstNode::SizeofType { location, .. } => location,
            AstNode::SizeofExpr { location, .. } => location,
        }
    }
}

/// Top-level program structure
#[derive(Debug, Clone, Default)]
pub struct Program {
    pub nodes: Vec<AstNode>, // All top-level declarations (FunctionDef, StructDef)
}

impl Program {
    pub fn new() -> Self {
        Program::default()
    }
}
