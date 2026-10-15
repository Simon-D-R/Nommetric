mod browser;
#[cfg(not(feature = "firefox"))]
mod chromium;
mod content;
mod logging;
mod site;
mod store;
mod ui;

use browser::{BrowserStorage, PlatformRuntime, PlatformStorage};
use content::observe;
use content::scan::{self, MenuScan};
use nommetric_core::Url;
use nommetric_core::slug::restaurant_slug;
use site::deliveroo::{DeliverooMenu, load_selectors};
use store::StoredRestaurantRatings;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn start_content() {
	console_error_panic_hook::set_once();
	logging::init();

	match current_restaurant_slug() {
		Some(slug) => wasm_bindgen_futures::spawn_local(async move {
			if let Some(observed) = start_menu(slug).await {
				observe::install(&observed, |new_slug| {
					log::info!("restaurant changed to {}, rebuilding", new_slug);
					scan::reset();
					wasm_bindgen_futures::spawn_local(async move {
						start_menu(new_slug).await;
					});
				});
				PlatformStorage::default().on_changed(scan::on_storage_change);
			}
		}),
		None => log::info!("start_content ok: no restaurant slug, idle"),
	}
}

pub(crate) fn current_restaurant_slug() -> Option<String> {
	let href = web_sys::window()?.location().href().ok()?;
	Url::parse(&href)
		.ok()
		.and_then(|parsed| restaurant_slug(&parsed))
}

/// The node the observer should watch, or None if there is nothing to mount on.
async fn start_menu(restaurant_slug: String) -> Option<web_sys::Node> {
	let window = web_sys::window()?;
	let document = window.document()?;

	let selectors = load_selectors(&PlatformRuntime::default(), &window).await?;
	let store = match StoredRestaurantRatings::load_for_restaurant(
		PlatformStorage::default(),
		&restaurant_slug,
	)
	.await
	{
		Ok(store) => store,
		Err(err) => {
			log::error!(
				"start_content: slug={}, store unavailable: {:?}",
				restaurant_slug,
				err
			);
			return None;
		}
	};
	log::info!(
		"start_content ok: slug={}, {} stored rating(s)",
		restaurant_slug,
		store.len()
	);

	let menu = DeliverooMenu::new(selectors, &document, &restaurant_slug);
	let observed = match menu.menu_container(&document) {
		Some(container) => web_sys::Node::from(container),
		None => {
			log::warn!("no menu container, observing document.body instead");
			web_sys::Node::from(document.body()?)
		}
	};

	scan::install(MenuScan::new(menu, store, restaurant_slug));
	scan::request_scan();
	Some(observed)
}

#[wasm_bindgen]
pub fn start_options() {
	console_error_panic_hook::set_once();
	logging::init();
	leptos::mount::mount_to_body(ui::options::OptionsPage);
}
