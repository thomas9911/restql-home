use axum::extract::{Json, Path, State};
use deadpool_postgres::PoolError;
use either::Either;
use std::sync::{Arc, Mutex};

pub mod error;
pub mod methods;
pub mod scripting;
pub mod value;

pub use error::{MyError, Result};
pub use value::Value;

pub type JsonMap = std::collections::HashMap<String, Value>;
pub type OptionalJsonMap = std::collections::HashMap<String, Option<Value>>;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(transparent)]
pub struct InsertBody {
    #[serde(with = "either::serde_untagged")]
    inner: Either<JsonMap, Vec<JsonMap>>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub pool: deadpool_postgres::Pool,
}

pub async fn get_record(
    Path((table_name, record_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<Option<OptionalJsonMap>>> {
    let client = state.pool.get().await?;
    let result = methods::get_record(&client, (table_name, record_id), state).await?;

    Ok(Json(result))
}

#[axum::debug_handler]
pub async fn insert_record(
    Path(table_name): Path<String>,
    State(state): State<AppState>,
    Json(data): Json<InsertBody>,
) -> Result<Json<OptionalJsonMap>> {
    dbg!((&table_name, &data));
    let client = state.pool.get().await?;

    match data.inner {
        Either::Left(data) => {
            let result = methods::insert_record(&client, table_name, data).await?;
            Ok(Json(result))
        }
        Either::Right(data) => todo!(),
    }
}
