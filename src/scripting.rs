use std::collections::HashMap;
use std::sync::Mutex;

use crate::methods;
use crate::AppState;
use crate::JsonMap;
use crate::{MyError, Result};

use deadpool_postgres::{Client, Transaction};
use either::Either;
use either::Either::Left;
use futures_util::FutureExt;
use hyper::client;
use mlua::Function;
use mlua::SerializeOptions;
use mlua::UserData;
use mlua::UserDataMethods;
use mlua::{DeserializeOptions, Lua, LuaOptions, LuaSerdeExt, StdLib};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

// only usefull for not luau
// fn default_opts() -> StdLib {
//     StdLib::TABLE | StdLib::STRING | StdLib::BIT | StdLib::MATH
// }

#[derive(Clone)]
struct TransactionAppState {
    sender: mpsc::Sender<(Command, oneshot::Sender<serde_json::Value>)>,
}

// impl<'a> UserData for TransactionAppState {}

// impl<'a> UserData for TransactionAppState<'a> {
impl<'a> UserData for TransactionAppState {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_async_method(
            "create",
            |lua, this, (table, value): (String, mlua::Value)| async move {
                // dbg!(&value);

                let options = DeserializeOptions::new();
                let user_input: JsonMap = lua.from_value_with(value, options)?;
                // let response = methods::insert_record(table, this, user_input).await?;
                // dbg!(&response);
                let (tx, rx) = oneshot::channel();
                this.sender
                    .send((Command::Create(table, user_input), tx))
                    .await
                    .unwrap();

                let response = rx.await.unwrap();

                let options = SerializeOptions::new();
                lua.to_value_with(&response, options)
            },
        );

        methods.add_async_method(
            "get",
            |lua, this, (table, record_id): (String, String)| async move {
                let (tx, rx) = oneshot::channel();
                this.sender
                    .send((Command::Get(table, record_id), tx))
                    .await
                    .unwrap();

                let response = rx.await.unwrap();

                // let mut conn = this.pool.get().await.unwrap();
                // let transaction = conn.transaction().await.unwrap();

                // let response = methods::get_record(transaction, (table, record_id), this).await?;
                let options = SerializeOptions::new();
                lua.to_value_with(&response, options)
            },
        );
    }
}

fn lua_print(_lua: &Lua, _asdf: mlua::Value) -> mlua::Result<()> {
    // dbg!(asdf);
    Ok(())
}

pub async fn build_runtime<'a>(
    app_state: AppState,
    sender: mpsc::Sender<(Command, oneshot::Sender<serde_json::Value>)>,
) -> Result<Lua> {
    let runtime = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())?;
    {
        let globals = runtime.globals();
        // override / mock the print function
        globals.set("print", runtime.create_function(lua_print)?)?;
        globals.set("transaction", TransactionAppState { sender })?;
        // globals.set("transaction", app_state)?;
    }
    runtime.sandbox(true)?;

    return Ok(runtime);
}

async fn something(
    app_state: AppState,
    script: &str,
    input: &serde_json::Value,
    sender: mpsc::Sender<(Command, oneshot::Sender<serde_json::Value>)>,
) -> Result<serde_json::Value> {
    let runtime = build_runtime(app_state, sender).await?;
    let options = SerializeOptions::new();
    let user_input = runtime.to_value_with(input, options)?;
    runtime.globals().set("input", user_input)?;

    let output = runtime.load(script).eval_async().await?;
    let options = DeserializeOptions::new().deny_unsupported_types(false);
    let result: serde_json::Value = runtime.from_value_with(output, options)?;

    Ok(result)
}

async fn xd(
    res: Result<serde_json::Value>,
    cmd_tx: mpsc::Sender<(Command, oneshot::Sender<serde_json::Value>)>,
) -> Result<serde_json::Value> {
    match res {
        Ok(success) => {
            let (tx, rx) = oneshot::channel();
            cmd_tx.send((Command::Done(success), tx)).await.unwrap();
            return rx.await.map_err(|e| e.into());
        }
        Err(e) => {
            let (tx, rx) = oneshot::channel();
            cmd_tx.send((Command::Error(e), tx)).await.unwrap();
            return rx.await.map_err(|e| e.into());
        }
    };
}

#[derive(Debug)]
pub enum Command {
    Get(String, String),
    Create(String, JsonMap),
    Error(MyError),
    Done(serde_json::Value),
}

pub async fn transaction(
    app_state: AppState,
    script: &str,
    input: &serde_json::Value,
) -> Result<serde_json::Value> {
    // if app_state.client.is_none() {
    //     let client = app_state.pool.get().await?;
    //     app_state.client = Some(Arc::new(Mutex::new(client)));
    // }

    // let transaction = client.transaction().await.unwrap();

    // dbg!(&result);
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<(Command, oneshot::Sender<serde_json::Value>)>(100);

    let app_state_transaction = app_state.clone();
    let cmd_sender = cmd_tx.clone();
    let transaction_join = tokio::spawn(async move {
        let mut client = app_state_transaction.clone().pool.get().await?;
        let transaction = client.transaction().await?;

        let app_state_copy = app_state_transaction.clone();
        let mut result_container = None;
        while let Some((command, responder)) = cmd_rx.recv().await {
            match command {
                Command::Get(table, record_id) => {
                    let response = methods::get_record(
                        &transaction,
                        (table, record_id),
                        app_state_copy.clone(),
                    )
                    .await?;

                    responder
                        .send(
                            serde_json::to_value(&response)
                                .expect("value cannot be converted to json"),
                        )
                        .unwrap();
                }
                Command::Create(table, data) => {
                    let response = methods::insert_record(&transaction, table, data).await?;

                    responder
                        .send(
                            serde_json::to_value(&response)
                                .expect("value cannot be converted to json"),
                        )
                        .unwrap();
                }
                Command::Done(value) => {
                    result_container = Some(Either::Left(value));
                    break;
                }
                Command::Error(value) => {
                    result_container = Some(Either::Right(value));
                    break;
                }
            }
        }

        // if let Some(err) = error_container {
        //     transaction.rollback().await?;
        //     Err(err)
        // } else {
        //     Result::Ok(())
        // }
        match result_container {
            Some(Either::Left(value)) => {
                dbg!(transaction.commit().await);
                Ok(value)
            }
            Some(Either::Right(value)) => {
                dbg!(transaction.rollback().await);
                Err(value)
            }
            _ => panic!("invalid "),
        }
    });

    // something(app_state, script, input, cmd_tx).await?;
    // transaction_join.await??;

    // something(app_state, script, input, cmd_sender).then( async {
    //     match res {
    //         Ok(success) => {
    //             let (tx, rx) = oneshot::channel();
    //             cmd_tx.send((Command::Done(success), tx)).await.unwrap();
    //             return rx.await.map_err(|e| e.into());
    //         },
    //         Err(e) => {
    //             let (tx, rx) = oneshot::channel();
    //             cmd_tx.send((Command::Error(e), tx)).await.unwrap();
    //             return rx.await.map_err(|e| e.into());
    //         },
    //     };
    // }).await;

    // something(app_state, script, input, cmd_sender)
    //     .then(move |res| async { xd(res, cmd_tx) })
    //     .await;

    // tokio::select! {
    //     res = something(app_state, script, input, cmd_sender) => {
    //         match res {
    //             Ok(success) => {
    //                 let (tx, rx) = oneshot::channel();
    //                 cmd_tx.send((Command::Done(success), tx)).await.unwrap();
    //                 return rx.await.map_err(|e| e.into());
    //             },
    //             Err(e) => {
    //                 let (tx, rx) = oneshot::channel();
    //                 cmd_tx.send((Command::Error(e), tx)).await.unwrap();
    //                 return rx.await.map_err(|e| e.into());
    //             },
    //         };
    //     },
    //     res = transaction_join => {
    //         res??;
    //     },
    // };

    let (left, right) = tokio::join!(
        something(app_state.clone(), script, input, cmd_sender.clone())
            .then(move |res| async { xd(res, cmd_tx) })
            .await,
        transaction_join
    );

    right??;

    left
}

#[cfg(test)]
fn test_app_state() -> AppState {
    use deadpool_postgres::{Config, ManagerConfig, RecyclingMethod, Runtime};
    use tokio_postgres::NoTls;

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

    let pool = pool;
    let app_state = AppState { pool };
    app_state
}

#[tokio::test]
async fn transaction_test_just_lua() {
    // if this test prints to commandline print is not overwritten

    let script = r#"
    
    print(math.log(2))

    return "test"
    "#;

    assert_eq!(
        serde_json::Value::String("test".to_string()),
        transaction(test_app_state(), script, &serde_json::Value::Null)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn transaction_test() {
    let script = r#"
    -- data = {email="example@example.com", username="example", password="example", created_on="2020-04-12T12:23:34"}
    return transaction:create("accounts", input)
    "#;

    let data = serde_json::json!({
        "email": "example1234@example.com",
        "username": "example1234",
        "password": "example",
        "created_on": "2020-04-12T12:23:34"
    });

    let response = transaction(test_app_state(), script, &data).await.unwrap();

    assert_eq!(response["email"], "example1234@example.com");
}

#[tokio::test]
async fn transaction_unsafe_lua_io() {
    let script = r#"
    io.popen("echo 'test'")
    return "test"
    "#;

    assert!(
        transaction(test_app_state(), script, &serde_json::Value::Null)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn transaction_unsafe_lua_os() {
    let script = r#"
    os.execute("echo 'test'")
    return "test"
    "#;

    assert!(
        transaction(test_app_state(), script, &serde_json::Value::Null)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn transaction_unsafe_lua_debug() {
    let script = r#"
    debug.getlocal()
    return "test"
    "#;

    assert!(
        transaction(test_app_state(), script, &serde_json::Value::Null)
            .await
            .is_err()
    );
}
