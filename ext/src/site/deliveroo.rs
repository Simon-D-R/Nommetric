//! query_selectors for Deliveroo.

use crate::browser::{BrowserRuntime, js_error_message};
use nommetric_core::id_extraction::{DishIdIndex, RestaurantCheck, check_restaurant};
use nommetric_core::selectors::{IdSource, SELECTORS_VERSION, Selector, SelectorError};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use web_sys::{Document, Element, HtmlElement, Window};

const SELECTORS_RESOURCE: &str = "selectors.json";
const THEME_ATTRIBUTE: &str = "data-dt-mode";
const DEFAULT_THEME: &str = "light";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CardScope {
	Census,
	Mount,
}

pub struct DeliverooMenu {
	selectors: Selector,
	id_index: Option<DishIdIndex>,
}

impl DeliverooMenu {
	pub fn new(selectors: Selector, document: &Document, restaurant_slug: &str) -> DeliverooMenu {
		let id_index = build_id_index(&selectors, document, restaurant_slug);
		match &id_index {
			Some(index) => log::info!("id index built, {} distinct dish name(s)", index.len()),
			None => log::warn!("no id index, this page load is name-keyed"),
		}
		DeliverooMenu {
			selectors,
			id_index,
		}
	}

	pub fn menu_container(&self, document: &Document) -> Option<Element> {
		match document.query_selector(&self.selectors.menu_container) {
			Ok(found) => found,
			Err(err) => {
				log_invalid_selector(&self.selectors.menu_container, &err);
				None
			}
		}
	}

	pub fn card_selector(&self, scope: CardScope) -> &str {
		match scope {
			CardScope::Census => &self.selectors.card_census,
			CardScope::Mount => &self.selectors.card_mount,
		}
	}

	pub fn find_item_cards(&self, document: &Document, scope: CardScope) -> Vec<Element> {
		let selector = self.card_selector(scope);
		match document.query_selector_all(selector) {
			Ok(node_list) => (0..node_list.length())
				.filter_map(|index| node_list.item(index))
				.filter_map(|node| node.dyn_into::<Element>().ok())
				.collect(),
			Err(err) => {
				log_invalid_selector(selector, &err);
				Vec::new()
			}
		}
	}

	pub fn dish_name(&self, card: &Element) -> Option<String> {
		let name = card.query_selector(&self.selectors.name).ok()??;
		let text = name.text_content()?.trim().to_string();
		(!text.is_empty()).then_some(text)
	}

	pub fn category_ancestor_id(&self, card: &Element) -> Option<String> {
		let section = card.closest(&self.selectors.category_ancestor).ok()??;
		let id = section.id();
		(!id.is_empty()).then_some(id)
	}

	pub fn item_id(&self, dish_lookup_key: &str) -> Option<String> {
		self.id_index
			.as_ref()?
			.smallest_item_id(dish_lookup_key)
			.map(str::to_string)
	}

	pub fn declared_theme(&self, document: &Document) -> String {
		document
			.body()
			.and_then(|body| body.get_attribute(THEME_ATTRIBUTE))
			.unwrap_or_else(|| DEFAULT_THEME.to_string())
	}

	pub fn mount_point(&self, card: &Element) -> Option<HtmlElement> {
		let element = match &self.selectors.mount {
			None => card.clone(),
			Some(selector) => card.query_selector(selector).ok()??,
		};
		element.dyn_into::<HtmlElement>().ok()
	}

	pub fn warn_if_index_empty(&self, cards_found: usize) {
		let empty = self.id_index.as_ref().is_some_and(DishIdIndex::is_empty);
		if empty && cards_found > 0 {
			log::warn!(
				"the id index parsed but holds 0 entries while {cards_found} card(s) rendered — item_path {:?} finds nothing",
				self.item_paths()
			);
		}
	}

	fn item_paths(&self) -> Vec<&str> {
		self.selectors
			.id_sources
			.iter()
			.filter_map(|source| match source {
				IdSource::NextData { item_path, .. } => Some(item_path.as_str()),
				IdSource::Unknown => None,
			})
			.collect()
	}
}

pub async fn load_selectors(runtime: &impl BrowserRuntime, window: &Window) -> Option<Selector> {
	let url = runtime.resource_url(SELECTORS_RESOURCE);
	match fetch_text(window, &url).await {
		Err(err) => {
			log::error!("selectors.json unreachable: {}", err);
			None
		}
		Ok(text) => match Selector::try_from(text.as_str()) {
			Ok(selectors) => Some(selectors),
			Err(SelectorError::Version { found_version }) => {
				log::error!(
					"selectors.json is version {}, this build needs {}",
					found_version,
					SELECTORS_VERSION
				);
				None
			}
			Err(SelectorError::Json(message)) => {
				log::error!("selectors.json is malformed: {}", message);
				None
			}
		},
	}
}

async fn fetch_text(window: &Window, url: &str) -> Result<String, String> {
	let init = web_sys::RequestInit::new();
	init.set_cache(web_sys::RequestCache::NoStore);
	let request = web_sys::Request::new_with_str_and_init(url, &init)
		.map_err(|err| js_error_message(&err))?;

	let response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
		.await
		.map_err(|err| js_error_message(&err))?
		.dyn_into::<web_sys::Response>()
		.map_err(|_| "fetch did not return a Response".to_string())?;

	if !response.ok() {
		return Err(format!("HTTP {}", response.status()));
	}
	let text = response.text().map_err(|err| js_error_message(&err))?;
	wasm_bindgen_futures::JsFuture::from(text)
		.await
		.map_err(|err| js_error_message(&err))?
		.as_string()
		.ok_or_else(|| "response body was not text".to_string())
}

fn log_invalid_selector(selector: &str, err: &JsValue) {
	log::error!(
		"{:?} is not a valid selector: {}",
		selector,
		js_error_message(err)
	);
}

fn build_id_index(
	selectors: &Selector,
	document: &Document,
	restaurant_slug: &str,
) -> Option<DishIdIndex> {
	for (position, source) in selectors.id_sources.iter().enumerate() {
		match source {
			IdSource::Unknown => log::warn!("id source {}: unknown kind, skipped", position),
			IdSource::NextData {
				script_id,
				item_path,
				id_field,
				name_field,
				slug_path,
			} => match read_blob(document, script_id) {
				Err(err) => log::warn!("id source {}: {}", position, err),
				Ok(root) => match check_restaurant(&root, slug_path, restaurant_slug) {
					RestaurantCheck::Different { in_page, in_url } => log::warn!(
						"id source {}: blob holds {}, the url says {}, no ids",
						position,
						in_page,
						in_url
					),
					RestaurantCheck::SlugMissing => log::warn!(
						"id source {}: {} does not resolve, no ids",
						position,
						slug_path
					),
					RestaurantCheck::Same => {
						return Some(DishIdIndex::from_next_data(
							&root, item_path, id_field, name_field,
						));
					}
				},
			},
		}
	}
	None
}

fn read_blob(document: &Document, script_id: &str) -> Result<serde_json::Value, String> {
	match document.get_element_by_id(script_id) {
		None => Err(format!("no element with id {:?} in the page", script_id)),
		Some(script) => {
			let text = script.text_content().unwrap_or_default();
			serde_json::from_str(&text)
				.map_err(|err| format!("{:?} did not parse: {}", script_id, err))
		}
	}
}
