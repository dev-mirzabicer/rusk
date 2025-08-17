use crate::models::{TaskPriority, TaskStatus};
use chrono::{DateTime, Utc, Duration};

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

impl Query {
    /// Helper constructor for creating AND queries from multiple filters
    pub fn and(filters: Vec<Filter>) -> Self {
        if filters.is_empty() {
            return Query::Filter(Filter::Status(TaskStatus::Pending));
        }
        
        if filters.len() == 1 {
            return Query::Filter(filters.into_iter().next().unwrap());
        }
        
        // Create a tree of AND operations
        let mut iter = filters.into_iter();
        let first = Query::Filter(iter.next().unwrap());
        
        iter.fold(first, |acc, filter| Query::Binary {
            op: Operator::And,
            left: Box::new(acc),
            right: Box::new(Query::Filter(filter)),
        })
    }
    
    /// Helper constructor for creating OR queries from multiple filters
    pub fn or(filters: Vec<Filter>) -> Self {
        if filters.is_empty() {
            return Query::Filter(Filter::Status(TaskStatus::Pending));
        }
        
        if filters.len() == 1 {
            return Query::Filter(filters.into_iter().next().unwrap());
        }
        
        // Create a tree of OR operations
        let mut iter = filters.into_iter();
        let first = Query::Filter(iter.next().unwrap());
        
        iter.fold(first, |acc, filter| Query::Binary {
            op: Operator::Or,
            left: Box::new(acc),
            right: Box::new(Query::Filter(filter)),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DueDate {
    /// Specific date/time (exact match)
    On(DateTime<Utc>),
    /// Before this date/time
    Before(DateTime<Utc>),
    /// After this date/time  
    After(DateTime<Utc>),
    /// Today (any time today)
    Today,
    /// Tomorrow (any time tomorrow)
    Tomorrow,
    /// Yesterday (any time yesterday)
    Yesterday,
    /// Tasks that are overdue (due_at < now and status = pending)
    Overdue,
    /// Within a duration from now (e.g., "in 2 weeks")
    Within(Duration),
    /// Duration ago from now (e.g., "2 days ago")
    Ago(Duration),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TagFilter {
    /// Task has this specific tag
    Has(String),
    /// Task has all of these tags
    HasAll(Vec<String>),
    /// Task has any of these tags
    HasAny(Vec<String>),
    /// Task's tags are exactly this set (no more, no less)
    Exact(Vec<String>),
    /// Task does not have this tag
    NotHas(String),
    /// Task does not have any of these tags
    NotHasAny(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextFilter {
    /// Case-insensitive substring search
    Contains(String),
    /// Case-insensitive exact match
    Equals(String),
    /// Case-insensitive prefix match
    StartsWith(String),
    /// Case-insensitive suffix match
    EndsWith(String),
    /// Does not contain substring (case-insensitive)
    NotContains(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    Project(String),
    Tags(TagFilter),
    Status(TaskStatus),
    Priority(TaskPriority),
    Due(DueDate),
    Name(TextFilter),
    Description(TextFilter),
}
