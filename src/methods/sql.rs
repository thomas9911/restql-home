use std::collections::VecDeque;

use crate::{MyError, Result};
use postgres_types::ToSql;
use postgrest_query_parser::ast::order::{self, OrderItem};
use postgrest_query_parser::ast::{select, Field, FieldKey, Order, Select};
use postgrest_query_parser::Ast;
use sea_query_binder::SqlxValues;
use sea_schema::sea_query::{Value, Values};

pub fn format_params_ast(ast: Ast, table_name: &str) -> Result<(String, SqlxValues)> {
    // ) -> Result<(String, Vec<Box<dyn ToSql + Sync + Send>>)> {
    dbg!(&ast);

    let select = format_select(ast.select.as_ref(), None)?;
    let join_part = format_join(ast.select.as_ref())?;
    let order = format_order(&ast.order)?;
    let limit = format_limit(&ast.limit)?;
    let offset = format_offset(&ast.offset)?;

    Ok(dbg!(
        format!("SELECT {select} FROM {table_name}{join_part}{order}{limit}{offset}"),
        SqlxValues(Values(Vec::new()))
    ))
}

pub fn format_select(select: Option<&Select>, nested: Option<&str>) -> Result<String> {
    if let Some(select) = select {
        let formatted_fields: Result<Vec<_>> = select
            .fields
            .iter()
            .map(|field| format_select_field(field, nested))
            .collect();
        Ok(formatted_fields?.join(", "))
    } else {
        Ok(String::from("*"))
    }
}

fn format_select_field(field: &Field, nested: Option<&str>) -> Result<String> {
    match field {
        Field::Key(key) => format_field_key(key, nested),
        Field::Nested(key, nested_field) => {
            let out = format_field_key(key, nested)?;
            format_select(Some(nested_field), Some(&out))
        }
        _ => {
            return Err(MyError::from(anyhow::anyhow!(
                "this select is not supported yet"
            )))
        }
    }
}

fn format_field_key(key: &FieldKey, nested: Option<&str>) -> Result<String> {
    let mut column = key.column.to_string();
    if let Some(nested) = nested {
        column = format!("{nested}.{column}")
    }

    if let Some(alias) = &key.alias {
        column.push_str(" as ");
        if let Some(nested) = nested {
            column.push_str(&format!("{nested}.{alias}"))
        } else {
            column.push_str(&alias);
        }
    };

    Ok(column)
}

pub fn format_join(select: Option<&Select>) -> Result<String> {
    if let Some(Select { fields }) = select {
        let mut paths = Vec::new();
        let out = collect_paths(fields, &mut paths)?;
    } else {
        return Ok(String::new());
    };

    Ok(String::new())
}

fn collect_paths(fields: &Vec<Field>, paths: &mut Vec<VecDeque<String>>) -> Result<()> {
    let out: Result<_> = fields
        .iter()
        .map(|x| inner_collect_paths(x, paths))
        .collect();
    out?;

    Ok(())
}

fn inner_collect_paths(field: &Field, paths: &mut Vec<VecDeque<String>>) -> Result<()> {
    match field {
        Field::Key(key) => Ok(()),
        Field::Nested(key, Select { fields }) => {
            let mut inner_paths = Vec::new();
            collect_paths(fields, &mut inner_paths)?;

            Ok(())
        }
        _ => {
            return Err(MyError::from(anyhow::anyhow!(
                "this select is not supported yet"
            )))
        }
    }
}

pub fn format_order(order: &Option<Order>) -> Result<String> {
    if let Some(order) = order {
        let formatted_fields: Result<Vec<_>> =
            order.fields.iter().map(format_order_field).collect();
        let order_fields = formatted_fields?.join(", ");
        Ok(format!(" ORDER BY {order_fields}"))
    } else {
        Ok(String::new())
    }
}

fn format_order_field(field: &OrderItem) -> Result<String> {
    let mut ordering = field.field.to_string();
    match field.operator {
        order::Operator::Asc => ordering.push_str(" ASC"),
        order::Operator::Desc => ordering.push_str(" DESC"),
    };

    match field.nulls_position {
        Some(order::NullOption::First) => ordering.push_str(" NULLS FIRST"),
        Some(order::NullOption::Last) => ordering.push_str(" NULLS LAST"),
        None => (),
    };

    Ok(ordering)
}

fn format_limit(limit: &Option<usize>) -> Result<String> {
    match limit {
        Some(limit) => Ok(format!(" LIMIT {limit}")),
        None => Ok(String::new()),
    }
}
fn format_offset(offset: &Option<usize>) -> Result<String> {
    match offset {
        Some(offset) => Ok(format!(" OFFSET {offset}")),
        None => Ok(String::new()),
    }
}

#[cfg(test)]
fn string_to_ast(input: &str) -> Ast {
    let lexer = postgrest_query_parser::Lexer::new(input.chars());
    postgrest_query_parser::Ast::from_lexer(input, lexer).unwrap()
}

#[test]
fn select_format_sql() {
    let input = "select=id,my_artist:artist";
    let (sql, args) = format_params_ast(string_to_ast(input), "testing").unwrap();

    assert_eq!("SELECT id, artist as my_artist FROM testing", sql);
    assert!(args.0 .0.is_empty())
}

#[test]
fn select_with_nested_format_sql() {
    let input = "select=id,projects(id)";
    let (sql, args) = format_params_ast(string_to_ast(input), "testing").unwrap();

    assert_eq!("SELECT id, artist as my_artist FROM testing", sql);
    assert!(args.0 .0.is_empty())
}

#[test]
fn order_by_format_sql() {
    let input = "select=id,artist&order=title.desc,width.asc.nullsfirst,id.desc.nullslast";
    let (sql, args) = format_params_ast(string_to_ast(input), "testing").unwrap();

    assert_eq!(
        "SELECT id, artist FROM testing ORDER BY title DESC, width ASC NULLS FIRST, id DESC NULLS LAST",
        sql
    );
    assert!(args.0 .0.is_empty())
}

#[test]
fn limit_and_offset_format_sql() {
    let input = "limit=512&offset=9321";
    let (sql, args) = format_params_ast(string_to_ast(input), "testing").unwrap();

    assert_eq!("SELECT * FROM testing LIMIT 512 OFFSET 9321", sql);
    assert!(args.0 .0.is_empty())
}
