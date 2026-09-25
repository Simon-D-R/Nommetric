//! TEMPORARY — delete this file and its `mod` line when ui/options.rs lands.

use crate::browser::{BrowserStorage, PlatformStorage};
use crate::store::StoredRestaurantRatings;
use nommetric_core::key::ItemIdentity;
use nommetric_core::lookup_key::dish_name_to_lookup_key;
use nommetric_core::rating::{Rating, SCHEMA_VERSION};
use wasm_bindgen::prelude::*;

/// Reserved slug, so nothing the harness writes can collide with a real restaurant.
const HARNESS_SLUG: &str = "__harness__";
const HARNESS_CATEGORY: &str = "133151720";
const HARNESS_ITEM_ID: &str = "997582203";
const HARNESS_DISH: &str = "Harness Dish";

thread_local! {
	static WATCHED: std::cell::RefCell<Option<StoredRestaurantRatings>> = const { std::cell::RefCell::new(None) };
}

/// What `start_options` runs until there is a real options page.
pub fn watch_and_run() {
	PlatformStorage::default().on_changed(|stored_key, new_value| {
		WATCHED.with_borrow_mut(|watched| {
			if let Some(stored_ratings) = watched.as_mut() {
				let outcome = stored_ratings.apply_change(stored_key, new_value);
				log::info!(
					"onChanged {} -> {:?} ({} records, {} unreadable)",
					stored_key,
					outcome,
					stored_ratings.len(),
					stored_ratings.unreadable().len()
				);
			}
		});
	});

	wasm_bindgen_futures::spawn_local(async {
		match load_harness_store().await {
			Ok(store) => WATCHED.with_borrow_mut(|watched| *watched = Some(store)),
			Err(err) => log::error!("cannot watch the harness store: {err}"),
		}
		report_harness(harness(false).await);
	});
}

fn harness_identity() -> ItemIdentity {
	ItemIdentity {
		category_id: HARNESS_CATEGORY.to_string(),
		item_id: Some(HARNESS_ITEM_ID.to_string()),
		dish_name: HARNESS_DISH.to_string(),
		dish_lookup_key: dish_name_to_lookup_key(HARNESS_DISH),
		same_category_duplicate: false,
	}
}

fn harness_rating(score: Option<u8>, comment: &str) -> Rating {
	let stamped_ms = js_sys::Date::now();
	Rating {
		schema_version: SCHEMA_VERSION,
		score,
		comment: comment.to_string(),
		dish_name: HARNESS_DISH.to_string(),
		dish_lookup_key: dish_name_to_lookup_key(HARNESS_DISH),
		item_id: Some(HARNESS_ITEM_ID.to_string()),
		category_id: HARNESS_CATEGORY.to_string(),
		restaurant_slug: HARNESS_SLUG.to_string(),
		created_at_ms: stamped_ms,
		updated_at_ms: stamped_ms,
	}
}

/// `keep: true` stops before the two cleanup steps, so a second tab
/// watching `onChanged` has a record left to find.
#[wasm_bindgen]
pub fn run_harness(keep: bool) {
	wasm_bindgen_futures::spawn_local(async move { report_harness(harness(keep).await) });
}

fn report_harness(outcome: Result<(), String>) {
	match outcome {
		Ok(()) => log::info!("store harness ok"),
		Err(message) => log::error!("store harness failed: {message}"),
	}
}

async fn harness(keep: bool) -> Result<(), String> {
	log::info!("store harness, restaurant={HARNESS_SLUG}");
	let identity = harness_identity();
	let id_key = identity
		.id_key(HARNESS_SLUG)
		.expect("the harness identity carries an item_id");
	let name_key = identity.name_key(HARNESS_SLUG);

	let mut store = load_harness_store().await?;
	log::info!("  1 load_for_restaurant       -> {} records", store.len());

	store
		.put(&id_key, harness_rating(Some(8), ""))
		.await
		.map_err(|err| format!("{err:?}"))?;
	log::info!("  2 put id-keyed  (score 8)   -> ok");

	let mut written_before_ids = harness_rating(None, "written before ids existed");
	written_before_ids.item_id = None;
	store
		.put(&name_key, written_before_ids)
		.await
		.map_err(|err| format!("{err:?}"))?;
	log::info!("  3 put name-keyed (comment)  -> ok");

	let mut store = load_harness_store().await?;
	log::info!("  4 load again                -> {} records", store.len());

	match store.get(&identity) {
		Some((key, record)) if key == id_key => log::info!(
			"  5 resolve by id             -> hit, score {:?}, read-only {}",
			record.score,
			store.is_unreadable(&identity)
		),
		other => return Err(format!("step 5 resolved to {other:?}")),
	}

	match store.promote(&identity).await {
		Ok(Some(_)) => {
			let merged = store
				.get(&identity)
				.ok_or("step 6 lost the merged record")?
				.1;
			log::info!(
				"  6 promote name -> id        -> merged, score {:?}, comment {:?}",
				merged.score,
				merged.comment
			);
			if store.records().contains_key(&name_key.encode()) {
				return Err("step 6 left the name key behind".to_string());
			}
		}
		Ok(None) => return Err("step 6 found nothing to promote".to_string()),
		Err(err) => return Err(format!("{err:?}")),
	}

	match store.promote(&identity).await {
		Ok(None) => log::info!("  7 promote again             -> no-op"),
		other => return Err(format!("step 7 returned {other:?}")),
	}

	if keep {
		log::info!("  8 keep=true, stopping here  -> merged record left in storage");
		return Ok(());
	}

	store
		.put(&id_key, harness_rating(None, "   "))
		.await
		.map_err(|err| format!("{err:?}"))?;
	log::info!("  8 put score=None comment=\"\" -> removed");

	let store = load_harness_store().await?;
	log::info!("  9 load again                -> {} records", store.len());
	if !store.is_empty() {
		return Err(format!("step 9 left {} records", store.len()));
	}
	Ok(())
}

async fn load_harness_store() -> Result<StoredRestaurantRatings, String> {
	StoredRestaurantRatings::load_for_restaurant(PlatformStorage::default(), HARNESS_SLUG)
		.await
		.map_err(|err| format!("{err:?}"))
}

/// Every stored key, for poking by hand from the options console.
#[wasm_bindgen]
pub fn dump_all() {
	wasm_bindgen_futures::spawn_local(async {
		match PlatformStorage::default().get_all().await {
			Ok(stored) => log::info!(
				"storage.local =\n{}",
				serde_json::to_string_pretty(&stored).unwrap_or_else(|err| err.to_string())
			),
			Err(err) => log::error!("dump_all failed: {err:?}"),
		}
	});
}

/// Removes only the harness's own keys.
#[wasm_bindgen]
pub fn wipe_harness() {
	wasm_bindgen_futures::spawn_local(async {
		let store = match load_harness_store().await {
			Ok(store) => store,
			Err(err) => return log::error!("wipe_harness failed: {err}"),
		};
		let keys: Vec<String> = store
			.records()
			.keys()
			.chain(store.unreadable().keys())
			.cloned()
			.collect();

		if keys.is_empty() {
			return log::info!("wipe_harness, nothing to remove");
		}
		match PlatformStorage::default().remove(&keys).await {
			Ok(()) => log::info!("wipe_harness removed {}", keys.len()),
			Err(err) => log::error!("wipe_harness failed: {err:?}"),
		}
	});
}
