//! Read dish ids from the page's embedded __NEXT_DATA__.

use crate::lookup_key::dish_name_to_lookup_key;
use crate::slug::canonical_slug;
use std::collections::HashMap;

pub fn value_at_path<'a>(root: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
	path.split('.')
		.try_fold(root, |value, field| value.get(field))
}

/// Follows a dotted path in which a segment may end in `[]`.
/// Returns nothing if the path is absent or a `[]` segment is not an array.
pub fn array_values_at_path<'a>(
	root: &'a serde_json::Value,
	path: &str,
) -> Vec<&'a serde_json::Value> {
	let mut found = vec![root];
	for segment in path.split('.') {
		let (field, is_array) = match segment.strip_suffix("[]") {
			Some(field) => (field, true),
			None => (segment, false),
		};
		let mut next = Vec::new();
		for value in found {
			let Some(child) = value.get(field) else {
				continue;
			};
			match (is_array, child.as_array()) {
				(true, Some(elements)) => next.extend(elements.iter()),
				(true, None) => {}
				(false, _) => next.push(child),
			}
		}
		found = next;
	}
	found
}

#[derive(Debug, Clone, PartialEq)]
pub enum RestaurantCheck {
	Same,
	Different { in_page: String, in_url: String },
	SlugMissing,
}

pub fn check_restaurant(
	root: &serde_json::Value,
	slug_path: &str,
	restaurant_slug: &str,
) -> RestaurantCheck {
	if let Some(slug_in_page) = value_at_path(root, slug_path).and_then(serde_json::Value::as_str) {
		let slug_in_page_lc = canonical_slug(slug_in_page);
		if slug_in_page_lc == restaurant_slug {
			RestaurantCheck::Same
		} else {
			RestaurantCheck::Different {
				in_page: slug_in_page_lc,
				in_url: restaurant_slug.to_string(),
			}
		}
	} else {
		RestaurantCheck::SlugMissing
	}
}

#[derive(Debug, Default)]
pub struct DishIdIndex {
	matches_by_dish: HashMap<String, MatchingItems>,
}

#[derive(Debug, Default)]
struct MatchingItems {
	item_ids: Vec<String>,
	count: usize,
}

impl DishIdIndex {
	pub fn from_next_data(
		root: &serde_json::Value,
		item_path: &str,
		id_field: &str,
		name_field: &str,
	) -> DishIdIndex {
		let mut matches_by_dish: HashMap<String, MatchingItems> = HashMap::new();
		for item in array_values_at_path(root, item_path) {
			let Some(dish_name) = item.get(name_field).and_then(serde_json::Value::as_str) else {
				continue;
			};
			let dish_lookup_key = dish_name_to_lookup_key(dish_name);
			if dish_lookup_key.is_empty() {
				continue;
			}
			let matching = matches_by_dish.entry(dish_lookup_key).or_default();
			matching.count += 1;
			if let Some(item_id) = read_item_id(item.get(id_field)) {
				matching.item_ids.push(item_id);
			}
		}
		DishIdIndex { matches_by_dish }
	}

	/// Numeric ids first, smallest wins; anything unparseable only if nothing else is there.
	pub fn smallest_item_id(&self, dish_lookup_key: &str) -> Option<&str> {
		self.matches_by_dish
			.get(dish_lookup_key)?
			.item_ids
			.iter()
			.min_by(|a, b| numeric_rank(a).cmp(&numeric_rank(b)).then_with(|| a.cmp(b)))
			.map(String::as_str)
	}

	pub fn len(&self) -> usize {
		self.matches_by_dish.len()
	}

	pub fn is_empty(&self) -> bool {
		self.matches_by_dish.is_empty()
	}

	pub fn item_count(&self, dish_lookup_key: &str) -> usize {
		self.matches_by_dish
			.get(dish_lookup_key)
			.map(|matching| matching.count)
			.unwrap_or(0)
	}
}

fn numeric_rank(item_id: &str) -> (bool, u128) {
	match item_id.parse::<u128>() {
		Ok(number) => (false, number),
		Err(_) => (true, 0),
	}
}

fn read_item_id(value: Option<&serde_json::Value>) -> Option<String> {
	match value? {
		serde_json::Value::String(text) if !text.is_empty() => Some(text.clone()),
		serde_json::Value::Number(number) => number.as_u64().map(|whole| whole.to_string()),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::{
		DishIdIndex, RestaurantCheck, array_values_at_path, check_restaurant, value_at_path,
	};

	const ITEM_PATH: &str = "props.initialState.menuPage.menu.metas.root.items[]";
	const SLUG_PATH: &str = "props.initialState.menuPage.menu.metas.root.restaurant.uname";
	const FIXTURE_SLUG: &str = "bocca-gent";

	fn page() -> serde_json::Value {
		json!(
			{
				"props": {
					"initialState": {
						"menuPage": {
							"menu": {
								"metas": {
									"root": {
										"restaurant": {
											"uname": "bocca-gent",
											"links": { "self": { "href": "/menu/ghent/<area>/bocca-gent" } }
										},
										"items": [
											{ "id": 900000001, "name": "Tiramisu" },
											{ "id": 998853398, "name": "Bolognaise" },
											{ "id": 998853437, "name": "Bolognaise" },
											{ "id": 998853438, "name": "Bolognaise" },
											{ "id": 1532454826, "name": "Bolognaise" },
											{ "id": 998853359, "name": "Marisol" },
											{ "id": 998853342, "name": "Marisol" },
											{ "id": 998853343, "name": "Marisol" },
											{ "id": 1532454828, "name": "Marisol" },
											{ "id": 998853462, "name": "Kaassaus met Ham" },
											{ "id": 998853433, "name": "Kaassaus met Ham" },
											{ "id": 998853417, "name": "Kaassaus met Ham" },
											{ "id": 998853418, "name": "Kaassaus met Ham" },
											{ "id": 998853422, "name": "4 Kazen" },
											{ "id": 998853266, "name": "4 Kazen" },
											{ "id": 998853267, "name": "4 Kazen" },
											{ "id": 998853302, "name": "Pasta van de Week" },
											{ "id": 1532455804, "name": "Pasta van de Week" },
											{ "id": 998853329, "name": "Groene Pesto" },
											{ "id": 998853429, "name": "Groene Pesto" },
											{ "id": 998853430, "name": "Groene Pesto" },
											{ "id": 1532454832, "name": "Groene Pesto" }
										]
									}
								}
							}
						}
					}
				}
			}
		)
	}

	fn index() -> DishIdIndex {
		DishIdIndex::from_next_data(&page(), ITEM_PATH, "id", "name")
	}

	#[test]
	fn bad_paths() {
		assert!(array_values_at_path(&page(), "props.nope.items[]").is_empty());
		assert!(array_values_at_path(&page(), "props[].initialState").is_empty());
		assert_eq!(value_at_path(&page(), "props.nope"), None);
	}

	#[test]
	fn good_path() {
		assert_eq!(
			value_at_path(&page(), SLUG_PATH).and_then(serde_json::Value::as_str),
			Some(FIXTURE_SLUG)
		);
	}

	#[test]
	fn bad_slug_path() {
		assert_eq!(
			check_restaurant(&page(), "props.nope.uname", FIXTURE_SLUG),
			RestaurantCheck::SlugMissing
		);
	}

	#[test]
	fn id_read() {
		assert_eq!(index().smallest_item_id("tiramisu"), Some("900000001"));
	}

	#[test]
	fn item_count() {
		let index = index();
		assert_eq!(index.item_count("bolognaise"), 4);
		assert_eq!(index.item_count("pasta van de week"), 2);
		assert_eq!(index.item_count("tiramisu"), 1);
	}

	#[test]
	fn missing_dish() {
		let index = index();
		assert_eq!(index.smallest_item_id("lasagne"), None);
		assert_eq!(index.item_count("lasagne"), 0);
	}
}
