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

const COMMENT_SEPARATOR: &str = "\n---\n";

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

	pub fn merged_with(&self, other: &Rating) -> Rating {
		let (newer, older) = if self.updated_at_ms >= other.updated_at_ms {
			(self, other)
		} else {
			(other, self)
		};
		Rating {
			score: newer.score.or(older.score),
			comment: merge_comments(&newer.comment, &older.comment),
			created_at_ms: newer.created_at_ms.min(older.created_at_ms),
			..newer.clone()
		}
	}
}

fn merge_comments(newer: &str, older: &str) -> String {
	let mut merged: Vec<&str> = comment_segments(newer).collect();
	for segment in comment_segments(older) {
		if !merged.contains(&segment) {
			merged.push(segment);
		}
	}
	merged.join(COMMENT_SEPARATOR)
}

fn comment_segments(comment: &str) -> impl Iterator<Item = &str> {
	comment
		.split(COMMENT_SEPARATOR)
		.map(str::trim)
		.filter(|segment| !segment.is_empty())
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
			category_id: "layout-133151720".to_string(),
			restaurant_slug: "de-kastart-bvba".to_string(),
			created_at_ms: 1_787_839_453_000.0,
			updated_at_ms: 1_787_839_453_000.0,
		}
	}

	fn stamped(record: Rating, created_at_ms: f64, updated_at_ms: f64) -> Rating {
		Rating {
			created_at_ms,
			updated_at_ms,
			..record
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

	#[test]
	fn merge() {
		let older = stamped(dummy_rating(Some(3), "x\n---\ny"), 1.0, 2.0);
		let newer = stamped(dummy_rating(None, " new \n---\nx"), 5.0, 6.0);
		let merged = older.merged_with(&newer);
		assert_eq!(merged.score, Some(3));
		assert_eq!(merged.comment, "new\n---\nx\n---\ny");
		assert_eq!(merged.created_at_ms, 1.0);
		assert_eq!(merged.updated_at_ms, 6.0);
	}

	#[test]
	fn merge_is_idempotent() {
		let older = stamped(dummy_rating(Some(3), "old"), 1.0, 1.0);
		let newer = stamped(dummy_rating(Some(9), "new"), 2.0, 2.0);
		let once = newer.merged_with(&older);
		assert_eq!(once.comment, "new\n---\nold");
		assert_eq!(once.merged_with(&older), once);
		assert_eq!(once.merged_with(&newer), once);
		assert_eq!(once.merged_with(&once), once);
	}
}
