use tokio_postgres::{Client, NoTls};

use futures_util::Future;
use libtest_mimic::{Arguments, Conclusion, Failed, Trial};
use std::{process::Stdio, sync::Arc};
use tokio::{sync::Notify, task::JoinHandle};

const SETUP_SQL: &str = include_str!("setup.sql");

struct SetupState {
    client: Client,
    killer_notify: Arc<Notify>,
    binary_handle: JoinHandle<()>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Arguments::from_args();

    let teststate = setup().await?;

    let conclusion = inner(args).await?;

    teardown(teststate).await?;

    conclusion.exit();
}

async fn setup() -> anyhow::Result<SetupState> {
    let (client, connection) = tokio_postgres::connect(
        "host=localhost port=5432 user=postgres password=example dbname=postgres",
        NoTls,
    )
    .await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    client.batch_execute(SETUP_SQL).await?;

    let notify = Arc::new(Notify::new());
    let notify2 = notify.clone();

    let notifier = notify.notified();

    let killer_notify = Arc::new(Notify::new());
    let killer_notify2 = killer_notify.clone();

    let binary_handle = tokio::spawn(async move {
        let cmd = escargot::CargoBuild::new()
            .bin("restql-at-home")
            .run()
            .unwrap();

        let mut child = cmd
            .command()
            .stdout(Stdio::null())
            // .stdout(Stdio::inherit())
            .spawn()
            .unwrap();
        notify2.notify_waiters();

        killer_notify2.notified().await;
        child.kill().unwrap();
    });

    notifier.await;

    Ok(SetupState {
        client,
        killer_notify,
        binary_handle,
    })
}

async fn teardown(teststate: SetupState) -> anyhow::Result<()> {
    teststate.killer_notify.notify_one();
    teststate.binary_handle.await?;

    Ok(())
}

async fn inner(args: Arguments) -> anyhow::Result<Conclusion> {
    let tests = vec![
        Trial::test("insert_data", || trialing(insert_data())),
        Trial::test("get string id", || trialing(get_string_id())),
        ];

    Ok(libtest_mimic::run(&args, tests))
}

fn trialing<F: Future>(future: F) -> Result<(), Failed> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(future);

    Ok(())
}

async fn insert_data() {
    let data = serde_json::json!({"email": "example@example.com", "username": "example", "password": "example", "created_on": "2020-04-12T12:23:34"});

    let client = reqwest::Client::new();
    let response = client
        .post("http://localhost:9503/accounts")
        .json(&data)
        .send()
        .await
        .unwrap();

    let data: serde_json::Map<String, serde_json::Value> = response.json().await.unwrap();

    assert_eq!(data["username"], "example");
    assert_eq!(data["password"], "example");
    assert_eq!(data["email"], "example@example.com");

    let id = &data["id"];

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://localhost:9503/accounts/{id}"))
        .send()
        .await
        .unwrap();

    let get_data: serde_json::Map<String, serde_json::Value> = response.json().await.unwrap();

    // assert post and get results are the same
    assert_eq!(data, get_data);
}


async fn get_string_id() {
    let id = "ID-12345";
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://localhost:9503/items/{id}"))
        .send()
        .await
        .unwrap();

    let data: serde_json::Map<String, serde_json::Value> = response.json().await.unwrap();

    assert_eq!(data["id"], id);
    assert_eq!(data["description"], "This is a nice object");
}
