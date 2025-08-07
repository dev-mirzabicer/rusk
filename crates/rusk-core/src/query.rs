use crate::models::{TaskPriority, TaskStatus};

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    Filter(Filter),
    Not(Box<Query>),
    Binary {
        op: Operator,
        left: Box<Query>,
        right: Box<Query>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    Project(String),
    Tag(String),
    Status(TaskStatus),
    Priority(TaskPriority),
    // Due(DueDate), // Add this later
}
