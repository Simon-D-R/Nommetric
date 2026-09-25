use url::Url;

/// Extracts restaurant slug from full url
///
/// e.g.
/// "https://deliveroo.be/en/menu/Ghent/sint-michiels/de-kastart-bvba/?day=today&…"
///   -> "de-kastart-bvba"
///
/// The route is `/menu/[city]/[neighbourhood]/[restaurant]`
pub fn restaurant_slug(url: &Url) -> Option<String> {
	let mut segs = url.path_segments()?.filter(|s| !s.is_empty());
	segs.find(|s| *s == "menu")?;
	let _city = segs.next()?;
	let _neighbourhood = segs.next()?;
	let uname = segs.next()?;
	Some(canonical_slug(uname))
}

pub fn canonical_slug(slug: &str) -> String {
	slug.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
	use super::restaurant_slug;
	use url::Url;

	#[test]
	fn test_restaurant_slug() {
		let cases: &[(&str, Option<&str>)] = &[
			(
				"https://deliveroo.be/en/menu/Ghent/sint-michiels/de-kastart-bvba/?day=today&geohash=u13dk53yn5v8",
				Some("de-kastart-bvba"),
			),
			(
				"https://deliveroo.be/en/menu/Ghent/sint-michiels/de-kastart-bvba",
				Some("de-kastart-bvba"),
			),
			(
				"https://deliveroo.be/en/menu/Ghent/sint-michiels/de-kastart-bvba/#menu",
				Some("de-kastart-bvba"),
			),
			(
				"https://deliveroo.be/fr/menu/Ghent/sint-michiels/de-kastart-bvba/",
				Some("de-kastart-bvba"),
			),
			(
				"https://deliveroo.be/nl/menu/Ghent/sint-michiels/de-kastart-bvba/",
				Some("de-kastart-bvba"),
			),
			(
				"https://deliveroo.be/menu/Ghent/sint-michiels/de-kastart-bvba/",
				Some("de-kastart-bvba"),
			),
			(
				"https://deliveroo.be/en/menu/GHENT/SINT-MICHIELS/De-Kastart-BVBA/",
				Some("de-kastart-bvba"),
			),
			(
				"https://deliveroo.be/fr/menu/Ghent/korenmarkt/selis-noodlebar-1/",
				Some("selis-noodlebar-1"),
			),
			(
				"https://deliveroo.be/en/menu/Ghent/sint-michiels/de-kastart-bvba/extra/segments",
				Some("de-kastart-bvba"),
			),
			("https://deliveroo.be/en/", None),
			("https://deliveroo.be/en/menu", None),
			("https://deliveroo.be/en/menu/Ghent", None),
			("https://deliveroo.be/en/menu/Ghent/sint-michiels", None),
			(
				"https://deliveroo.be/en/restaurants/ghent/sint-michiels",
				None,
			),
			("not a url", None),
			("", None),
		];

		for (url, expected) in cases {
			assert_eq!(
				Url::parse(url)
					.ok()
					.and_then(|parsed| restaurant_slug(&parsed))
					.as_deref(),
				*expected,
				"url: {url}"
			);
		}
	}
}
