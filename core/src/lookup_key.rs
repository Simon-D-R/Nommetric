use unicode_normalization::UnicodeNormalization as _;
use unicode_normalization::char::is_combining_mark;

pub fn dish_name_to_lookup_key(dish_name: &str) -> String {
	dish_name
		.nfd()
		.filter(|c| !is_combining_mark(*c))
		.collect::<String>()
		.to_lowercase()
		.split_whitespace()
		.collect::<Vec<_>>()
		.join(" ")
}

#[cfg(test)]
mod tests {
	use super::dish_name_to_lookup_key;

	const EXPECTED_LOOKUP_KEYS: &[(&str, &str)] = &[
		("Small Spaghetti Kastart", "small spaghetti kastart"),
		(
			"Large Spaghetti Kastart Vegetarisch",
			"large spaghetti kastart vegetarisch",
		),
		("Lasagne", "lasagne"),
		("Lasagne Vegetarisch", "lasagne vegetarisch"),
		("Small Kastart Saus (250ml)", "small kastart saus (250ml)"),
		("Broodje Ham & Jonge Gouda", "broodje ham & jonge gouda"),
		("Ice-Tea Green", "ice-tea green"),
		("Jupiler 0.0%", "jupiler 0.0%"),
		("Boule de Berlin met crème", "boule de berlin met creme"),
		("Broodje Rundspréparé", "broodje rundsprepare"),
		("Chicken Samouraï", "chicken samourai"),
		("Flaguette Crusty Chick'n", "flaguette crusty chick'n"),
		("Soep van de Dag 0,33L", "soep van de dag 0,33l"),
		("Coca\u{00A0}Cola", "coca cola"),
		("Coca\u{2007}Cola", "coca cola"),
		("Coca\u{202F}Cola", "coca cola"),
		("  Coca   Cola  ", "coca cola"),
	];

	#[test]
	fn test_dish_name_to_lookup_key() {
		for (dish_name, expected) in EXPECTED_LOOKUP_KEYS {
			assert_eq!(dish_name_to_lookup_key(dish_name), *expected, "{dish_name}");
		}
	}
}
