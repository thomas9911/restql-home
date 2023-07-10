use axum::extract::{Json, Path, RawQuery, State};
use either::Either;
use postgrest_query_parser::{Ast, Lexer};
pub mod error;
pub mod methods;
pub mod scripting;
pub mod value;

pub use error::{MyError, Result};
pub use value::Value;
pub use value::{JsonMap, OptionalJsonMap};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(transparent)]
pub struct InsertBody {
    #[serde(with = "either::serde_untagged")]
    inner: Either<JsonMap, Vec<JsonMap>>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    // pub pool: deadpool_postgres::Pool,
    pub pool: sqlx_core::pool::Pool<sqlx_core::postgres::Postgres>,
}

pub async fn get_record(
    Path((table_name, record_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<Option<OptionalJsonMap>>> {
    // let client = state.pool.get().await?;
    let mut client = state.pool.acquire().await?;
    let result = methods::get_record(&mut client, (table_name, record_id), state).await?;

    Ok(Json(result))
}

#[axum::debug_handler]
pub async fn list_records(
    Path(table_name): Path<String>,
    params: RawQuery,
    State(state): State<AppState>,
) -> Result<Json<Vec<OptionalJsonMap>>> {
    let params = if let Some(params) = params.0 {
        let lexer = Lexer::new(params.chars());
        Ast::from_lexer(&params, lexer)?
    } else {
        Ast::default()
    };

    // let client = state.pool.get().await?;
    let mut client = state.pool.acquire().await?;
    let result = methods::list_records(&mut client, table_name, params, state).await?;

    Ok(Json(result))
}

#[axum::debug_handler]
pub async fn insert_record(
    Path(table_name): Path<String>,
    State(state): State<AppState>,
    Json(data): Json<InsertBody>,
) -> Result<Json<OptionalJsonMap>> {
    dbg!((&table_name, &data));
    let mut client = state.pool.acquire().await?;

    match data.inner {
        Either::Left(data) => {
            let result = methods::insert_record(&mut client, table_name, data).await?;
            Ok(Json(result))
        }
        Either::Right(_data) => todo!(),
    }
}
