use crate::methods;
use crate::AppState;
use crate::JsonMap;
use crate::{MyError, Result};

use futures_util::FutureExt;
use mlua::SerializeOptions;
use mlua::UserData;
use mlua::UserDataMethods;
use mlua::{DeserializeOptions, Lua, LuaOptions, LuaSerdeExt, StdLib};
use sqlx_core::connection::Connection;
use std::sync::Arc;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{mpsc, oneshot};

// only usefull for not luau
// fn default_opts() -> StdLib {
//     StdLib::TABLE | StdLib::STRING | StdLib::BIT | StdLib::MATH
// }

#[derive(Clone)]
struct TransactionAppState {
    sender: mpsc::Sender<(Command, oneshot::Sender<mlua::Result<serde_json::Value>>)>,
}

// impl<'a> UserData for TransactionAppState {}

// impl<'a> UserData for TransactionAppState<'a> {

fn to_lua_error(e: Arc<dyn std::error::Error + Send + Sync>) -> mlua::Error {
    mlua::Error::ExternalError(e)
}

impl UserData for TransactionAppState {
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

                match rx.await.map_err(|e| to_lua_error(Arc::new(e)))? {
                    Ok(x) => {
                        let options = SerializeOptions::new();
                        lua.to_value_with(&x, options)
                    }
                    Err(e) => Err(e),
                }
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

                match rx.await.map_err(|e| to_lua_error(Arc::new(e)))? {
                    Ok(x) => {
                        let options = SerializeOptions::new();
                        lua.to_value_with(&x, options)
                    }
                    Err(e) => Err(e),
                }
            },
        );

        methods.add_async_method("rollback", |lua, this, value: mlua::Value| async move {
            let options = DeserializeOptions::new();
            let user_output: serde_json::Value = lua.from_value_with(value, options)?;

            let (tx, rx) = oneshot::channel();
            this.sender
                .send((Command::Rollback(user_output), tx))
                .await
                .unwrap();

            match rx.await.map_err(|e| to_lua_error(Arc::new(e)))? {
                Ok(x) => {
                    let options = SerializeOptions::new();
                    lua.to_value_with(&x, options)
                }
                Err(e) => Err(e),
            }
        });
    }
}

fn lua_print(_lua: &Lua, _asdf: mlua::Value) -> mlua::Result<()> {
    // dbg!(asdf);
    Ok(())
}

pub async fn build_runtime<'a>(
    _app_state: AppState,
    sender: mpsc::Sender<(Command, oneshot::Sender<mlua::Result<serde_json::Value>>)>,
) -> Result<Lua> {
    let runtime = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())?;
    // let debug = runtime
    //     .create_function(|_, value: mlua::Value| {
    //         dbg!(&value);
    //         Ok(())
    //     })
    //     .unwrap();

    {
        let globals = runtime.globals();
        // override / mock the print function
        globals.set("print", runtime.create_function(lua_print)?)?;
        // globals.set("debug", debug)?;
        globals.set("transaction", TransactionAppState { sender })?;
        // globals.set("transaction", app_state)?;
    }
    runtime.sandbox(true)?;

    Ok(runtime)
}

// fn value_from_option(value: Option<serde_json::Value>) -> serde_json::Value {
//     match value {
//         Some(val) => val,
//         None => serde_json::Value::Null,
//     }
// }

fn convert_lua_value<'lua>(lua: &'lua Lua, value: mlua::Value<'lua>) -> Result<mlua::Value<'lua>> {
    match value {
        mlua::Value::Error(e) => {
            let string = lua.create_string(&e.to_string())?;
            Ok(mlua::Value::String(string))
        }
        value => Ok(value),
    }
}

async fn something(
    app_state: AppState,
    script: &str,
    input: &serde_json::Value,
    sender: mpsc::Sender<(Command, oneshot::Sender<mlua::Result<serde_json::Value>>)>,
) -> Result<serde_json::Value> {
    let runtime = build_runtime(app_state, sender).await?;
    let options = SerializeOptions::new();
    let user_input = runtime.to_value_with(input, options)?;
    runtime.globals().set("input", user_input)?;

    let output = runtime.load(script).eval_async().await?;
    let output = convert_lua_value(&runtime, output)?;
    let options = DeserializeOptions::new().deny_unsupported_types(false);
    let result: serde_json::Value = runtime.from_value_with(output, options)?;

    Ok(result)
}

fn result_flatten(
    res: std::result::Result<mlua::Result<serde_json::Value>, RecvError>,
) -> Result<serde_json::Value> {
    match res {
        Ok(x) => x.map_err(MyError::from),
        Err(e) => Err(e.into()),
    }
}

async fn xd(
    res: Result<serde_json::Value>,
    cmd_tx: mpsc::Sender<(Command, oneshot::Sender<mlua::Result<serde_json::Value>>)>,
) -> Result<serde_json::Value> {
    match res {
        Ok(success) => {
            let (tx, rx) = oneshot::channel();
            cmd_tx.send((Command::Done(success), tx)).await.ok();
            return result_flatten(rx.await);
        }
        Err(e) => {
            let (tx, rx) = oneshot::channel();
            cmd_tx.send((Command::Error(e), tx)).await.ok();
            return result_flatten(rx.await);
        }
    };
}

#[derive(Debug)]
pub enum Command {
    Get(String, String),
    Create(String, JsonMap),
    Rollback(serde_json::Value),
    Error(MyError),
    Done(serde_json::Value),
}

pub enum CommitOrRollback {
    Commit(serde_json::Value),
    Rollback(serde_json::Value),
    RollbackError(MyError),
}

pub async fn transaction(
    app_state: AppState,
    script: &str,
    input: &serde_json::Value,
) -> Result<serde_json::Value> {
    let (cmd_tx, mut cmd_rx) =
        mpsc::channel::<(Command, oneshot::Sender<mlua::Result<serde_json::Value>>)>(100);

    let app_state_transaction = app_state.clone();
    let cmd_sender = cmd_tx.clone();
    let transaction_join = tokio::spawn(async move {
        let mut client = app_state_transaction.clone().pool.acquire().await?;
        let mut transaction = client.begin().await?;

        let app_state_copy = app_state_transaction.clone();
        let mut result_container = None;
        while let Some((command, responder)) = cmd_rx.recv().await {
            match command {
                Command::Get(table, record_id) => {
                    let response = methods::get_record(
                        &mut transaction,
                        (table, record_id),
                        app_state_copy.clone(),
                    )
                    .await
                    .map(|data| {
                        serde_json::to_value(data).expect("value cannot be converted to json")
                    })
                    .map_err(|e| e.into());

                    responder.send(response).unwrap();
                }
                Command::Create(table, data) => {
                    let response = methods::insert_record(&mut transaction, table, data)
                        .await
                        .map(|data| {
                            serde_json::to_value(data).expect("value cannot be converted to json")
                        })
                        .map_err(|e| e.into());

                    responder.send(response).unwrap();
                }
                Command::Rollback(value) => {
                    result_container = Some(CommitOrRollback::Rollback(value));
                    break;
                }
                Command::Done(value) => {
                    result_container = Some(CommitOrRollback::Commit(value));
                    break;
                }
                Command::Error(value) => {
                    result_container = Some(CommitOrRollback::RollbackError(value));
                    break;
                }
            }
        }

        match result_container {
            Some(CommitOrRollback::Commit(value)) => {
                if let Err(e) = transaction.commit().await {
                    eprintln!("{:?}", e);
                }
                Ok(value)
            }
            Some(CommitOrRollback::RollbackError(value)) => {
                if let Err(e) = transaction.rollback().await {
                    eprintln!("{:?}", e);
                }
                Err(value)
            }
            Some(CommitOrRollback::Rollback(value)) => {
                if let Err(e) = transaction.rollback().await {
                    eprintln!("{:?}", e);
                }
                Ok(value)
            }
            _ => panic!("invalid "),
        }
    });

    let (left, right) = tokio::join!(
        something(app_state.clone(), script, input, cmd_sender.clone())
            .then(move |res| async { xd(res, cmd_tx) })
            .await,
        transaction_join
    );

    // returns user script error
    let out = right??;
    left.ok();

    Ok(out)
}

#[cfg(test)]
async fn test_app_state() -> AppState {
    use sqlx_core::{pool::PoolOptions, postgres::PgConnectOptions};

    // let mut cfg = Config::new();
    // cfg.dbname = Some("postgres".to_string());
    // cfg.user = Some("postgres".to_string());
    // cfg.host = Some("localhost".to_string());
    // cfg.port = Some(5432);
    // cfg.password = Some("example".to_string());
    // cfg.manager = Some(ManagerConfig {
    //     recycling_method: RecyclingMethod::Fast,
    // });
    // let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();
    let connect_opts = PgConnectOptions::new()
        .host("localhost")
        .port(5432)
        .database("postgres")
        .username("postgres")
        .password("example");

    let pool_opts = PoolOptions::new();
    let pool = pool_opts.connect_with(connect_opts).await.unwrap();

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
        transaction(test_app_state().await, script, &serde_json::Value::Null)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn transaction_test() {
    let script = r#"
    -- data = {email="example@example.com", username="example", password="example", created_on="2020-04-12T12:23:34"}
    out = transaction:create("accounts", input)
    transaction:rollback(out) 
    "#;

    let data = serde_json::json!({
        "email": "example1234@example.com",
        "username": "example1234",
        "password": "example",
        "created_on": "2020-04-12T12:23:34"
    });

    let response = transaction(test_app_state().await, script, &data)
        .await
        .unwrap();

    assert_eq!(response["email"], "example1234@example.com");
}

#[tokio::test]
async fn transaction_test_rollback() {
    let script = r#"
    out = transaction:create("accounts", input)
    out = transaction:get("accounts", out["id"])
    transaction:rollback("rolled back")
    -- does not get executed because rollback returns
    out = transaction:create("accounts", input)
    return out
    "#;

    let data = serde_json::json!({
        "email": "example1235@example.com",
        "username": "example1235",
        "password": "example",
        "created_on": "2020-04-12T12:23:34"
    });

    let response = transaction(test_app_state().await, script, &data)
        .await
        .unwrap();

    assert_eq!(response, "rolled back");
}

#[tokio::test]
async fn transaction_test_uniqueness_error() {
    let script = r#"
    local status, err = pcall(function () transaction:create("accounts", input) end)
    local status, err = pcall(function () transaction:create("accounts", input) end)
    if not status and tostring(err):find("unique constraint") then
        return err
    else
        error(err)
    end
    "#;

    let data = serde_json::json!({
        "email": "example1236@example.com",
        "username": "example1236",
        "password": "example",
        "created_on": "2020-04-12T12:23:34"
    });

    let response = transaction(test_app_state().await, script, &data)
        .await
        .unwrap();
    let response = response.as_str().unwrap();

    assert!(response.contains("caused by: runtime error: db error: ERROR: duplicate key value violates unique constraint \"accounts_username_key\"\nDETAIL: Key (username)=(example1236) already exists."));
}

#[tokio::test]
async fn transaction_unsafe_lua_io() {
    let script = r#"
    io.popen("echo 'test'")
    return "test"
    "#;

    assert!(
        transaction(test_app_state().await, script, &serde_json::Value::Null)
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
        transaction(test_app_state().await, script, &serde_json::Value::Null)
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
        transaction(test_app_state().await, script, &serde_json::Value::Null)
            .await
            .is_err()
    );
}
