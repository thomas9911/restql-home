use axum::{
    routing::{get, post},
    Router,
};
use deadpool_postgres::{Config, ManagerConfig, RecyclingMethod, Runtime};
use restql_home::{get_record, insert_record, list_records, AppState};
use tokio_postgres::NoTls;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut cfg = Config::new();
    cfg.dbname = Some("postgres".to_string());
    cfg.user = Some("postgres".to_string());
    cfg.host = Some("localhost".to_string());
    cfg.port = Some(5432);
    cfg.password = Some("example".to_string());
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });
    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();

    // let (client, connection) = tokio_postgres::connect("host=localhost user=postgres password=example", NoTls).await?;

    let shared_state = AppState { pool };

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/:table_name", post(insert_record).get(list_records))
        .route("/:table_name/:record_id", get(get_record))
        .with_state(shared_state);

    axum::Server::bind(&"0.0.0.0:9503".parse().unwrap())
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
