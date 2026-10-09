//! Convert editor data to keys to write or remove.

use crate::key::{ItemIdentity, ItemKey};
use crate::rating::{RATING_SCALE, Rating, SCHEMA_VERSION};

#[derive(Debug, Clone, PartialEq)]
pub struct DishEdit {
	pub score: Option<u8>,
	pub comment: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EditWrite {
	pub upsert: Option<(ItemKey, Rating)>,
	pub remove: Vec<ItemKey>,
}

impl EditWrite {
	pub fn is_empty(&self) -> bool {
		self.upsert.is_none() && self.remove.is_empty()
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditRefusal {
	SameCategoryDuplicate,
	ScoreOutOfRange(u8),
}

pub fn plan_edit(
	identity: &ItemIdentity,
	restaurant_slug: &str,
	shown: Option<(&ItemKey, &Rating)>,
	edit: &DishEdit,
	now_ms: f64,
) -> Result<EditWrite, EditRefusal> {
	if identity.same_category_duplicate {
		return Err(EditRefusal::SameCategoryDuplicate);
	}
	if let Some(score) = edit.score
		&& !RATING_SCALE.contains(score)
	{
		return Err(EditRefusal::ScoreOutOfRange(score));
	}

	let record = Rating {
		schema_version: SCHEMA_VERSION,
		score: edit.score,
		comment: edit.comment.trim().to_string(),
		dish_name: identity.dish_name.clone(),
		dish_lookup_key: identity.dish_lookup_key.clone(),
		item_id: identity.item_id.clone(),
		category_id: identity.category_id.clone(),
		restaurant_slug: restaurant_slug.to_string(),
		created_at_ms: shown.map_or(now_ms, |(_key, shown_record)| shown_record.created_at_ms),
		updated_at_ms: now_ms,
	};
	let shown_key = shown.map(|(key, _record)| key.clone());

	if record.is_empty() {
		return Ok(EditWrite {
			upsert: None,
			remove: shown_key.into_iter().collect(),
		});
	}

	let target_key = identity
		.id_key(restaurant_slug)
		.unwrap_or_else(|| identity.name_key(restaurant_slug));

	// Saving an unchanged record would still move updated_at_ms, promote's tiebreak.
	if let Some((key, shown_record)) = shown
		&& *key == target_key
		&& shown_record.score == record.score
		&& shown_record.comment.trim() == record.comment
	{
		return Ok(EditWrite::default());
	}

	Ok(EditWrite {
		remove: shown_key
			.filter(|key| *key != target_key)
			.into_iter()
			.collect(),
		upsert: Some((target_key, record)),
	})
}

#[cfg(test)]
mod tests {
	use super::{DishEdit, EditRefusal, EditWrite, plan_edit};
	use crate::key::{ItemIdentity, ItemKey};
	use crate::lookup_key::dish_name_to_lookup_key;
	use crate::rating::{Rating, SCHEMA_VERSION};

	const SLUG: &str = "de-kastart-bvba";
	const CATEGORY: &str = "layout-133151720";
	const ITEM_ID: &str = "997582238";
	const CREATED_MS: f64 = 1_000.0;
	const NOW_MS: f64 = 5_000.0;

	fn dummy_identity(dish_name: &str, item_id: Option<&str>) -> ItemIdentity {
		ItemIdentity {
			category_id: CATEGORY.to_string(),
			item_id: item_id.map(str::to_string),
			dish_name: dish_name.to_string(),
			dish_lookup_key: dish_name_to_lookup_key(dish_name),
			same_category_duplicate: false,
		}
	}

	fn dummy_rating(dish_name: &str, score: Option<u8>, comment: &str) -> Rating {
		Rating {
			schema_version: SCHEMA_VERSION,
			score,
			comment: comment.to_string(),
			dish_name: dish_name.to_string(),
			dish_lookup_key: dish_name_to_lookup_key(dish_name),
			item_id: None,
			category_id: CATEGORY.to_string(),
			restaurant_slug: SLUG.to_string(),
			created_at_ms: CREATED_MS,
			updated_at_ms: CREATED_MS,
		}
	}

	fn dummy_dish_edit(score: Option<u8>, comment: &str) -> DishEdit {
		DishEdit {
			score,
			comment: comment.to_string(),
		}
	}

	fn upserted_key(write: &EditWrite) -> &ItemKey {
		&write.upsert.as_ref().unwrap().0
	}

	#[test]
	fn with_item_id_writes_the_id_key() {
		let identity = dummy_identity("Lasagne", Some(ITEM_ID));
		let write = plan_edit(
			&identity,
			SLUG,
			None,
			&dummy_dish_edit(Some(8), "good"),
			NOW_MS,
		)
		.unwrap();
		assert_eq!(upserted_key(&write), &identity.id_key(SLUG).unwrap());
		assert!(write.remove.is_empty());
	}

	#[test]
	fn shown_name_key_moves_to_the_id_key() {
		let identity = dummy_identity("Lasagne", Some(ITEM_ID));
		let name_key = identity.name_key(SLUG);
		let shown_record = dummy_rating("Lasagne", Some(3), "");
		let write = plan_edit(
			&identity,
			SLUG,
			Some((&name_key, &shown_record)),
			&dummy_dish_edit(Some(8), ""),
			NOW_MS,
		)
		.unwrap();
		assert_eq!(upserted_key(&write), &identity.id_key(SLUG).unwrap());
		assert_eq!(write.remove, vec![name_key]);
	}

	#[test]
	fn shown_id_key_is_not_removed() {
		let identity = dummy_identity("Lasagne", Some(ITEM_ID));
		let id_key = identity.id_key(SLUG).unwrap();
		let shown_record = dummy_rating("Lasagne", Some(3), "");
		let write = plan_edit(
			&identity,
			SLUG,
			Some((&id_key, &shown_record)),
			&dummy_dish_edit(Some(8), ""),
			NOW_MS,
		)
		.unwrap();
		assert_eq!(upserted_key(&write), &id_key);
		assert!(write.remove.is_empty());
	}

	#[test]
	fn name_key_fallback() {
		let identity = dummy_identity("Lasagne", None);
		let write =
			plan_edit(&identity, SLUG, None, &dummy_dish_edit(Some(8), ""), NOW_MS).unwrap();
		assert_eq!(upserted_key(&write), &identity.name_key(SLUG));
		assert!(write.remove.is_empty());
	}

	#[test]
	fn empty_edit_removes_key() {
		let identity = dummy_identity("Lasagne", Some(ITEM_ID));
		let id_key = identity.id_key(SLUG).unwrap();
		let shown_record = dummy_rating("Lasagne", Some(3), "note");
		let write = plan_edit(
			&identity,
			SLUG,
			Some((&id_key, &shown_record)),
			&dummy_dish_edit(None, ""),
			NOW_MS,
		)
		.unwrap();
		assert_eq!(write.upsert, None);
		assert_eq!(write.remove, vec![id_key]);
	}

	#[test]
	fn same_category_duplicate_is_refused() {
		let mut identity = dummy_identity("Kaassaus met Ham", Some(ITEM_ID));
		identity.same_category_duplicate = true;
		assert_eq!(
			plan_edit(&identity, SLUG, None, &dummy_dish_edit(Some(8), ""), NOW_MS),
			Err(EditRefusal::SameCategoryDuplicate)
		);
	}
}
