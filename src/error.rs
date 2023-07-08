use axum::response::{IntoResponse, Response};
use hyper::StatusCode;

pub type Result<T> = std::result::Result<T, MyError>;

#[derive(Debug)]
// #[error(transparent)]
pub struct MyError {
    pub source: anyhow::Error,
}

impl<E: Into<anyhow::Error>> From<E> for MyError {
    fn from(err: E) -> MyError {
        MyError { source: err.into() }
    }
}

impl IntoResponse for MyError {
    fn into_response(self) -> Response {
        let mut res = self.source.to_string().into_response();
        *res.status_mut() = StatusCode::BAD_REQUEST;
        res
    }
}

impl From<MyError> for mlua::Error {
    fn from(err: MyError) -> mlua::Error {
        mlua::Error::RuntimeError(err.source.to_string())
    }
}
