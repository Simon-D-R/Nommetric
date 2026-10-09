//! Storage of Ratings in browser local storage.

use crate::browser::{BrowserStorage, BrowserStorageError, PlatformStorage};
use nommetric_core::edit::EditWrite;
use nommetric_core::key::{ItemIdentity, ItemKey};
use nommetric_core::rating::Rating;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum StoreError {
	Storage(BrowserStorageError),
	Json(String),
	ScoreOutOfRange(u8),
	Unreadable(String),
}

impl From<BrowserStorageError> for StoreError {
	fn from(err: BrowserStorageError) -> StoreError {
		StoreError::Storage(err)
	}
}

pub struct StoredRestaurantRatings<Storage: BrowserStorage = PlatformStorage> {
	storage: Storage,
	restaurant_slug: String,
	records: HashMap<String, Rating>,
	unreadable: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub enum ChangeResult {
	Upserted,
	Removed,
	Ignored,
	Unreadable,
}

/// Bypass cache (catches up with onChanged).
pub async fn write_edit(
	storage: &impl BrowserStorage,
	write: &EditWrite,
) -> Result<(), StoreError> {
	if let Some((key, record)) = &write.upsert {
		let value =
			serde_json::to_value(record).map_err(|err| StoreError::Json(err.to_string()))?;
		storage.insert(&key.encode(), &value).await?;
	}
	if !write.remove.is_empty() {
		let stored_keys: Vec<String> = write.remove.iter().map(ItemKey::encode).collect();
		storage.remove(&stored_keys).await?;
	}
	Ok(())
}

impl<Storage: BrowserStorage> StoredRestaurantRatings<Storage> {
	pub async fn load_for_restaurant(
		storage: Storage,
		restaurant_slug: &str,
	) -> Result<StoredRestaurantRatings<Storage>, StoreError> {
		let prefix = ItemKey::restaurant_prefix(restaurant_slug);
		let mut ratings = StoredRestaurantRatings {
			storage,
			restaurant_slug: restaurant_slug.to_string(),
			records: HashMap::new(),
			unreadable: HashMap::new(),
		};

		let serde_json::Value::Object(all_keys) = ratings.storage.get_all().await? else {
			return Ok(ratings);
		};
		for (stored_key, value) in all_keys {
			if stored_key.starts_with(&prefix) {
				ratings.ingest(stored_key, &value);
			}
		}
		Ok(ratings)
	}

	fn ingest(&mut self, stored_key: String, value: &serde_json::Value) -> bool {
		match Rating::load_value(value) {
			Ok(record) => {
				self.unreadable.remove(&stored_key);
				self.records.insert(stored_key, record);
				true
			}
			Err(err) => {
				log::warn!("error on {stored_key}: '{err}', left untouched");
				self.records.remove(&stored_key);
				self.unreadable.insert(stored_key, value.clone());
				false
			}
		}
	}

	pub fn len(&self) -> usize {
		self.records.len()
	}

	pub fn is_empty(&self) -> bool {
		self.records.is_empty()
	}

	pub fn records(&self) -> &HashMap<String, Rating> {
		&self.records
	}

	pub fn unreadable(&self) -> &HashMap<String, serde_json::Value> {
		&self.unreadable
	}

	pub fn get(&self, identity: &ItemIdentity) -> Option<(ItemKey, &Rating)> {
		identity.resolve(&self.records, &self.restaurant_slug)
	}

	pub fn is_unreadable(&self, identity: &ItemIdentity) -> bool {
		identity
			.candidate_keys(&self.restaurant_slug)
			.any(|candidate| self.unreadable.contains_key(&candidate.encode()))
	}

	pub async fn put(&mut self, key: &ItemKey, record: Rating) -> Result<(), StoreError> {
		let stored_key = key.encode();
		if self.unreadable.contains_key(&stored_key) {
			return Err(StoreError::Unreadable(stored_key));
		}
		if let Some(score) = record.score
			&& !RATING_SCALE.contains(score)
		{
			return Err(StoreError::ScoreOutOfRange(score));
		}

		if record.is_empty() {
			self.remove_through(stored_key).await
		} else {
			self.write_through(stored_key, record).await
		}
	}

	async fn write_through(
		&mut self,
		stored_key: String,
		record: Rating,
	) -> Result<(), StoreError> {
		let value =
			serde_json::to_value(&record).map_err(|err| StoreError::Json(err.to_string()))?;
		self.storage.insert(&stored_key, &value).await?;
		self.records.insert(stored_key, record);
		Ok(())
	}

	async fn remove_through(&mut self, stored_key: String) -> Result<(), StoreError> {
		self.storage
			.remove(std::slice::from_ref(&stored_key))
			.await?;
		self.records.remove(&stored_key);
		Ok(())
	}

	pub async fn promote(
		&mut self,
		identity: &ItemIdentity,
	) -> Result<Option<ItemKey>, StoreError> {
		let from_key = identity.name_key(&self.restaurant_slug).encode();
		if let Some(name_record) = self.records.get(&from_key).cloned() {
			let id_record = identity
				.id_key(&self.restaurant_slug)
				.and_then(|to| self.records.get(&to.encode()))
				.cloned();

			if let Some(merge) =
				identity.merge_to_id_key(&self.restaurant_slug, id_record.as_ref(), &name_record)
			{
				let to_key = merge.to.encode();
				if self.unreadable.contains_key(&to_key) {
					return Err(StoreError::Unreadable(to_key));
				}

				self.write_through(to_key, merge.record).await?;
				self.remove_through(from_key).await?;

				Ok(Some(merge.to))
			} else {
				Ok(None)
			}
		} else {
			Ok(None)
		}
	}

	pub fn apply_change(
		&mut self,
		stored_key: &str,
		new_value: Option<&serde_json::Value>,
	) -> ChangeResult {
		if let Some(key) = ItemKey::decode(stored_key) {
			if key.restaurant_slug() != self.restaurant_slug {
				return ChangeResult::Ignored;
			}

			if let Some(value) = new_value {
				if self.ingest(stored_key.to_string(), value) {
					ChangeResult::Upserted
				} else {
					ChangeResult::Unreadable
				}
			} else {
				self.records.remove(stored_key);
				self.unreadable.remove(stored_key);
				ChangeResult::Removed
			}
		} else {
			ChangeResult::Ignored
		}
	}
}
