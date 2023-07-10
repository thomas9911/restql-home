use ::serde::Serialize;
use postgres_types::ToSql;
use sea_schema::sea_query;
use sqlx_core::column::Column;
use sqlx_core::database::{Database, HasValueRef};
use sqlx_core::decode::Decode;
use sqlx_core::from_row::FromRow;
use sqlx_core::postgres::{PgRow, PgValueFormat, PgValueRef, Postgres};
use sqlx_core::row::Row;
use sqlx_core::type_info::TypeInfo;
use sqlx_core::types::Type;
use sqlx_core::value::ValueRef;
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

pub type JsonMap = std::collections::HashMap<String, Value>;
pub type OptionalJsonMap = std::collections::HashMap<String, Option<Value>>;

pub struct OptionalJsonMapWrapper(pub OptionalJsonMap);

#[derive(Debug, Clone, Serialize, PartialEq)]
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
        if value.starts_with('"') && value.ends_with('"') {
            return Value::parse_str(&value[1..(value.len() - 1)]);
        };

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

// impl<'r> Decode<'r, Postgres> for Value {
//     fn decode(
//         value: PgValueRef<'r>,
//     ) -> Result<Value, Box<dyn std::error::Error + 'static + Send + Sync>> {
//         // match value.format() {
//         //     PgValueFormat::Binary => ,
//         //     PgValueFormat::Text => {
//         //         Err("unsupported decode to `&[u8]` of BYTEA in a simple query; use a prepared query or decode to `Vec<u8>`".into())
//         //     }
//         // }

//         match value.type_info().name() {
//             a => panic!("{:?}", a)
//         }
//     }
// }

// impl<'r> Decode<'r, Postgres> for Value {
//     fn decode(
//         value: PgValueRef<'r>,
//     ) -> Result<Value, Box<dyn std::error::Error + 'static + Send + Sync>> {
//         // match value {
//         //     Ok(raw_value) if !raw_value.is_null()=> match raw_value.type_info().name() {
//         //         "REAL" | "FLOAT" | "NUMERIC" | "FLOAT4" | "FLOAT8" | "DOUBLE" =>
//         //             map_serialize::<_, _, f64>(&mut map, key, raw_value),
//         //         "INT" | "INTEGER" | "INT8" | "INT2" | "INT4" | "TINYINT" | "SMALLINT" | "BIGINT" =>
//         //             map_serialize::<_, _, i64>(&mut map, key, raw_value),
//         //         "BOOL" | "BOOLEAN" =>
//         //             map_serialize::<_, _, bool>(&mut map, key, raw_value),
//         //         // Deserialize as a string by default
//         //         _ => map_serialize::<_, _, &str>(&mut map, key, raw_value)
//         //     },
//         //     _ => map.serialize_entry(key, &()) // Serialize null
//         // }?

//         match value.type_info().name() {
//             "BOOL" | "BOOLEAN" => row.get::<_, Option<bool>>(index).map(Value::Bool),
//         "INT2" => row
//             .get::<_, Option<i16>>(index)
//             .map(|x| Value::Int(x as i64)),
//         "INT4" => row
//             .get::<_, Option<i32>>(index)
//             .map(|x| Value::Int(x as i64)),
//         "INT8" => row.get::<_, Option<i64>>(index).map(Value::Int),
//         "FLOAT4" => row
//             .get::<_, Option<f32>>(index)
//             .map(|x| Value::Float(x as f64)),
//         "FLOAT8" => row.get::<_, Option<f64>>(index).map(Value::Float),
//         "TIMESTAMP" => row
//             .get::<_, Option<PrimitiveDateTime>>(index)
//             .map(Value::DateTime),
//         "TIMESTAMPTZ" => row
//             .get::<_, Option<OffsetDateTime>>(index)
//             .map(Value::DateTimeTz),
//         "UUID" => row.get::<_, Option<Uuid>>(index).map(Value::Uuid),
//         _other_type => row.get::<_, Option<String>>(index).map(Value::String),
//         }
//     }
// }

// impl<'r, DB: Database> Decode<'r, DB> for Value
// where
//     // we want to delegate some of the work to string decoding so let's make sure strings
//     // are supported by the database
//     &'r str: Decode<'r, DB>
// {
//     fn decode(
//         value: <DB as HasValueRef<'r>>::ValueRef,
//     ) -> Result<Value, Box<dyn std::error::Error + 'static + Send + Sync>> {
//         // todo!("not implemented for not postgres")
//         match value.type_info().name() {
//             a => panic!("{:?}", a)
//         }
//     }
// }

impl<DB: Database> Type<DB> for Value {
    fn type_info() -> <DB as Database>::TypeInfo {
        todo!()
    }
}

impl FromRow<'_, PgRow> for OptionalJsonMapWrapper {
    fn from_row(row: &PgRow) -> sqlx_core::error::Result<OptionalJsonMapWrapper> {
        // Ok(Self {
        //     bar: MyCustomType {
        //         custom: row.try_get("custom")?
        //     }
        // })

        // panic!("{:?}", row);
        let mut data = OptionalJsonMap::new();
        for column in row.columns() {
            let column_name = column.name();
            let index = column.ordinal();
            let Ok(raw_value) = row.try_get_raw(column.ordinal()) else {continue;};
            let value = match raw_value.type_info().name() {
                "BOOL" | "BOOLEAN" => row.get::<Option<bool>, _>(index).map(Value::Bool),
                "INT2" => row
                    .get::<Option<i16>, _>(index)
                    .map(|x| Value::Int(x as i64)),
                "INT4" => row
                    .get::<Option<i32>, _>(index)
                    .map(|x| Value::Int(x as i64)),
                "INT8" => row.get::<Option<i64>, _>(index).map(Value::Int),
                "FLOAT4" => row
                    .get::<Option<f32>, _>(index)
                    .map(|x| Value::Float(x as f64)),
                "FLOAT8" => row.get::<Option<f64>, _>(index).map(Value::Float),
                "TIMESTAMP" => row
                    .get::<Option<PrimitiveDateTime>, _>(index)
                    .map(Value::DateTime),
                "TIMESTAMPTZ" => row
                    .get::<Option<OffsetDateTime>, _>(index)
                    .map(Value::DateTimeTz),
                "UUID" => row.get::<Option<Uuid>, _>(index).map(Value::Uuid),
                _ if raw_value.is_null() => None,
                _other_type => row.get::<Option<String>, _>(index).map(Value::String),
            };

            data.insert(column_name.to_string(), value);
        }

        todo!()
    }
}

impl From<Value> for sea_query::Value {
    fn from(value: Value) -> Self {
        match value {
            Value::Bool(b) => sea_query::Value::Bool(Some(b)),
            Value::DateTime(dt) => sea_query::Value::String(Some(Box::new(dt.to_string()))),
            Value::DateTimeTz(dt) => sea_query::Value::String(Some(Box::new(dt.to_string()))),
            Value::Float(f) => sea_query::Value::Double(Some(f)),
            Value::Int(i) => sea_query::Value::BigInt(Some(i)),
            Value::String(s) => sea_query::Value::String(Some(Box::new(s))),
            Value::Uuid(u) => sea_query::Value::String(Some(Box::new(u.to_string()))),
        }
    }
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
