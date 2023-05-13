use ::serde::{Deserialize, Serialize};
use postgres_types::ToSql;
use time::format_description::well_known::{iso8601, Iso8601};
use time::{OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

use self::datetime_iso8601::parse_datetime;

pub mod datetime_iso8601;
mod serde;

pub const ISO8601_DATETIME_CFG: u128 = {
    iso8601::Config::DEFAULT
        .set_formatted_components(iso8601::FormattedComponents::DateTime)
        .encode()
};

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Uuid(Uuid),
    Int(i64),
    Float(f64),
    #[serde(with = "time::serde::iso8601")]
    DateTimeTz(OffsetDateTime),
    #[serde(with = "datetime_iso8601")]
    DateTime(PrimitiveDateTime),
    String(String),
}

impl Value {
    pub fn parse_str(value: &str) -> Value {
        if let Ok(uuid) = Uuid::try_parse(value) {
            return Value::Uuid(uuid);
        };

        if let Ok(dt) = parse_datetime(value) {
            return Value::DateTime(dt);
        };

        if let Ok(dt) = OffsetDateTime::parse(value, &Iso8601::DEFAULT) {
            return Value::DateTimeTz(dt);
        };

        Value::String(value.to_owned())
    }
}

impl ToSql for Value {
    fn to_sql(
        &self,
        ty: &tokio_postgres::types::Type,
        out: &mut tokio_postgres::types::private::BytesMut,
    ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match self {
            Value::Int(x) => x.to_sql(ty, out),
            Value::Float(x) => x.to_sql(ty, out),
            Value::Bool(x) => x.to_sql(ty, out),
            Value::Uuid(x) => x.to_sql(ty, out),
            Value::DateTimeTz(x) => x.to_sql(ty, out),
            Value::DateTime(x) => x.to_sql(ty, out),
            Value::String(x) => x.to_sql(ty, out),
        }
    }

    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        i64::accepts(ty)
            || f64::accepts(ty)
            || bool::accepts(ty)
            || Uuid::accepts(ty)
            || OffsetDateTime::accepts(ty)
            || PrimitiveDateTime::accepts(ty)
            || String::accepts(ty)
    }

    tokio_postgres::types::to_sql_checked!();
}

#[test]
fn value_test_int_1() {
    let data = serde_json::from_str("1").unwrap();
    let out: Value = serde_json::from_value(data).unwrap();
    assert_eq!(out, Value::Int(1))
}

#[test]
fn value_test_int_2() {
    let data = serde_json::from_str("-152").unwrap();
    let out: Value = serde_json::from_value(data).unwrap();
    assert_eq!(out, Value::Int(-152))
}

#[test]
fn value_test_float() {
    let data = serde_json::from_str("512.255").unwrap();
    let out: Value = serde_json::from_value(data).unwrap();
    assert_eq!(out, Value::Float(512.255))
}

#[test]
fn value_test_true() {
    let data = serde_json::from_str("true").unwrap();
    let out: Value = serde_json::from_value(data).unwrap();
    assert_eq!(out, Value::Bool(true))
}

#[test]
fn value_test_uuid() {
    let data = serde_json::from_str("\"89592c86-f85d-4527-bdb9-4c3f5dd63f2d\"").unwrap();
    let out: Value = serde_json::from_value(data).unwrap();
    assert_eq!(
        out,
        Value::Uuid("89592c86-f85d-4527-bdb9-4c3f5dd63f2d".parse().unwrap())
    )
}

#[test]
fn value_test_datetime() {
    use time::macros::datetime;

    let data = serde_json::from_str("\"2020-01-01T12:00:00\"").unwrap();
    let out: Value = serde_json::from_value(data).unwrap();

    let x = datetime!(2020-01-01 12:00:00);
    assert_eq!(out, Value::DateTime(x))
}

#[test]
fn value_test_datetime_tz() {
    let data = serde_json::from_str("\"2020-01-01T12:00:00Z\"").unwrap();
    let out: Value = serde_json::from_value(data).unwrap();

    let x = OffsetDateTime::from_unix_timestamp(1577880000).unwrap();
    assert_eq!(out, Value::DateTimeTz(x))
}

#[test]
fn value_test_string() {
    let data = serde_json::from_str("\"testing\"").unwrap();
    let out: Value = serde_json::from_value(data).unwrap();
    assert_eq!(out, Value::String("testing".to_string()))
}
