//! MIR type definitions

use crate::frontend::rust::ast::RustType;

/// A MIR function body
#[derive(Debug, Clone)]
pub struct MirBody {
    /// Local variable declarations
    pub locals: Vec<MirLocal>,
    /// Basic blocks
    pub blocks: Vec<MirBlock>,
    /// Entry block index
    pub entry_block: BlockId,
    /// Return type
    pub return_type: RustType,
}

impl MirBody {
    pub fn new(return_type: RustType) -> Self {
        Self {
            locals: Vec::new(),
            blocks: Vec::new(),
            entry_block: BlockId(0),
            return_type,
        }
    }

    pub fn add_local(&mut self, ty: RustType, name: Option<String>) -> LocalId {
        let id = LocalId(self.locals.len());
        self.locals.push(MirLocal { id, ty, name });
        id
    }

    pub fn add_block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len());
        self.blocks.push(MirBlock::new(id));
        id
    }

    pub fn block_mut(&mut self, id: BlockId) -> &mut MirBlock {
        &mut self.blocks[id.0]
    }
}

/// A local variable
#[derive(Debug, Clone)]
pub struct MirLocal {
    pub id: LocalId,
    pub ty: RustType,
    pub name: Option<String>,
}

/// Local variable ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub usize);

/// Basic block ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

/// A basic block
#[derive(Debug, Clone)]
pub struct MirBlock {
    pub id: BlockId,
    pub statements: Vec<MirStatement>,
    pub terminator: Option<MirTerminator>,
}

impl MirBlock {
    pub fn new(id: BlockId) -> Self {
        Self {
            id,
            statements: Vec::new(),
            terminator: None,
        }
    }

    pub fn push(&mut self, stmt: MirStatement) {
        self.statements.push(stmt);
    }

    pub fn terminate(&mut self, term: MirTerminator) {
        self.terminator = Some(term);
    }
}

/// A MIR statement
#[derive(Debug, Clone)]
pub enum MirStatement {
    /// Assign a value to a place
    Assign {
        dest: Place,
        value: Rvalue,
    },
    /// Drop a value (for ownership)
    Drop(Place),
    /// No operation
    Nop,
}

/// A terminator instruction
#[derive(Debug, Clone)]
pub enum MirTerminator {
    /// Return from the function
    Return,
    /// Unconditional jump
    Goto(BlockId),
    /// Conditional branch
    If {
        condition: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
    /// Switch on an integer value
    Switch {
        value: Operand,
        targets: Vec<(i64, BlockId)>,
        default: BlockId,
    },
    /// Call a function
    Call {
        func: Operand,
        args: Vec<Operand>,
        dest: Place,
        target: BlockId,
    },
    /// Unreachable (for exhaustive matches, panic, etc.)
    Unreachable,
}

/// A place (lvalue) - where to store a value
#[derive(Debug, Clone)]
pub struct Place {
    pub local: LocalId,
    pub projections: Vec<Projection>,
}

impl Place {
    pub fn local(id: LocalId) -> Self {
        Self {
            local: id,
            projections: Vec::new(),
        }
    }

    pub fn field(mut self, index: usize) -> Self {
        self.projections.push(Projection::Field(index));
        self
    }

    pub fn index(mut self, operand: Operand) -> Self {
        self.projections.push(Projection::Index(Box::new(operand)));
        self
    }

    pub fn deref(mut self) -> Self {
        self.projections.push(Projection::Deref);
        self
    }
}

/// A projection on a place
#[derive(Debug, Clone)]
pub enum Projection {
    /// Field access (by index)
    Field(usize),
    /// Array/slice index
    Index(Box<Operand>),
    /// Dereference
    Deref,
    /// Downcast to variant (for enums)
    Downcast(usize),
}

/// An rvalue (right-hand side of assignment)
#[derive(Debug, Clone)]
pub enum Rvalue {
    /// Use an operand directly
    Use(Operand),
    /// Take a reference
    Ref {
        mutable: bool,
        place: Place,
    },
    /// Binary operation
    BinaryOp {
        op: MirBinOp,
        left: Operand,
        right: Operand,
    },
    /// Unary operation
    UnaryOp {
        op: MirUnaryOp,
        operand: Operand,
    },
    /// Cast
    Cast {
        operand: Operand,
        ty: RustType,
    },
    /// Create an aggregate (struct, tuple, array)
    Aggregate {
        kind: AggregateKind,
        operands: Vec<Operand>,
    },
    /// Get the length of a slice/array
    Len(Place),
}

/// Aggregate kinds
#[derive(Debug, Clone)]
pub enum AggregateKind {
    Tuple,
    Array,
    Struct(String),
    Enum { name: String, variant: usize },
}

/// An operand (value to use in an operation)
#[derive(Debug, Clone)]
pub enum Operand {
    /// Copy from a place
    Copy(Place),
    /// Move from a place
    Move(Place),
    /// A constant
    Constant(MirConstant),
}

/// A constant value
#[derive(Debug, Clone)]
pub enum MirConstant {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    String(String),
    Unit,
    /// Function reference
    Function(String),
}

/// Binary operations in MIR
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Unary operations in MIR
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirUnaryOp {
    Neg,
    Not,
}
