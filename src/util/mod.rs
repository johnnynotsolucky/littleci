use chrono::{NaiveDateTime, Utc};
use serde::Serializer;

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub fn serialize_date<S>(dt: &NaiveDateTime, s: S) -> Result<S::Ok, S::Error>
where
	S: Serializer,
{
	let formatted = format!("{}", dt.format(DATETIME_FORMAT));
	s.serialize_str(&formatted)
}

pub fn utc_now() -> NaiveDateTime {
	Utc::now().naive_utc()
}
