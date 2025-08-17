use crate::models;
use crate::query::{Filter, Operator, Query, TagFilter, TextFilter, DueDate};
use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite};

/// Utility functions for building SQL queries from our AST
pub struct SqlQueryBuilder;

impl SqlQueryBuilder {
    /// Build a SQL WHERE clause from a Query AST
    pub fn build_sql_where_clause<'a>(
        query: &Query,
        qb: &mut QueryBuilder<'a, Sqlite>,
    ) {
        match query {
            Query::Filter(filter) => match filter {
                Filter::Project(name) => {
                    qb.push("p.name = ");
                    qb.push_bind(name.clone());
                }
                Filter::Status(status) => {
                    qb.push("th.status = ");
                    qb.push_bind(status.clone());
                }
                Filter::Priority(priority) => {
                    qb.push("th.priority = ");
                    qb.push_bind(priority.clone());
                }
                Filter::Tags(tag_filter) => {
                    Self::build_tag_filter_clause(tag_filter, qb);
                }
                Filter::Name(text_filter) => {
                    Self::build_text_filter_clause(text_filter, "th.name", qb);
                }
                Filter::Description(text_filter) => {
                    Self::build_text_filter_clause(text_filter, "th.description", qb);
                }
                Filter::Due(due_date) => {
                    Self::build_due_date_clause(due_date, qb);
                }
            },
            Query::Not(query) => {
                qb.push("NOT (");
                Self::build_sql_where_clause(query, qb);
                qb.push(")");
            }
            Query::Binary { op, left, right } => {
                qb.push("(");
                Self::build_sql_where_clause(left, qb);
                match op {
                    Operator::And => qb.push(") AND ("),
                    Operator::Or => qb.push(") OR ("),
                };
                Self::build_sql_where_clause(right, qb);
                qb.push(")");
            }
        }
    }

    /// Build SQL clause for tag filters
    fn build_tag_filter_clause<'a>(
        tag_filter: &TagFilter,
        qb: &mut QueryBuilder<'a, Sqlite>,
    ) {
        match tag_filter {
            TagFilter::Has(tag) => {
                qb.push("th.id IN (SELECT task_id FROM task_tags WHERE tag_name = ");
                qb.push_bind(tag.clone());
                qb.push(")");
            }
            TagFilter::HasAll(tags) => {
                qb.push("th.id IN (SELECT task_id FROM task_tags WHERE tag_name IN (");
                for (i, tag) in tags.iter().enumerate() {
                    if i > 0 {
                        qb.push(", ");
                    }
                    qb.push_bind(tag.clone());
                }
                qb.push(") GROUP BY task_id HAVING COUNT(DISTINCT tag_name) = ");
                qb.push_bind(tags.len() as i64);
                qb.push(")");
            }
            TagFilter::HasAny(tags) => {
                qb.push("th.id IN (SELECT task_id FROM task_tags WHERE tag_name IN (");
                for (i, tag) in tags.iter().enumerate() {
                    if i > 0 {
                        qb.push(", ");
                    }
                    qb.push_bind(tag.clone());
                }
                qb.push("))");
            }
            TagFilter::Exact(tags) => {
                // Tasks that have exactly these tags (no more, no less)
                qb.push("th.id IN (SELECT task_id FROM task_tags WHERE tag_name IN (");
                for (i, tag) in tags.iter().enumerate() {
                    if i > 0 {
                        qb.push(", ");
                    }
                    qb.push_bind(tag.clone());
                }
                qb.push(") GROUP BY task_id HAVING COUNT(tag_name) = ");
                qb.push_bind(tags.len() as i64);
                qb.push(") AND th.id NOT IN (SELECT task_id FROM task_tags WHERE tag_name NOT IN (");
                for (i, tag) in tags.iter().enumerate() {
                    if i > 0 {
                        qb.push(", ");
                    }
                    qb.push_bind(tag.clone());
                }
                qb.push("))");
            }
            TagFilter::NotHas(tag) => {
                qb.push("th.id NOT IN (SELECT task_id FROM task_tags WHERE tag_name = ");
                qb.push_bind(tag.clone());
                qb.push(")");
            }
            TagFilter::NotHasAny(tags) => {
                qb.push("th.id NOT IN (SELECT task_id FROM task_tags WHERE tag_name IN (");
                for (i, tag) in tags.iter().enumerate() {
                    if i > 0 {
                        qb.push(", ");
                    }
                    qb.push_bind(tag.clone());
                }
                qb.push("))");
            }
        }
    }

    /// Build SQL clause for text filters
    fn build_text_filter_clause<'a>(
        text_filter: &TextFilter,
        column: &str,
        qb: &mut QueryBuilder<'a, Sqlite>,
    ) {
        match text_filter {
            TextFilter::Contains(text) => {
                qb.push(&format!("LOWER({}) LIKE LOWER(", column));
                // Optimize: avoid format! allocation
                let mut pattern = String::with_capacity(text.len() + 2);
                pattern.push('%');
                pattern.push_str(text);
                pattern.push('%');
                qb.push_bind(pattern);
                qb.push(")");
            }
            TextFilter::Equals(text) => {
                qb.push(&format!("LOWER({}) = LOWER(", column));
                qb.push_bind(text.clone());
                qb.push(")");
            }
            TextFilter::StartsWith(text) => {
                qb.push(&format!("LOWER({}) LIKE LOWER(", column));
                // Optimize: avoid format! allocation
                let mut pattern = String::with_capacity(text.len() + 1);
                pattern.push_str(text);
                pattern.push('%');
                qb.push_bind(pattern);
                qb.push(")");
            }
            TextFilter::EndsWith(text) => {
                qb.push(&format!("LOWER({}) LIKE LOWER(", column));
                // Optimize: avoid format! allocation  
                let mut pattern = String::with_capacity(text.len() + 1);
                pattern.push('%');
                pattern.push_str(text);
                qb.push_bind(pattern);
                qb.push(")");
            }
            TextFilter::NotContains(text) => {
                qb.push(&format!("LOWER({}) NOT LIKE LOWER(", column));
                // Optimize: avoid format! allocation
                let mut pattern = String::with_capacity(text.len() + 2);
                pattern.push('%');
                pattern.push_str(text);
                pattern.push('%');
                qb.push_bind(pattern);
                qb.push(")");
            }
        }
    }

    /// Build SQL clause for due date filters
    fn build_due_date_clause<'a>(
        due_date: &DueDate,
        qb: &mut QueryBuilder<'a, Sqlite>,
    ) {
        match due_date {
            DueDate::On(date_time) => {
                qb.push("DATE(th.due_at) = DATE(");
                qb.push_bind(date_time.clone());
                qb.push(")");
            }
            DueDate::Before(date_time) => {
                qb.push("th.due_at < ");
                qb.push_bind(date_time.clone());
            }
            DueDate::After(date_time) => {
                qb.push("th.due_at > ");
                qb.push_bind(date_time.clone());
            }
            DueDate::Today => {
                qb.push("DATE(th.due_at) = DATE('now')");
            }
            DueDate::Tomorrow => {
                qb.push("DATE(th.due_at) = DATE('now', '+1 day')");
            }
            DueDate::Yesterday => {
                qb.push("DATE(th.due_at) = DATE('now', '-1 day')");
            }
            DueDate::Overdue => {
                qb.push("th.due_at < datetime('now') AND th.status = 'pending'");
            }
            DueDate::Within(duration) => {
                let target_date = Utc::now() + *duration;
                qb.push("th.due_at BETWEEN datetime('now') AND ");
                qb.push_bind(target_date);
            }
            DueDate::Ago(duration) => {
                let start_date = Utc::now() - *duration;
                qb.push("th.due_at BETWEEN ");
                qb.push_bind(start_date);
                qb.push(" AND datetime('now')");
            }
        }
    }

    /// Extract filters from Query structure for materialization window calculation
    pub fn extract_filters_from_query(query: &Query) -> Vec<models::Filter> {
        let mut filters = Vec::new();
        Self::collect_filters_recursive(query, &mut filters);
        filters
    }

    /// Recursively collect filters from a Query AST
    fn collect_filters_recursive(query: &Query, filters: &mut Vec<models::Filter>) {
        match query {
            Query::Filter(filter) => {
                // Convert query::Filter to models::Filter if possible
                // For now, only convert DueDate filters since that's what MaterializationManager needs
                match filter {
                    crate::query::Filter::Due(due_date) => {
                        // Convert query::DueDate to models::DueDate
                        let models_due_date = match due_date {
                            crate::query::DueDate::Today => models::DueDate::Today,
                            crate::query::DueDate::Tomorrow => models::DueDate::Tomorrow,
                            crate::query::DueDate::Overdue => models::DueDate::Overdue,
                            crate::query::DueDate::Before(dt) => models::DueDate::Before(*dt),
                            crate::query::DueDate::After(dt) => models::DueDate::After(*dt),
                            _ => return, // Skip other types for now
                        };
                        filters.push(models::Filter::DueDate(models_due_date));
                    }
                    // Skip other filter types for now as they don't affect materialization windows
                    _ => {}
                }
            }
            Query::Not(inner_query) => {
                Self::collect_filters_recursive(inner_query, filters);
            }
            Query::Binary { left, right, .. } => {
                Self::collect_filters_recursive(left, filters);
                Self::collect_filters_recursive(right, filters);
            }
        }
    }
}