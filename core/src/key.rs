//! Storage keys, dish id lookup, name -> id flow.

use crate::rating::Rating;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemKey {
	Id {
		restaurant_slug: String,
		category_id: String,
		item_id: String,
	},
	Name {
		restaurant_slug: String,
		category_id: String,
		dish_lookup_key: String,
	},
}

impl ItemKey {
	pub fn encode(&self) -> String {
		match self {
			ItemKey::Id {
				restaurant_slug,
				category_id,
				item_id,
			} => format!(
				"{}i:{}:{}",
				ItemKey::restaurant_prefix(restaurant_slug),
				category_id,
				item_id
			),
			ItemKey::Name {
				restaurant_slug,
				category_id,
				dish_lookup_key,
			} => format!(
				"{}n:{}:{}",
				ItemKey::restaurant_prefix(restaurant_slug),
				category_id,
				dish_lookup_key
			),
		}
	}

	pub fn decode(key: &str) -> Option<ItemKey> {
		let mut fields = key.splitn(5, ':');
		if fields.next()? != "r" {
			return None;
		}
		let restaurant_slug = fields.next()?.to_string();
		let kind = fields.next()?;
		let category_id = fields.next()?.to_string();
		let remainder = fields.next()?.to_string();

		if restaurant_slug.is_empty() || category_id.is_empty() || remainder.is_empty() {
			return None;
		}
		match kind {
			"i" => Some(ItemKey::Id {
				restaurant_slug,
				category_id,
				item_id: remainder,
			}),
			"n" => Some(ItemKey::Name {
				restaurant_slug,
				category_id,
				dish_lookup_key: remainder,
			}),
			_ => None,
		}
	}

	pub fn restaurant_slug(&self) -> &str {
		match self {
			ItemKey::Id {
				restaurant_slug, ..
			}
			| ItemKey::Name {
				restaurant_slug, ..
			} => restaurant_slug,
		}
	}

	pub fn restaurant_prefix(restaurant_slug: &str) -> String {
		format!("r:{restaurant_slug}:")
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemIdentity {
	pub category_id: String,
	pub item_id: Option<String>,
	pub dish_name: String,
	pub dish_lookup_key: String,
	pub same_category_duplicate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Merge {
	pub to: ItemKey,
	pub record: Rating,
}

impl ItemIdentity {
	pub fn id_key(&self, restaurant_slug: &str) -> Option<ItemKey> {
		self.item_id.as_ref().map(|item_id| ItemKey::Id {
			restaurant_slug: restaurant_slug.to_string(),
			category_id: self.category_id.clone(),
			item_id: item_id.clone(),
		})
	}

	pub fn name_key(&self, restaurant_slug: &str) -> ItemKey {
		ItemKey::Name {
			restaurant_slug: restaurant_slug.to_string(),
			category_id: self.category_id.clone(),
			dish_lookup_key: self.dish_lookup_key.clone(),
		}
	}

	pub fn candidate_keys(&self, restaurant_slug: &str) -> impl Iterator<Item = ItemKey> {
		self.id_key(restaurant_slug)
			.into_iter()
			.chain(std::iter::once(self.name_key(restaurant_slug)))
	}

	pub fn resolve<'a>(
		&self,
		records: &'a HashMap<String, Rating>,
		restaurant_slug: &str,
	) -> Option<(ItemKey, &'a Rating)> {
		self.candidate_keys(restaurant_slug).find_map(|key| {
			let record = records.get(&key.encode()).filter(|r| !r.is_empty())?;
			Some((key, record))
		})
	}

	pub fn merge_to_id_key(
		&self,
		restaurant_slug: &str,
		id_record: Option<&Rating>,
		name_record: &Rating,
	) -> Option<Merge> {
		if self.same_category_duplicate {
			return None;
		}
		let to = self.id_key(restaurant_slug)?;

		let record = match id_record {
			None => Rating {
				item_id: self.item_id.clone(),
				category_id: self.category_id.clone(),
				..name_record.clone()
			},
			Some(id_record) => {
				let (newer, older) = if id_record.updated_at_ms >= name_record.updated_at_ms {
					(id_record, name_record)
				} else {
					(name_record, id_record)
				};
				Rating {
					score: newer.score.or(older.score),
					comment: merge_comments(&newer.comment, &older.comment),
					item_id: self.item_id.clone(),
					category_id: self.category_id.clone(),
					created_at_ms: newer.created_at_ms.min(older.created_at_ms),
					..newer.clone()
				}
			}
		};

		Some(Merge { to, record })
	}
}

fn merge_comments(newer: &str, older: &str) -> String {
	let new_trimmed = newer.trim();
	let old_trimmed = older.trim();
	match (new_trimmed.is_empty(), old_trimmed.is_empty()) {
		(true, true) => String::new(),
		(false, true) => new_trimmed.to_string(),
		(true, false) => old_trimmed.to_string(),
		(false, false) if new_trimmed == old_trimmed => new_trimmed.to_string(),
		(false, false) => format!("{}\n---\n{}", new_trimmed, old_trimmed),
	}
}

#[cfg(test)]
mod tests {
	use super::{ItemIdentity, ItemKey};
	use crate::lookup_key::dish_name_to_lookup_key;
	use crate::rating::{Rating, SCHEMA_VERSION};

	const SLUG: &str = "de-kastart-bvba";
	const CATEGORY: &str = "133151720";

	fn dummy_rating(dish_name: &str, score: Option<u8>, comment: &str, stamped_ms: f64) -> Rating {
		Rating {
			schema_version: SCHEMA_VERSION,
			score,
			comment: comment.to_string(),
			dish_name: dish_name.to_string(),
			dish_lookup_key: dish_name_to_lookup_key(dish_name),
			item_id: None,
			category_id: CATEGORY.to_string(),
			restaurant_slug: SLUG.to_string(),
			created_at_ms: stamped_ms,
			updated_at_ms: stamped_ms,
		}
	}

	fn dummy_identity(dish_name: &str, item_id: Option<&str>) -> ItemIdentity {
		ItemIdentity {
			category_id: CATEGORY.to_string(),
			item_id: item_id.map(str::to_string),
			dish_name: dish_name.to_string(),
			dish_lookup_key: dish_name_to_lookup_key(dish_name),
			same_category_duplicate: false,
		}
	}

	fn dummy_id_key(item_id: &str) -> ItemKey {
		ItemKey::Id {
			restaurant_slug: SLUG.to_string(),
			category_id: CATEGORY.to_string(),
			item_id: item_id.to_string(),
		}
	}

	fn dummy_name_key(dish_lookup_key: &str) -> ItemKey {
		ItemKey::Name {
			restaurant_slug: SLUG.to_string(),
			category_id: CATEGORY.to_string(),
			dish_lookup_key: dish_lookup_key.to_string(),
		}
	}

	#[test]
	fn keys_round_trip() {
		for key in [
			dummy_id_key("997582203"),
			dummy_id_key("not-numeric"),
			dummy_name_key("lasagne"),
			dummy_name_key("menu: kip curry"),
			dummy_name_key("menu:: kip curry"),
		] {
			assert_eq!(ItemKey::decode(&key.encode()), Some(key.clone()), "{key:?}");
		}
	}

	#[test]
	fn rejected_keys() {
		for key in [
			"",
			"lasagne",
			"r:de-kastart-bvba",
			"r:de-kastart-bvba:i:133151720",
			"r:de-kastart-bvba:i:133151720:",
			"r:de-kastart-bvba:x:133151720:lasagne",
			"r::i:133151720:997582203",
			"x:de-kastart-bvba:i:133151720:997582203",
		] {
			assert_eq!(ItemKey::decode(key), None, "{key}");
		}
	}

	#[test]
	fn merge_to_id_record() {
		let name_record = dummy_rating("Lasagne", Some(3), "from name key", 4.0);
		let id_record = dummy_rating("Lasagne", Some(9), "from id key", 4.0);

		let merged = dummy_identity("Lasagne", Some("1"))
			.merge_to_id_key(SLUG, Some(&id_record), &name_record)
			.unwrap();
		assert_eq!(merged.record.score, Some(9));
		assert_eq!(merged.record.comment, "from id key\n---\nfrom name key");
	}

	#[test]
	fn same_category_duplicate_blocks_merging() {
		let mut identity = dummy_identity("Kaassaus met Ham", Some("1"));
		identity.same_category_duplicate = true;
		let name_record = dummy_rating("Kaassaus met Ham", Some(3), "", 4.0);

		assert_eq!(identity.merge_to_id_key(SLUG, None, &name_record), None);
	}

	#[test]
	fn merging_idempotency() {
		let identity = dummy_identity("Lasagne", Some("47"));
		let name_record = dummy_rating("Lasagne", Some(7), "lennart", 5.0);
		let id_record = dummy_rating("Lasagne", Some(9), "vill du lasagne?", 6.0);

		let once = identity
			.merge_to_id_key(SLUG, Some(&id_record), &name_record)
			.unwrap()
			.record;
		let twice = identity
			.merge_to_id_key(SLUG, Some(&once), &once)
			.unwrap()
			.record;
		assert_eq!(twice, once);
	}
}
