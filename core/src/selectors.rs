//! Serde mirror of `extension/selectors.json`.
pub const SELECTORS_VERSION: u32 = 1;

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Selector {
	pub version: u32,
	pub captured_at: String,
	pub site: String,
	pub card_census: String,
	pub card_mount: String,
	pub category_ancestor: String,
	pub name: String,

	#[serde(default)]
	pub mount: Option<String>, // None == the card itself
	pub menu_container: String,
	pub id_sources: Vec<IdSource>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum IdSource {
	NextData {
		script_id: String,
		item_path: String,
		id_field: String,
		name_field: String,
		slug_path: String,
	},
	#[serde(other)]
	Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectorError {
	Json(String),
	Version { found_version: u32 },
}

impl<'a> TryFrom<&'a str> for Selector {
	type Error = SelectorError;

	fn try_from(value: &'a str) -> Result<Self, Self::Error> {
		let selector: Selector =
			serde_json::from_str(value).map_err(|err| SelectorError::Json(err.to_string()))?;
		if selector.version != SELECTORS_VERSION {
			return Err(SelectorError::Version {
				found_version: selector.version,
			});
		}
		Ok(selector)
	}
}

#[cfg(test)]
mod tests {
	use crate::selectors::Selector;

	use super::{IdSource, SelectorError};

	const SHIPPED: &str = include_str!("../../extension/selectors.json");

	#[test]
	fn the_shipped_file_parses() {
		let selector = Selector::try_from(SHIPPED).unwrap();
		assert!(matches!(
			selector.id_sources.first(),
			Some(IdSource::NextData { .. })
		));
		assert_eq!(Selector::try_from(SHIPPED).unwrap().mount, None);
	}

	/// "newer than this build" and "malformed" want different runbook entries.
	#[test]
	fn unhappy_paths() {
		let newer = SHIPPED.replace("\"version\": 1", "\"version\": 2");
		assert_eq!(
			Selector::try_from(newer.as_str()).unwrap_err(),
			SelectorError::Version { found_version: 2 }
		);
		assert!(matches!(
			Selector::try_from("{ not json"),
			Err(SelectorError::Json(_))
		));

		let without_category = SHIPPED.replace("\"category_ancestor\"", "\"unused_by_this_build\"");
		match Selector::try_from(without_category.as_str()) {
			Err(SelectorError::Json(message)) => assert!(
				message.contains("category_ancestor"),
				"the error must name the missing field: {message}"
			),
			other => panic!("a file without category_ancestor parsed as {other:?}"),
		}

		let text = SHIPPED.replace("\"kind\": \"next-data\"", "\"kind\": \"ness\"");
		let selector = Selector::try_from(text.as_str()).unwrap();
		assert!(matches!(
			selector.id_sources.first(),
			Some(IdSource::Unknown)
		));
	}
}
