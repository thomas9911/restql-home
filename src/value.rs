use postgres_types::ToSql;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::{iso8601, Iso8601};
use time::{OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

mod datetime_iso8601 {
    use std::marker::PhantomData;

    use serde::de;
    use serde::ser::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use time::error::ComponentRange;
    use time::format_description::well_known::{iso8601, Iso8601};
    use time::format_description::FormatItem;
    use time::macros::format_description;
    use time::{Date, OffsetDateTime, PrimitiveDateTime};

    use super::ISO8601_DATETIME_CFG;

    const PRIMITIVE_DATE_TIME_FORMAT: &[FormatItem<'_>] =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");

    // copypasted from time crate
    pub(super) struct Visitor<T: ?Sized>(pub(super) PhantomData<T>);

    macro_rules! item {
        ($seq:expr, $name:literal) => {
            $seq.next_element()?
                .ok_or_else(|| <A::Error as serde::de::Error>::custom(concat!("expected ", $name)))
        };
    }

    pub(crate) fn into_de_error<E: serde::de::Error>(range: ComponentRange) -> E {
        E::invalid_value(serde::de::Unexpected::Signed(0), &range)
    }

    impl<'a> de::Visitor<'a> for Visitor<Iso8601<ISO8601_DATETIME_CFG>> {
        type Value = PrimitiveDateTime;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a `PrimitiveDateTime`")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<PrimitiveDateTime, E> {
            PrimitiveDateTime::parse(value, &PRIMITIVE_DATE_TIME_FORMAT).map_err(E::custom)
        }

        fn visit_seq<A: de::SeqAccess<'a>>(
            self,
            mut seq: A,
        ) -> Result<PrimitiveDateTime, A::Error> {
            let year = item!(seq, "year")?;
            let ordinal = item!(seq, "day of year")?;
            let hour = item!(seq, "hour")?;
            let minute = item!(seq, "minute")?;
            let second = item!(seq, "second")?;
            let nanosecond = item!(seq, "nanosecond")?;

            Date::from_ordinal_date(year, ordinal)
                .and_then(|date| date.with_hms_nano(hour, minute, second, nanosecond))
                .map_err(into_de_error)
        }
    }

    pub fn serialize<S: Serializer>(
        datetime: &PrimitiveDateTime,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        datetime
            .format(&Iso8601::<ISO8601_DATETIME_CFG>)
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }

    pub fn deserialize<'a, D: Deserializer<'a>>(
        deserializer: D,
    ) -> Result<PrimitiveDateTime, D::Error> {
        deserializer.deserialize_str(Visitor::<Iso8601<ISO8601_DATETIME_CFG>>(PhantomData))
    }
}

pub const ISO8601_DATETIME_CFG: u128 = {
    iso8601::Config::DEFAULT
        .set_formatted_components(iso8601::FormattedComponents::DateTime)
        .encode()
};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Uuid(Uuid),
    #[serde(with = "time::serde::iso8601")]
    DateTimeTz(OffsetDateTime),
    #[serde(with = "datetime_iso8601")]
    DateTime(PrimitiveDateTime),
    String(String),
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
