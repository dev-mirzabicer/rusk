use std::str::FromStr;

use pest::iterators::{Pair, Pairs};
use pest::pratt_parser::PrattParser;
use pest::Parser;
use pest_derive::Parser;
use thiserror::Error;
use chrono::{DateTime, Utc, Local, Duration, NaiveDate, TimeZone};
use chrono_english::{parse_date_string, Dialect};

use rusk_core::models::{TaskPriority, TaskStatus};
use rusk_core::query::{DueDate, Filter, Operator, Query, TagFilter, TextFilter};

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
    #[error("Invalid date format: {0}. Try 'today', '2024-01-15', or 'in 2 weeks'")]
    InvalidDateFormat(String),
    #[error("Date parsing error: {0}")]
    DateParseError(String),
}

/// Parse a date value from a pest pair, supporting various date formats
fn parse_date_value(pair: Pair<Rule>) -> Result<DueDate, QueryParseError> {
    let input = pair.as_str();
    
    match pair.as_rule() {
        Rule::keyword_date => match input {
            "today" => Ok(DueDate::Today),
            "tomorrow" => Ok(DueDate::Tomorrow), 
            "yesterday" => Ok(DueDate::Yesterday),
            "overdue" => Ok(DueDate::Overdue),
            _ => Err(QueryParseError::InvalidDateFormat(input.to_string())),
        },
        Rule::iso_date | Rule::iso_datetime => {
            // Parse ISO 8601 format dates
            let parsed_date = if input.contains('T') {
                DateTime::parse_from_rfc3339(input)
                    .map_err(|e| QueryParseError::DateParseError(e.to_string()))?
                    .with_timezone(&Utc)
            } else {
                // Parse date-only format and assume start of day in local timezone
                let naive_date = NaiveDate::parse_from_str(input, "%Y-%m-%d")
                    .map_err(|e| QueryParseError::DateParseError(e.to_string()))?;
                let naive_datetime = naive_date.and_hms_opt(0, 0, 0)
                    .ok_or_else(|| QueryParseError::DateParseError("Invalid time".to_string()))?;
                Local.from_local_datetime(&naive_datetime).single()
                    .ok_or_else(|| QueryParseError::DateParseError("Ambiguous local datetime".to_string()))?
                    .with_timezone(&Utc)
            };
            Ok(DueDate::On(parsed_date))
        },
        Rule::relative_expr => {
            // Parse "in X units" format
            let parts: Vec<&str> = input.split_whitespace().collect();
            if parts.len() >= 3 && parts[0] == "in" {
                let amount: i64 = parts[1].parse()
                    .map_err(|_| QueryParseError::InvalidDateFormat(input.to_string()))?;
                let duration = match parts[2] {
                    "day" | "days" => Duration::days(amount),
                    "week" | "weeks" => Duration::weeks(amount),
                    "month" | "months" => Duration::days(amount * 30), // Approximate
                    "year" | "years" => Duration::days(amount * 365), // Approximate
                    "hour" | "hours" => Duration::hours(amount),
                    "minute" | "minutes" => Duration::minutes(amount),
                    _ => return Err(QueryParseError::InvalidDateFormat(input.to_string())),
                };
                Ok(DueDate::Within(duration))
            } else {
                Err(QueryParseError::InvalidDateFormat(input.to_string()))
            }
        },
        Rule::ago_expr => {
            // Parse "X units ago" format
            let parts: Vec<&str> = input.split_whitespace().collect();
            if parts.len() >= 3 && parts[parts.len() - 1] == "ago" {
                let amount: i64 = parts[0].parse()
                    .map_err(|_| QueryParseError::InvalidDateFormat(input.to_string()))?;
                let duration = match parts[1] {
                    "day" | "days" => Duration::days(amount),
                    "week" | "weeks" => Duration::weeks(amount),
                    "month" | "months" => Duration::days(amount * 30), // Approximate
                    "year" | "years" => Duration::days(amount * 365), // Approximate
                    "hour" | "hours" => Duration::hours(amount),
                    "minute" | "minutes" => Duration::minutes(amount),
                    _ => return Err(QueryParseError::InvalidDateFormat(input.to_string())),
                };
                Ok(DueDate::Ago(duration))
            } else {
                Err(QueryParseError::InvalidDateFormat(input.to_string()))
            }
        },
        Rule::value_quoted | Rule::value_single => {
            // Try to parse with chrono-english for natural language dates
            let cleaned_input = input.trim_matches('"');
            match parse_date_string(cleaned_input, Local::now(), Dialect::Uk) {
                Ok(parsed_date) => Ok(DueDate::On(parsed_date.with_timezone(&Utc))),
                Err(_) => Err(QueryParseError::InvalidDateFormat(cleaned_input.to_string())),
            }
        },
        _ => Err(QueryParseError::InvalidDateFormat(input.to_string())),
    }
}

/// Parse a date comparison (before:date, after:date, on:date)
fn parse_date_comparison(pair: Pair<Rule>) -> Result<DueDate, QueryParseError> {
    let mut inner = pair.into_inner();
    let op = inner.next().unwrap().as_str();
    let date_pair = inner.next().unwrap();
    
    // Parse the date part first
    let base_date = match parse_date_value(date_pair)? {
        DueDate::On(dt) => dt,
        DueDate::Today => Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(),
        DueDate::Tomorrow => (Utc::now() + Duration::days(1)).date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(),
        DueDate::Yesterday => (Utc::now() - Duration::days(1)).date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(),
        _ => return Err(QueryParseError::DateParseError("Cannot use relative dates with comparison operators".to_string())),
    };
    
    match op {
        "before" => Ok(DueDate::Before(base_date)),
        "after" => Ok(DueDate::After(base_date)),
        "on" => Ok(DueDate::On(base_date)),
        _ => Err(QueryParseError::InvalidDateFormat(format!("Unknown date operator: {}", op))),
    }
}

/// Parse a tag list (comma-separated values)
fn parse_tag_list(pair: Pair<Rule>) -> Vec<String> {
    pair.into_inner()
        .map(|p| p.as_str().trim_matches('"').to_string())
        .collect()
}

/// Parse a text filter expression
fn parse_text_filter(pair: Pair<Rule>) -> Result<TextFilter, QueryParseError> {
    let mut inner = pair.into_inner();
    let op = inner.next().unwrap().as_str();
    let value = inner.next().unwrap().as_str().trim_matches('"').to_string();
    
    match op {
        "contains" => Ok(TextFilter::Contains(value)),
        "equals" => Ok(TextFilter::Equals(value)),
        "startswith" => Ok(TextFilter::StartsWith(value)),
        "endswith" => Ok(TextFilter::EndsWith(value)),
        "notcontains" => Ok(TextFilter::NotContains(value)),
        _ => Err(QueryParseError::InvalidFilter(format!("Unknown text operator: {}", op))),
    }
}

/// Parse a tag filter expression  
fn parse_tag_filter(pair: Pair<Rule>) -> Result<TagFilter, QueryParseError> {
    let mut inner = pair.into_inner();
    let op = inner.next().unwrap().as_str();
    let value_part = inner.next().unwrap();
    
    match op {
        "has" => {
            let tag = value_part.as_str().trim_matches('"').to_string();
            Ok(TagFilter::Has(tag))
        }
        "hasall" => {
            let tags = if value_part.as_rule() == Rule::tag_list {
                parse_tag_list(value_part)
            } else {
                vec![value_part.as_str().trim_matches('"').to_string()]
            };
            Ok(TagFilter::HasAll(tags))
        }
        "hasany" => {
            let tags = if value_part.as_rule() == Rule::tag_list {
                parse_tag_list(value_part)
            } else {
                vec![value_part.as_str().trim_matches('"').to_string()]
            };
            Ok(TagFilter::HasAny(tags))
        }
        "exact" => {
            let tags = if value_part.as_rule() == Rule::tag_list {
                parse_tag_list(value_part)
            } else {
                vec![value_part.as_str().trim_matches('"').to_string()]
            };
            Ok(TagFilter::Exact(tags))
        }
        "nothas" => {
            let tag = value_part.as_str().trim_matches('"').to_string();
            Ok(TagFilter::NotHas(tag))
        }
        "nothasany" => {
            let tags = if value_part.as_rule() == Rule::tag_list {
                parse_tag_list(value_part)
            } else {
                vec![value_part.as_str().trim_matches('"').to_string()]
            };
            Ok(TagFilter::NotHasAny(tags))
        }
        _ => Err(QueryParseError::InvalidFilter(format!("Unknown tag operator: {}", op))),
    }
}

fn build_ast(pairs: Pairs<Rule>) -> Result<Query, QueryParseError> {
    PRATT_PARSER
        .map_primary(|primary| match primary.as_rule() {
            Rule::filter_expression => {
                let inner_rule = primary.into_inner().next().unwrap();
                
                match inner_rule.as_rule() {
                    Rule::due_filter => {
                        let mut due_inner = inner_rule.into_inner();
                        let _due_key = due_inner.next().unwrap(); // Skip "due"
                        let date_part = due_inner.next().unwrap();
                        
                        let due_date = match date_part.as_rule() {
                            Rule::date_comparison => parse_date_comparison(date_part)?,
                            _ => parse_date_value(date_part)?,
                        };
                        
                        Ok(Query::Filter(Filter::Due(due_date)))
                    }
                    Rule::tag_filter => {
                        let mut tag_inner = inner_rule.into_inner();
                        let _tag_key = tag_inner.next().unwrap(); // Skip "tags" or "tag"
                        let filter_part = tag_inner.next().unwrap();
                        
                        let tag_filter = match filter_part.as_rule() {
                            Rule::tag_filter_expr => parse_tag_filter(filter_part)?,
                            _ => {
                                // Simple tag:value format - backward compatibility
                                let tag = filter_part.as_str().trim_matches('"').to_string();
                                TagFilter::Has(tag)
                            }
                        };
                        
                        Ok(Query::Filter(Filter::Tags(tag_filter)))
                    }
                    Rule::text_filter => {
                        let mut text_inner = inner_rule.into_inner();
                        let field_key = text_inner.next().unwrap().as_str();
                        let filter_part = text_inner.next().unwrap();
                        
                        let text_filter = match filter_part.as_rule() {
                            Rule::text_filter_expr => parse_text_filter(filter_part)?,
                            _ => {
                                // Simple field:value format - default to contains
                                let value = filter_part.as_str().trim_matches('"').to_string();
                                TextFilter::Contains(value)
                            }
                        };
                        
                        let filter = match field_key {
                            "name" => Filter::Name(text_filter),
                            "description" => Filter::Description(text_filter),
                            _ => return Err(QueryParseError::InvalidFilter(format!(
                                "Unknown text field: {}", field_key
                            ))),
                        };
                        
                        Ok(Query::Filter(filter))
                    }
                    Rule::basic_filter => {
                        let mut basic_inner = inner_rule.into_inner();
                        let key = basic_inner.next().unwrap().as_str();
                        let value = basic_inner.next().unwrap().as_str().trim_matches('"');

                        let filter = match key {
                            "project" => Filter::Project(value.to_string()),
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
                    _ => Err(QueryParseError::UnknownRule(inner_rule.as_rule())),
                }
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