use std::marker::PhantomData;

use serde::de;
use serde::ser::Error as _;
use serde::{Deserializer, Serialize, Serializer};
use time::error::ComponentRange;
use time::format_description::well_known::Iso8601;
use time::format_description::FormatItem;
use time::macros::format_description;
use time::{Date, PrimitiveDateTime};

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

pub fn parse_datetime(value: &str) -> std::result::Result<PrimitiveDateTime, time::error::Parse> {
    PrimitiveDateTime::parse(value, &PRIMITIVE_DATE_TIME_FORMAT)
}

impl<'a> de::Visitor<'a> for Visitor<Iso8601<ISO8601_DATETIME_CFG>> {
    type Value = PrimitiveDateTime;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a `PrimitiveDateTime`")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<PrimitiveDateTime, E> {
        parse_datetime(value).map_err(E::custom)
    }

    fn visit_seq<A: de::SeqAccess<'a>>(self, mut seq: A) -> Result<PrimitiveDateTime, A::Error> {
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
