use std::str::FromStr;

use pest::iterators::Pairs;
use pest::pratt_parser::PrattParser;
use pest::Parser;
use pest_derive::Parser;
use thiserror::Error;

use rusk_core::models::{TaskPriority, TaskStatus};
use rusk_core::query::{Filter, Operator, Query};

#[derive(Parser)]
#[grammar = "filter.pest"]
pub struct FilterParser;

lazy_static::lazy_static! {
    static ref PRATT_PARSER: PrattParser<Rule> = {
        use pest::pratt_parser::{Assoc::*, Op};
        use Rule::*;

        PrattParser::new()
            .op(Op::infix(or, Left))
            .op(Op::infix(and, Left))
            .op(Op::prefix(not))
    };
}

#[derive(Error, Debug)]
pub enum QueryParseError {
    #[error("Pest parsing error: {0}")]
    Pest(#[from] pest::error::Error<Rule>),
    #[error("Invalid filter expression: {0}")]
    InvalidFilter(String),
    #[error("Unknown rule: {0:?}")]
    UnknownRule(Rule),
    #[error("Invalid status value: {0}")]
    InvalidStatus(String),
    #[error("Invalid priority value: {0}")]
    InvalidPriority(String),
}

fn build_ast(pairs: Pairs<Rule>) -> Result<Query, QueryParseError> {
    PRATT_PARSER
        .map_primary(|primary| match primary.as_rule() {
            Rule::filter_expression => {
                let mut inner = primary.into_inner();
                let key = inner.next().unwrap().as_str();
                let value = inner.next().unwrap().as_str().trim_matches('"');

                let filter = match key {
                    "project" => Filter::Project(value.to_string()),
                    "tag" => Filter::Tag(value.to_string()),
                    "status" => {
                        let status = TaskStatus::from_str(value)
                            .map_err(|_| QueryParseError::InvalidStatus(value.to_string()))?;
                        Filter::Status(status)
                    }
                    "priority" => {
                        let priority = TaskPriority::from_str(value)
                            .map_err(|_| QueryParseError::InvalidPriority(value.to_string()))?;
                        Filter::Priority(priority)
                    }
                    _ => {
                        return Err(QueryParseError::InvalidFilter(format!(
                            "Unknown filter key: {}",
                            key
                        )))
                    }
                };
                Ok(Query::Filter(filter))
            }
            Rule::expression => build_ast(primary.into_inner()),
            rule => Err(QueryParseError::UnknownRule(rule)),
        })
        .map_prefix(|op, rhs| {
            let op_rule = op.as_rule();
            match op_rule {
                Rule::not => Ok(Query::Not(Box::new(rhs?))),
                _ => Err(QueryParseError::UnknownRule(op_rule)),
            }
        })
        .map_infix(|lhs, op, rhs| {
            let op = match op.as_rule() {
                Rule::and => Operator::And,
                Rule::or => Operator::Or,
                rule => return Err(QueryParseError::UnknownRule(rule)),
            };
            Ok(Query::Binary {
                op,
                left: Box::new(lhs?),
                right: Box::new(rhs?),
            })
        })
        .parse(pairs)
}

pub fn parse_query(input: &str) -> Result<Query, QueryParseError> {
    let pairs = FilterParser::parse(Rule::filter_query, input)?.next().unwrap().into_inner();
    build_ast(pairs)
}