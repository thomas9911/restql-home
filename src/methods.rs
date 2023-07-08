use futures_util::stream::StreamExt;
use postgres_types::BorrowToSql;
use postgrest_query_parser::Ast;
use time::{OffsetDateTime, PrimitiveDateTime};
use tokio_postgres::{Column, Row};
use uuid::Uuid;

pub mod sql;
use crate::{AppState, JsonMap, OptionalJsonMap, Result, Value};

pub async fn get_record<C: deadpool_postgres::GenericClient>(
    client: &C,
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
    let result = match client.query_opt(&statement, &[&record_id]).await {
        Ok(Some(record)) => Some(row_to_object(record)),
        Ok(None) => None,
        Err(e) => return Err(e.into()),
    };
    // let data: Vec<_> = result.into_iter().map(row_to_object).collect();
    dbg!(&result);

    Ok(result)
}

pub async fn list_records<C: deadpool_postgres::GenericClient>(
    client: &C,
    table_name: String,
    params: Ast,
    _state: AppState,
) -> Result<Vec<OptionalJsonMap>> {
    let (sql, parameters) = sql::format_params_ast(params, table_name)?;
    let statement = sql;
    let statement = client.prepare(&statement).await?;
    let parameters = parameters.iter().map(|x| x.borrow_to_sql());

    let result = match client.query_raw(&statement, parameters).await {
        Ok(record) => record.map(try_row_to_object).collect::<Vec<_>>().await,
        Err(e) => return Err(e.into()),
    };
    let result: Result<Vec<_>> = result.into_iter().collect();
    result
}

pub async fn insert_record<C: deadpool_postgres::GenericClient>(
    client: &C,
    table_name: String,
    data: JsonMap,
) -> Result<OptionalJsonMap> {
    let mut columns: Vec<_> = data.keys().collect();
    columns.sort_unstable();

    let values: Vec<_> = columns
        .iter()
        .map(|key| data.get(*key).expect("is a valid key"))
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
    let stream = client.query_raw(&statement, values.as_slice()).await?;

    // let data: Vec<_> = stream.collect().await;

    let mut data = None;
    let mut s = Box::pin(stream);
    while let Some(item) = s.next().await {
        // data.push(row_to_object(item?));
        data = Some(row_to_object(item?));
    }

    let data = data.ok_or_else(|| anyhow::Error::msg("invalid return statement"))?;

    Ok(data)
}

fn try_row_to_object(
    row: std::result::Result<Row, tokio_postgres::Error>,
) -> Result<OptionalJsonMap> {
    Ok(row_to_object(row?))
}

fn row_to_object(row: Row) -> OptionalJsonMap {
    row.columns()
        .iter()
        .enumerate()
        .map(|x| row_to_pair(x, &row))
        .collect()
}

fn row_to_pair((index, key): (usize, &Column), row: &Row) -> (String, Option<Value>) {
    use tokio_postgres::types::Type;

    let value = match key.type_() {
        &Type::BOOL => row.get::<_, Option<bool>>(index).map(Value::Bool),
        &Type::INT2 | &Type::INT4 | &Type::INT8 => row.get::<_, Option<i64>>(index).map(Value::Int),
        &Type::FLOAT4 | &Type::FLOAT8 => row.get::<_, Option<f64>>(index).map(Value::Float),
        &Type::TIMESTAMP => row
            .get::<_, Option<PrimitiveDateTime>>(index)
            .map(Value::DateTime),
        &Type::TIMESTAMPTZ => row
            .get::<_, Option<OffsetDateTime>>(index)
            .map(Value::DateTimeTz),
        &Type::UUID => row.get::<_, Option<Uuid>>(index).map(Value::Uuid),
        _other_type => row.get::<_, Option<String>>(index).map(Value::String),
    };

    // let value = row.get(index);

    (key.name().to_string(), value)
}
