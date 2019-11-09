use super::super::executor::{Context, Executor};
use super::super::expression::{Expression, Expressions};
use super::super::optimizer;
use super::super::optimizer::Optimizer;
use super::super::parser::ast;
use super::super::schema;
use super::super::types::ResultSet;
use super::Planner;
use crate::Error;
use std::collections::BTreeMap;

/// A query plan
#[derive(Debug)]
pub struct Plan {
    pub root: Node,
}

impl Plan {
    /// Builds a plan from an AST statement
    pub fn build(statement: ast::Statement) -> Result<Self, Error> {
        Planner::new().build(statement)
    }

    /// Executes the plan, consuming it
    pub fn execute(self, mut ctx: Context) -> Result<ResultSet, Error> {
        Ok(ResultSet::from_executor(Executor::execute(&mut ctx, self.root)?))
    }

    /// Optimizes the plan, consuming it
    pub fn optimize(mut self) -> Result<Self, Error> {
        self.root = optimizer::ConstantFolder.optimize(self.root)?;
        Ok(self)
    }
}

/// A plan node
#[derive(Debug)]
pub enum Node {
    CreateTable { schema: schema::Table },
    Delete { table: String, source: Box<Self> },
    DropTable { name: String },
    Filter { source: Box<Self>, predicate: Expression },
    Insert { table: String, columns: Vec<String>, expressions: Vec<Expressions> },
    Limit { source: Box<Self>, limit: u64 },
    Nothing,
    Offset { source: Box<Self>, offset: u64 },
    Order { source: Box<Self>, orders: Vec<(Expression, Order)> },
    Projection { source: Box<Self>, labels: Vec<Option<String>>, expressions: Expressions },
    Scan { table: String },
    // Uses BTreeMap for test stability
    Update { table: String, source: Box<Self>, expressions: BTreeMap<String, Expression> },
}

impl Node {
    /// Recursively transforms nodes by applying functions before and after descending.
    pub fn transform<B, A>(mut self, pre: &B, post: &A) -> Result<Self, Error>
    where
        B: Fn(Self) -> Result<Self, Error>,
        A: Fn(Self) -> Result<Self, Error>,
    {
        self = pre(self)?;
        self = match self {
            n @ Self::CreateTable { .. } => n,
            n @ Self::DropTable { .. } => n,
            n @ Self::Insert { .. } => n,
            n @ Self::Nothing => n,
            n @ Self::Scan { .. } => n,
            Self::Delete { table, source } => {
                Self::Delete { table, source: source.transform(pre, post)?.into() }
            }
            Self::Filter { source, predicate } => {
                Self::Filter { source: source.transform(pre, post)?.into(), predicate }
            }
            Self::Limit { source, limit } => {
                Self::Limit { source: source.transform(pre, post)?.into(), limit }
            }
            Self::Offset { source, offset } => {
                Self::Offset { source: source.transform(pre, post)?.into(), offset }
            }
            Self::Order { source, orders } => {
                Self::Order { source: source.transform(pre, post)?.into(), orders }
            }
            Self::Projection { source, labels, expressions } => Self::Projection {
                source: source.transform(pre, post)?.into(),
                labels,
                expressions,
            },
            Self::Update { table, source, expressions } => {
                Self::Update { table, source: source.transform(pre, post)?.into(), expressions }
            }
        };
        post(self)
    }

    /// Transforms all expressions in a node by calling .transform() on them
    /// with the given functions.
    pub fn transform_expressions<B, A>(self, pre: &B, post: &A) -> Result<Self, Error>
    where
        B: Fn(Expression) -> Result<Expression, Error>,
        A: Fn(Expression) -> Result<Expression, Error>,
    {
        Ok(match self {
            n @ Self::CreateTable { .. } => n,
            n @ Self::Delete { .. } => n,
            n @ Self::DropTable { .. } => n,
            n @ Self::Limit { .. } => n,
            n @ Self::Nothing => n,
            n @ Self::Offset { .. } => n,
            n @ Self::Scan { .. } => n,
            Self::Filter { source, predicate } => {
                Self::Filter { source, predicate: predicate.transform(pre, post)? }
            }
            Self::Insert { table, columns, expressions } => Self::Insert {
                table,
                columns,
                expressions: expressions
                    .into_iter()
                    .map(|exprs| exprs.into_iter().map(|e| e.transform(pre, post)).collect())
                    .collect::<Result<_, Error>>()?,
            },
            Self::Order { source, orders } => Self::Order {
                source,
                orders: orders
                    .into_iter()
                    .map(|(e, o)| e.transform(pre, post).map(|e| (e, o)))
                    .collect::<Result<_, Error>>()?,
            },
            Self::Projection { source, labels, expressions } => Self::Projection {
                source,
                labels,
                expressions: expressions
                    .into_iter()
                    .map(|e| e.transform(pre, post))
                    .collect::<Result<_, Error>>()?,
            },
            Self::Update { table, source, expressions } => Self::Update {
                table,
                source,
                expressions: expressions
                    .into_iter()
                    .map(|(k, e)| e.transform(pre, post).map(|e| (k, e)))
                    .collect::<Result<_, Error>>()?,
            },
        })
    }
}

/// A sort order
#[derive(Debug, PartialEq)]
pub enum Order {
    Ascending,
    Descending,
}

impl From<ast::Order> for Order {
    fn from(o: ast::Order) -> Self {
        match o {
            ast::Order::Ascending => Self::Ascending,
            ast::Order::Descending => Self::Descending,
        }
    }
}