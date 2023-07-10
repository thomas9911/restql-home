use either::Either::{Left, Right};
use futures_util::stream::StreamExt;
use postgres_types::BorrowToSql;
use postgrest_query_parser::Ast;
use sea_query_binder::SqlxValues;
use sea_schema::sea_query::Values;
use sqlx_core::connection::Connection;
use sqlx_core::database::Database;
use sqlx_core::from_row::FromRow;
use sqlx_core::pool::PoolConnection;
use sqlx_core::postgres::PgConnection;
use sqlx_core::statement::Statement;
use sqlx_core::{executor::Executor, postgres::Postgres};
use time::{OffsetDateTime, PrimitiveDateTime};
use tokio_postgres::{Column, Row};

pub mod sql;
use crate::value::OptionalJsonMapWrapper;
use crate::{AppState, JsonMap, MyError, OptionalJsonMap, Result, Value};

pub async fn get_record(
    client: &mut PgConnection,
    (table_name, record_id): (String, String),
    _state: AppState,
) -> Result<Option<OptionalJsonMap>> {
    // let client = state.pool.get().await?;

    // dbg!(serde_json::from_str::<Value>(&record_id));
    // let record_id: Value = serde_json::from_str(&record_id).unwrap_or(Value::String(record_id));
    let record_id = Value::parse_str(&record_id);

    dbg!((&table_name, &record_id));

    let statement = format!("select * from {table_name} where id = $1");
    let statement = client.prepare(&statement).await?;
    // let params = SqlxValues(Values(vec![(&record_id).into()]));
    let params = SqlxValues(Values(vec![]));
    let query = statement.query_with(params);

    let result = match client.fetch_optional(query).await {
        Ok(Some(record)) => Some(OptionalJsonMapWrapper::from_row(&record)?.0),
        Ok(None) => None,
        Err(e) => return Err(e.into()),
    };
    // let data: Vec<_> = result.into_iter().map(row_to_object).collect();
    dbg!(&result);

    Ok(result)
}

pub async fn list_records(
    client: &mut PgConnection,
    table_name: String,
    params: Ast,
    _state: AppState,
) -> Result<Vec<OptionalJsonMap>> {
    let (sql, parameters) = sql::format_params_ast(params, &table_name)?;
    let statement = sql;
    let statement = client.prepare(&statement).await?;
    // let parameters = parameters.iter().map(|x| x.borrow_to_sql());
    // let statement = sqlx_core::query::query(&sql);

    let query = statement.query_with(parameters);

    let result = match client.fetch_all(query).await {
        Ok(record) => record
            .iter()
            .map(OptionalJsonMapWrapper::from_row)
            .collect::<Vec<_>>(),
        Err(e) => return Err(e.into()),
    };
    let result: Result<Vec<_>> = result
        .into_iter()
        .map(|result| match result {
            Ok(x) => Ok(x.0),
            Err(e) => Err(MyError::from(e)),
        })
        .collect();
    result
}

pub async fn insert_record(
    client: &mut PgConnection,
    table_name: String,
    data: JsonMap,
) -> Result<OptionalJsonMap> {
    let mut columns: Vec<_> = data.keys().collect();
    columns.sort_unstable();

    let values: Vec<_> = columns
        .iter()
        .map(|key| data.get(*key).expect("is a valid key").clone().into())
        .collect();

    let columns_text = columns
        .iter()
        .copied()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let values_placeholders = columns
        .iter()
        .enumerate()
        .map(|(mut i, _)| {
            i += 1;
            format!("${i}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    // let client = state.pool.get().await?;
    let statement = format!(
        "INSERT INTO {table_name} ({columns_text}) VALUES ({values_placeholders}) RETURNING *"
    );
    dbg!(&statement);

    let statement = client.prepare(&statement).await?;
    let query = statement.query_with(SqlxValues(Values(values)));

    let mut stream = client.fetch_many(query);

    // let data: Vec<_> = stream.collect().await;

    let mut data = None;
    while let Some(item) = stream.next().await {
        // data.push(row_to_object(item?));
        // data = Some(OptionalJsonMapWrapper::from_row(item)?);
        match item? {
            Left(a) => (),
            Right(row) => {
                data = Some(OptionalJsonMapWrapper::from_row(&row)?);
            }
        }
    }

    let data = data.ok_or_else(|| anyhow::Error::msg("invalid return statement"))?;

    Ok(data.0)
}

// fn try_row_to_object<R: sqlx_core::row::Row<Database = Postgres>>(
//     row: std::result::Result<R, tokio_postgres::Error>,
// ) -> Result<OptionalJsonMap>
// where
//     usize: sqlx_core::column::ColumnIndex<R>,
// {
//     Ok(row_to_object(row?))
// }

// fn row_to_object<R: sqlx_core::row::Row<Database = Postgres>>(row: R) -> OptionalJsonMap
// where
//     usize: sqlx_core::column::ColumnIndex<R>,
// {
//     row.columns()
//         .iter()
//         .enumerate()
//         .map(|x| row_to_pair(x, &row))
//         .collect()
// }

// fn row_to_pair<'a, R: sqlx_core::row::Row<Database = Postgres>, C: sqlx_core::column::Column<Database = Postgres>>(
//     (index, key): (usize, &C),
//     row: &R,
// ) -> (String, Option<Value>)
// where
//     usize: sqlx_core::column::ColumnIndex<R>,
//     &'a str: sqlx_core::decode::Decode<'a, Postgres>,
// {
//     // use tokio_postgres::types::Type;

//     // let value = match key.type_info() {
//     //     // &Type::BOOL => row.get::<_, Option<bool>>(index).map(Value::Bool),
//     //     // &Type::INT2 => row
//     //     //     .get::<_, Option<i16>>(index)
//     //     //     .map(|x| Value::Int(x as i64)),
//     //     // &Type::INT4 => row
//     //     //     .get::<_, Option<i32>>(index)
//     //     //     .map(|x| Value::Int(x as i64)),
//     //     // &Type::INT8 => row.get::<_, Option<i64>>(index).map(Value::Int),
//     //     // &Type::FLOAT4 => row
//     //     //     .get::<_, Option<f32>>(index)
//     //     //     .map(|x| Value::Float(x as f64)),
//     //     // &Type::FLOAT8 => row.get::<_, Option<f64>>(index).map(Value::Float),
//     //     // &Type::TIMESTAMP => row
//     //     //     .get::<_, Option<PrimitiveDateTime>>(index)
//     //     //     .map(Value::DateTime),
//     //     // &Type::TIMESTAMPTZ => row
//     //     //     .get::<_, Option<OffsetDateTime>>(index)
//     //     //     .map(Value::DateTimeTz),
//     //     // &Type::UUID => row.get::<_, Option<Uuid>>(index).map(Value::Uuid),
//     //     // _other_type => row.get::<_, Option<String>>(index).map(Value::String),
//     //     _ => ()
//     // };

//     // let value = row.get(index);

//     let value = row.get(index);
//     (key.name().to_string(), Some(value))
// }
