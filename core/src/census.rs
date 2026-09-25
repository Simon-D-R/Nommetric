//! Go over dishes for a restaurant, getting ids and counting duplicate names.

use std::collections::{BTreeMap, HashMap};

type DishLookupKey = String;

#[derive(Default, Debug)]
pub struct DishCensus {
	entries: HashMap<DishLookupKey, CensusEntry>,
}

#[derive(Default, Debug)]
struct CensusEntry {
	cards_per_category: BTreeMap<String, usize>,
}

impl CensusEntry {
	fn has_same_category_duplicate(&self) -> bool {
		self.cards_per_category.values().any(|count| *count > 1)
	}
}

impl DishCensus {
	pub fn observe_pass<'a>(
		&mut self,
		cards: impl Iterator<Item = (&'a str, &'a str)>,
	) -> Vec<DishLookupKey> {
		let mut this_pass: HashMap<&str, BTreeMap<&str, usize>> = HashMap::new();
		for (dish_lookup_key, category_id) in cards {
			*this_pass
				.entry(dish_lookup_key)
				.or_default()
				.entry(category_id)
				.or_default() += 1;
		}

		let mut newly_unseparable = Vec::new();
		for (dish_lookup_key, per_category) in this_pass {
			let entry = self.entries.entry(dish_lookup_key.to_string()).or_default();
			let was_unseparable = entry.has_same_category_duplicate();
			for (category_id, count) in per_category {
				let seen = entry
					.cards_per_category
					.entry(category_id.to_string())
					.or_default();
				*seen = (*seen).max(count);
			}
			if entry.has_same_category_duplicate() && !was_unseparable {
				newly_unseparable.push(dish_lookup_key.to_string());
			}
		}
		newly_unseparable
	}

	pub fn is_duplicated(&self, dish_lookup_key: &str) -> bool {
		self.cards_with_name(dish_lookup_key) > 1
	}

	pub fn cards_with_name(&self, dish_lookup_key: &str) -> usize {
		self.entries
			.get(dish_lookup_key)
			.map(|entry| entry.cards_per_category.values().sum())
			.unwrap_or(0)
	}

	pub fn has_same_category_duplicate(&self, dish_lookup_key: &str) -> bool {
		self.entries
			.get(dish_lookup_key)
			.is_some_and(CensusEntry::has_same_category_duplicate)
	}

	pub fn sole_category(&self, dish_lookup_key: &str) -> Option<&str> {
		let entry = self.entries.get(dish_lookup_key)?;
		let mut categories = entry.cards_per_category.keys();
		let category = categories.next()?;
		categories.next().is_none().then_some(category.as_str())
	}
}

#[cfg(test)]
mod tests {
	use super::DishCensus;

	const CATEGORY: &str = "6283185";

	#[test]
	fn menu_with_no_repeats() {
		let names = [
			"Small Spaghetti Kastart",
			"Medium Spaghetti Kastart",
			"Large Spaghetti Kastart",
			"Small Spaghetti Kastart Vegetarisch",
			"Medium Spaghetti Kastart Vegetarisch",
			"Large Spaghetti Kastart Vegetarisch",
			"Small Spaghetti Bolognaise",
			"Medium Spaghetti Bolognaise",
			"Large Spaghetti Bolognaise",
		];

		let mut census = DishCensus::default();
		let newly_unseparable = census.observe_pass(names.iter().map(|key| (*key, CATEGORY)));

		assert!(newly_unseparable.is_empty());
		for key in &names {
			assert!(!census.is_duplicated(key), "{key}");
			assert!(!census.has_same_category_duplicate(key));
			assert_eq!(census.sole_category(key), Some(CATEGORY));
		}
	}

	#[test]
	fn repeat_across_categories() {
		let mut census = DishCensus::default();
		let newly_unseparable = census.observe_pass(
			[
				("kaassaus met ham", "111111111"),
				("kaassaus met ham", "222222222"),
			]
			.into_iter(),
		);

		assert!(newly_unseparable.is_empty());
		assert!(census.is_duplicated("kaassaus met ham"));
		assert!(!census.has_same_category_duplicate("kaassaus met ham"));
		assert_eq!(census.cards_with_name("kaassaus met ham"), 2);
		assert_eq!(census.sole_category("kaassaus met ham"), None);
	}

	#[test]
	fn repeat_within_category() {
		let cards = [
			("kaassaus met ham", "111111111"),
			("kaassaus met ham", "111111111"),
		];
		let mut census = DishCensus::default();
		let newly_unseparable = census.observe_pass(cards.into_iter());

		assert_eq!(newly_unseparable, vec!["kaassaus met ham".to_string()]);
		assert!(census.has_same_category_duplicate("kaassaus met ham"));

		// second pass should no longer report new unseparables
		let newly_unseparable_2 = census.observe_pass(cards.into_iter());
		assert!(newly_unseparable_2.is_empty());
	}
}
