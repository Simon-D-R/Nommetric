//! Import/Export ratings to/from file.

use crate::key::ItemKey;
use crate::rating::{RATING_SCALE, Rating};
use std::collections::BTreeMap;

pub const BACKUP_FORMAT: &str = "nommetric-backup";
pub const BACKUP_FORMAT_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct BackupFile {
	pub format: String,
	pub format_version: u32,
	pub exported_at_ms: f64,
	pub records: BTreeMap<String, serde_json::Value>,
}

pub fn export_backup(stored: &serde_json::Value, exported_at_ms: f64) -> BackupFile {
	let records = stored
		.as_object()
		.into_iter()
		.flatten()
		.filter(|(stored_key, _value)| ItemKey::decode(stored_key).is_some())
		.map(|(stored_key, value)| (stored_key.clone(), value.clone()))
		.collect();
	BackupFile {
		format: BACKUP_FORMAT.to_string(),
		format_version: BACKUP_FORMAT_VERSION,
		exported_at_ms,
		records,
	}
}

pub fn import_backup(
	backup_text: &str,
	stored: &serde_json::Value,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
	let parsed: serde_json::Value =
		serde_json::from_str(backup_text).map_err(|err| format!("not JSON: {}", err))?;
	if parsed.get("format").and_then(serde_json::Value::as_str) != Some(BACKUP_FORMAT) {
		return Err("not a Nommetric backup".to_string());
	}
	let format_version = parsed
		.get("format_version")
		.and_then(serde_json::Value::as_u64)
		.ok_or("not a Nommetric backup")?;
	if format_version != u64::from(BACKUP_FORMAT_VERSION) {
		return Err(format!(
			"made by a newer version (format {})",
			format_version
		));
	}
	let backup: BackupFile =
		serde_json::from_value(parsed).map_err(|_err| "not a Nommetric backup".to_string())?;

	let writes = backup
		.records
		.into_iter()
		.filter_map(|(stored_key, backup_value)| {
			let key = ItemKey::decode(&stored_key)?;
			let local_value = stored.get(&stored_key);
			let write = match Rating::load_value(&backup_value) {
				Err(_reason) => local_value.is_none().then_some(backup_value),
				Ok(backup_record)
					if key.matches_record(&backup_record)
						&& !backup_record.is_empty()
						&& backup_record
							.score
							.is_none_or(|score| RATING_SCALE.contains(score)) =>
				{
					match local_value.map(Rating::load_value) {
						None => Some(backup_value),
						Some(Ok(local_record)) => {
							let merged = local_record.merged_with(&backup_record);
							(merged != local_record).then(|| {
								serde_json::to_value(&merged).expect("failed to serialise Rating")
							})
						}
						Some(Err(_reason)) => None,
					}
				}
				Ok(_invalid_record) => None,
			};
			Some((stored_key, write?))
		})
		.collect();
	Ok(writes)
}

#[cfg(test)]
mod tests {
	use super::{export_backup, import_backup};
	use crate::rating::{Rating, SCHEMA_VERSION};
	use serde_json::json;

	fn lasagne(score: Option<u8>, comment: &str, stamped_ms: f64) -> serde_json::Value {
		serde_json::to_value(Rating {
			schema_version: SCHEMA_VERSION,
			score,
			comment: comment.to_string(),
			dish_name: "Lasagne".to_string(),
			dish_lookup_key: "lasagne".to_string(),
			item_id: Some("1".to_string()),
			category_id: "c".to_string(),
			restaurant_slug: "s".to_string(),
			created_at_ms: stamped_ms,
			updated_at_ms: stamped_ms,
		})
		.unwrap()
	}

	#[test]
	fn round_trip() {
		let stored = json!({
			"r:s:i:c:1": lasagne(Some(8), "", 1.0),
			"r:s:n:c:lasagne": { "schema_version": 99 },
			"meta": { "last_export_at_ms": 1.0 },
		});
		let exported = export_backup(&stored, 5.0);
		assert_eq!(
			exported.records.keys().collect::<Vec<_>>(),
			["r:s:i:c:1", "r:s:n:c:lasagne"]
		);

		let text = serde_json::to_string(&exported).unwrap();
		assert_eq!(import_backup(&text, &json!({})), Ok(exported.records));
		assert_eq!(import_backup(&text, &stored), Ok(Default::default()));
	}

	#[test]
	fn import_merges_and_leaves_the_rest_alone() {
		let stored = json!({
			"r:s:i:c:1": lasagne(Some(9), "new", 5.0),
			"r:s:n:c:lasagne": { "schema_version": 99 },
		});
		let backup = json!({
			"format": "nommetric-backup",
			"format_version": 1,
			"exported_at_ms": 0,
			"records": {
				"r:s:i:c:1": lasagne(Some(3), "old", 1.0),
				"r:s:n:c:lasagne": lasagne(Some(3), "", 1.0),
				"r:s:i:c:2": lasagne(Some(3), "", 1.0),
				"r:s:i:c:3": lasagne(Some(42), "", 1.0),
				"hello": lasagne(Some(3), "", 1.0),
			},
		});

		let mut merged = lasagne(Some(9), "new\n---\nold", 5.0);
		merged["created_at_ms"] = json!(1.0);
		assert_eq!(
			import_backup(&backup.to_string(), &stored),
			Ok([("r:s:i:c:1".to_string(), merged)].into_iter().collect())
		);
	}
}
