pub mod text_partition;

#[cfg(feature = "serde")]
pub mod serde {
    use serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize};

    use chrono::{DateTime, Utc};

    pub mod date_time {
        use super::*;
        use serde::de::Error;

        pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&dt.to_rfc2822())
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let as_string = String::deserialize(deserializer)?;
            let date_time = DateTime::parse_from_rfc2822(&as_string)
                .map_err(|e| D::Error::custom(format!("invalid rfc2822 date time: {}", e)))?;

            Ok(date_time.with_timezone(&Utc))
        }
    }

    pub mod opt_date_time {
        use super::*;

        pub fn serialize<S>(dt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            SerOptDateTime::from(dt).serialize(serializer)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
        where
            D: Deserializer<'de>,
        {
            DeOptDateTime::deserialize(deserializer).map(Into::into)
        }

        #[derive(Deserialize)]
        enum DeOptDateTime {
            Some(#[serde(with = "super::date_time")] DateTime<Utc>),
            None,
        }

        impl Into<Option<DateTime<Utc>>> for DeOptDateTime {
            fn into(self) -> Option<DateTime<Utc>> {
                match self {
                    DeOptDateTime::Some(dt) => Some(dt),
                    DeOptDateTime::None => None,
                }
            }
        }

        #[derive(Serialize)]
        enum SerOptDateTime<'a> {
            Some(#[serde(with = "super::date_time")] &'a DateTime<Utc>),
            None,
        }

        impl<'a> From<&'a Option<DateTime<Utc>>> for SerOptDateTime<'a> {
            fn from(val: &'a Option<DateTime<Utc>>) -> Self {
                match val {
                    Some(ref dt) => SerOptDateTime::Some(dt),
                    None => SerOptDateTime::None,
                }
            }
        }
    }
}
