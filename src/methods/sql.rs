use crate::{MyError, Result};
use postgres_types::{BorrowToSql, ToSql};
use postgrest_query_parser::ast::{Field, Select};
use postgrest_query_parser::Ast;

pub fn format_params_ast(
    ast: Ast,
    table_name: String,
) -> Result<(String, Vec<Box<dyn ToSql + Sync + Send>>)> {
    dbg!(&ast);

    let select = format_select(&ast.select)?;

    Ok((format!("SELECT {select} FROM {table_name}"), Vec::new()))
}

pub fn format_select(select: &Option<Select>) -> Result<String> {
    if let Some(select) = select {
        let formatted_fields: Result<Vec<_>> =
            select.fields.iter().map(format_select_field).collect();
        Ok(formatted_fields?.join(", "))
    } else {
        Ok(String::from("*"))
    }
}

fn format_select_field(field: &Field) -> Result<String> {
    match field {
        Field::Key(key) => {
            let mut column = key.column.to_string();

            if let Some(alias) = &key.alias {
                column.push_str(" as ");
                column.push_str(&alias);
            };

            Ok(column)
        }
        _ => {
            return Err(MyError::from(anyhow::anyhow!(
                "this select is not supported yet"
            )))
        }
    }
}
