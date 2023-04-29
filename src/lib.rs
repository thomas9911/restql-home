use axum::extract::{Json, Path, Query, State};
use deadpool_postgres::GenericClient;
use futures_util::stream::StreamExt;
use time::format_description::well_known::{iso8601, Iso8601};
use time::{OffsetDateTime, PrimitiveDateTime};
use tokio_postgres::{Column, Row};
use uuid::Uuid;
// use postgres_types::{ToSql, FromSql};

pub mod error;
pub mod value;

pub use error::MyError;
pub use value::Value;

pub type JsonMap = std::collections::HashMap<String, Value>;
pub type OptionalJsonMap = std::collections::HashMap<String, Option<Value>>;

// #[derive(Debug, serde::Deserialize)]
// #[serde(untagged)]
// pub enum RecordId {
//     Int(i64),
//     Uuid(Uuid),
//     String(String),
// }

// impl ToSql for RecordId {
//     fn to_sql(
//         &self,
//         ty: &tokio_postgres::types::Type,
//         out: &mut tokio_postgres::types::private::BytesMut,
//     ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
//         match self {
//             RecordId::Int(x) => x.to_sql(ty, out),
//             RecordId::Uuid(x) => x.to_sql(ty, out),
//             RecordId::String(x) => x.to_sql(ty, out),
//         }
//     }

//     fn accepts(ty: &tokio_postgres::types::Type) -> bool {
//         i64::accepts(ty) || Uuid::accepts(ty) || String::accepts(ty)
//     }

//     tokio_postgres::types::to_sql_checked!();
// }

#[derive(Debug, Clone)]
pub struct AppState {
    pub pool: deadpool_postgres::Pool,
}

pub async fn get_record(
    Path((table_name, record_id)): Path<(String, Value)>,
    State(state): State<AppState>,
) -> Result<Json<Option<OptionalJsonMap>>, MyError> {
    let client = state.pool.get().await?;
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

    Ok(Json(result))
}

pub async fn insert_record(
    Path(table_name): Path<String>,
    State(state): State<AppState>,
    Json(data): Json<JsonMap>,
) -> Result<Json<OptionalJsonMap>, MyError> {
    dbg!((&table_name, &data));

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

    let client = state.pool.get().await?;
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

    let data = data.ok_or(anyhow::Error::msg("invalid return statement"))?;

    Ok(Json(data))
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
        &Type::BOOL => {
            if let Some(a) = row.get(index) {
                Some(Value::Bool(a))
            } else {
                None
            }
        }
        &Type::INT2 | &Type::INT4 | &Type::INT8 => {
            if let Some(number) = row.get::<_, Option<i64>>(index) {
                Some(Value::Int(number))
            } else {
                None
            }
        }
        &Type::FLOAT4 | &Type::FLOAT8 => {
            if let Some(number) = row.get::<_, Option<f64>>(index) {
                Some(Value::Float(number))
            } else {
                None
            }
        }
        &Type::TIMESTAMP => {
            if let Some(datetime) = row.get::<_, Option<PrimitiveDateTime>>(index) {
                dbg!(&datetime);
                Some(Value::DateTime(datetime))
            } else {
                None
            }
        }
        &Type::TIMESTAMPTZ => {
            if let Some(datetime) = row.get::<_, Option<OffsetDateTime>>(index) {
                Some(Value::DateTimeTz(datetime))
            } else {
                None
            }
        }
        &Type::UUID => {
            if let Some(uuid) = row.get::<_, Option<Uuid>>(index) {
                Some(Value::Uuid(uuid))
            } else {
                None
            }
        }
        _other_type => {
            if let Some(x) = row.get::<_, Option<String>>(index) {
                Some(Value::String(x))
            } else {
                None
            }
        }
    };

    // let value = row.get(index);

    (key.name().to_string(), value)
}
