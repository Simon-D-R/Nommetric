//! The stored record and the read-time version checking.

use serde::Deserialize as _;

pub const SCHEMA_VERSION: u32 = 1;
pub struct RatingScale {
	pub min: u8,
	pub max: u8,
	pub step: u8,
}

impl RatingScale {
	pub fn contains(&self, score: u8) -> bool {
		score >= self.min && score <= self.max
	}
}

pub const RATING_SCALE: RatingScale = RatingScale {
	min: 1,
	max: 10,
	step: 1,
};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Rating {
	pub schema_version: u32,

	pub score: Option<u8>,
	pub comment: String,

	pub dish_name: String,
	pub dish_lookup_key: String,
	pub item_id: Option<String>,
	pub category_id: String,
	pub restaurant_slug: String,

	pub created_at_ms: f64,
	pub updated_at_ms: f64,
}

impl Rating {
	pub fn is_empty(&self) -> bool {
		self.score.is_none() && self.comment.trim().is_empty()
	}

	pub fn load_value(value: &serde_json::Value) -> Result<Self, String> {
		let schema_version = value
			.get("schema_version")
			.and_then(serde_json::Value::as_u64)
			.ok_or("no readable `schema_version`")?;

		if schema_version != u64::from(SCHEMA_VERSION) {
			return Err(format!("incompatible schema_version: {schema_version}"));
		}

		Rating::deserialize(value).map_err(|err| err.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::{Rating, SCHEMA_VERSION};

	fn dummy_rating(score: Option<u8>, comment: &str) -> Rating {
		Rating {
			schema_version: SCHEMA_VERSION,
			score,
			comment: comment.to_string(),
			dish_name: "Lasagne".to_string(),
			dish_lookup_key: "lasagne".to_string(),
			item_id: Some("997582203".to_string()),
			category_id: "133151720".to_string(),
			restaurant_slug: "de-kastart-bvba".to_string(),
			created_at_ms: 1_787_839_453_000.0,
			updated_at_ms: 1_787_839_453_000.0,
		}
	}

	fn as_value(record: &Rating) -> serde_json::Value {
		serde_json::to_value(record).unwrap()
	}

	#[test]
	fn test_is_empty() {
		assert!(!dummy_rating(Some(7), "good").is_empty());
		assert!(!dummy_rating(Some(7), "").is_empty());
		assert!(!dummy_rating(None, "no score, but a note").is_empty());
		assert!(dummy_rating(None, "").is_empty());
		assert!(dummy_rating(None, "   ").is_empty());
	}

	#[test]
	fn round_trip() {
		let record = dummy_rating(Some(7), "good");
		assert_eq!(Rating::load_value(&as_value(&record)), Ok(record));
	}

	#[test]
	fn schema_version_mismatch_errors() {
		let mut value = as_value(&dummy_rating(Some(7), "good"));
		value["schema_version"] = serde_json::json!(SCHEMA_VERSION - 1);
		assert!(Rating::load_value(&value).is_err());
	}
}
