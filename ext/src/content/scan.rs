use crate::content::badge::{BadgeMount, BadgeState, CARD_ATTRIBUTE, ReadOnlyReason};
use crate::site::deliveroo::{CardScope, DeliverooMenu};
use crate::store::StoredRestaurantRatings;
use nommetric_core::census::DishCensus;
use nommetric_core::key::{ItemIdentity, ItemKey};
use nommetric_core::lookup_key::dish_name_to_lookup_key;
use std::cell::{Cell, RefCell};
use web_sys::{Document, Element};

thread_local! {
	static SCAN: RefCell<Option<MenuScan>> = const { RefCell::new(None) };
	static SCAN_IN_FLIGHT: Cell<bool> = const { Cell::new(false) };
	static RESCAN_REQUESTED: Cell<bool> = const { Cell::new(false) };
	static CHANGES_DURING_SCAN: RefCell<Vec<(String, Option<serde_json::Value>)>> =
		const { RefCell::new(Vec::new()) };
	static INSTALLED_RESTAURANT: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub struct MenuScan {
	menu: DeliverooMenu,
	census: DishCensus,
	store: StoredRestaurantRatings,
	mounts: Vec<BadgeMount>,
	restaurant_slug: String,
	timed: bool,
}

#[derive(Default)]
pub struct ScanReport {
	census_cards: usize,
	census_skipped: usize,
	mount_cards: usize,
	mounted_total: usize,
	mounted_new: usize,
	unmountable_cards: usize,
	with_item_id: usize,
	name_keyed: usize,
	read_only: usize,
	promoted: usize,
}

impl std::fmt::Display for ScanReport {
	fn fmt(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(
			out,
			"census_cards: {}\n census_skipped: {}\n mount_cards: {}\n mounted_total: {}\n mounted_new: {}\n unmountable_cards: {}\n with_item_id: {}\n name_keyed: {}\n read_only: {}\n promoted: {}",
			self.census_cards,
			self.census_skipped,
			self.mount_cards,
			self.mounted_total,
			self.mounted_new,
			self.unmountable_cards,
			self.with_item_id,
			self.name_keyed,
			self.read_only,
			self.promoted
		)
	}
}

struct CensusCard {
	card: Element,
	category_id: String,
	dish_lookup_key: String,
}

impl MenuScan {
	pub fn new(
		menu: DeliverooMenu,
		store: StoredRestaurantRatings,
		restaurant_slug: String,
	) -> MenuScan {
		MenuScan {
			menu,
			census: DishCensus::default(),
			store,
			mounts: Vec::new(),
			restaurant_slug,
			timed: false,
		}
	}

	pub async fn run(&mut self, document: &Document) -> ScanReport {
		let started_ms = js_sys::Date::now();
		let mut report = ScanReport::default();

		let census_cards = self.census_pass(document, &mut report);
		self.menu.warn_if_index_empty(census_cards.len());
		let newly_unseparable = self.census.observe_pass(
			census_cards
				.iter()
				.map(|seen| (seen.dish_lookup_key.as_str(), seen.category_id.as_str())),
		);

		self.drop_stale_mounts();
		self.mount_pass(document, &census_cards, &mut report).await;

		let theme = self.menu.declared_theme(document);
		for mount in &self.mounts {
			mount.set_theme(&theme);
		}
		self.repaint_matching(|identity| newly_unseparable.contains(&identity.dish_lookup_key));

		report.mounted_total = self.mounts.len();
		if !self.timed {
			self.timed = true;
			log::debug!("scan took {:.0} ms", js_sys::Date::now() - started_ms);
		}
		report
	}

	fn census_pass(&self, document: &Document, report: &mut ScanReport) -> Vec<CensusCard> {
		let mut seen = Vec::new();
		for card in self.menu.find_item_cards(document, CardScope::Census) {
			report.census_cards += 1;
			let named = self.menu.dish_name(&card);
			let category = self.menu.category_ancestor_id(&card);
			match (named, category) {
				(Some(dish_name), Some(category_id)) => seen.push(CensusCard {
					card,
					category_id,
					dish_lookup_key: dish_name_to_lookup_key(&dish_name),
				}),
				(dish_name, category_id) => {
					report.census_skipped += 1;
					log::warn!(
						"census card skipped: name {:?}, category {:?}",
						dish_name,
						category_id
					);
				}
			}
		}
		seen
	}

	fn drop_stale_mounts(&mut self) {
		for mount in std::mem::take(&mut self.mounts) {
			match mount.is_still_attached() {
				true => self.mounts.push(mount),
				false => mount.remove(),
			}
		}
	}

	async fn mount_pass(
		&mut self,
		document: &Document,
		census_cards: &[CensusCard],
		report: &mut ScanReport,
	) {
		for (index, card) in self
			.menu
			.find_item_cards(document, CardScope::Mount)
			.into_iter()
			.enumerate()
		{
			report.mount_cards += 1;
			if self
				.mounts
				.iter()
				.any(|mount| mount.card().is_same_node(Some(&card)))
			{
				continue;
			}
			let _ = card.remove_attribute(CARD_ATTRIBUTE);

			let identity = match self.identify(&card, census_cards) {
				Some(identity) => identity,
				None => {
					report.unmountable_cards += 1;
					continue;
				}
			};
			if identity.item_id.is_some() {
				report.with_item_id += 1;
			} else {
				report.name_keyed += 1;
			}

			let state = self.resolve(&identity, report).await;
			if matches!(state, BadgeState::ReadOnly { .. }) {
				report.read_only += 1;
			}
			log::debug!(
				"card {}, {}, {}, id {:?}, {}",
				index,
				identity.category_id,
				identity.dish_lookup_key,
				identity.item_id,
				identity
					.candidate_keys(&self.restaurant_slug)
					.next()
					.expect(
						"somehow, candidate_keys is empty (should contain at least the name key)"
					)
					.encode()
			);

			match self.menu.mount_point(&card) {
				None => log::warn!("card {} has no mount point", index),
				Some(mount_point) => {
					match BadgeMount::attach(&card, &mount_point, identity, &state) {
						Some(mount) => {
							self.mounts.push(mount);
							report.mounted_new += 1;
						}
						None => log::warn!("card {} would not take a badge", index),
					}
				}
			}
		}
	}

	fn identify(&self, card: &Element, census_cards: &[CensusCard]) -> Option<ItemIdentity> {
		let dish_name = self.menu.dish_name(card)?;

		let own_entry = census_cards
			.iter()
			.find(|seen| seen.card.is_same_node(Some(card)));
		let (category_id, dish_lookup_key) = match own_entry {
			Some(seen) => (seen.category_id.clone(), seen.dish_lookup_key.clone()),
			None => {
				let dish_lookup_key = dish_name_to_lookup_key(&dish_name);
				let category_id = self.census.sole_category(&dish_lookup_key)?.to_string();
				(category_id, dish_lookup_key)
			}
		};

		Some(ItemIdentity {
			category_id,
			item_id: self.menu.item_id(&dish_lookup_key),
			dish_name,
			same_category_duplicate: self.census.has_same_category_duplicate(&dish_lookup_key),
			dish_lookup_key,
		})
	}

	async fn resolve(&mut self, identity: &ItemIdentity, report: &mut ScanReport) -> BadgeState {
		let read_from_name_key = matches!(
			self.store.get(identity),
			Some((ItemKey::Name { .. }, _record))
		);
		if read_from_name_key && identity.item_id.is_some() {
			match self.store.promote(identity).await {
				Ok(Some(_)) => report.promoted += 1,
				Ok(None) => {}
				Err(err) => log::warn!("promotion failed for {:?}: {:?}", identity.dish_name, err),
			}
		}
		self.state_for(identity)
	}

	fn state_for(&self, identity: &ItemIdentity) -> BadgeState {
		if self.store.is_unreadable(identity) {
			return BadgeState::ReadOnly {
				reason: ReadOnlyReason::UnreadableRecord,
			};
		}

		if identity.same_category_duplicate {
			return BadgeState::ReadOnly {
				reason: ReadOnlyReason::SameCategoryDuplicate,
			};
		}
		match self.store.get(identity) {
			Some((_key, record)) => BadgeState::Rated {
				score: record.score,
				comment: record.comment.clone(),
			},
			None => BadgeState::Unrated,
		}
	}

	pub fn repaint_matching(&self, matches: impl Fn(&ItemIdentity) -> bool) {
		for mount in &self.mounts {
			if matches(mount.identity()) {
				mount.update(&self.state_for(mount.identity()));
			}
		}
	}

	fn apply_change(&mut self, stored_key: &str, new_value: Option<&serde_json::Value>) {
		let outcome = self.store.apply_change(stored_key, new_value);
		log::debug!("onChanged {} -> {:?}", stored_key, outcome);
		match ItemKey::decode(stored_key) {
			None => {}
			Some(changed) => self.repaint_matching(|identity| {
				identity
					.candidate_keys(&self.restaurant_slug)
					.any(|candidate| candidate == changed)
			}),
		}
	}

	pub fn unmount_all(&mut self) {
		for mount in std::mem::take(&mut self.mounts) {
			mount.remove();
		}
	}
}

pub fn install(scan: MenuScan) {
	INSTALLED_RESTAURANT.with_borrow_mut(|slug| *slug = Some(scan.restaurant_slug.clone()));
	SCAN.with_borrow_mut(|slot| *slot = Some(scan));
}

pub fn reset() {
	INSTALLED_RESTAURANT.with_borrow_mut(|slug| *slug = None);
	SCAN.with_borrow_mut(|slot| {
		if let Some(mut scan) = slot.take() {
			scan.unmount_all();
		}
	});
}

pub fn installed_restaurant() -> Option<String> {
	INSTALLED_RESTAURANT.with_borrow(Clone::clone)
}

pub fn on_storage_change(stored_key: &str, new_value: Option<&serde_json::Value>) {
	SCAN.with_borrow_mut(|slot| match slot.as_mut() {
		Some(scan) => scan.apply_change(stored_key, new_value),
		None => CHANGES_DURING_SCAN
			.with_borrow_mut(|pending| pending.push((stored_key.to_string(), new_value.cloned()))),
	});
}

pub fn request_scan() {
	if SCAN_IN_FLIGHT.get() {
		RESCAN_REQUESTED.set(true);
		return;
	}
	SCAN_IN_FLIGHT.set(true);
	wasm_bindgen_futures::spawn_local(async {
		loop {
			let taken = SCAN.with_borrow_mut(|slot| slot.take());
			let mut scan = match taken {
				Some(scan) => scan,
				None => break,
			};
			match document() {
				None => log::error!("no document to scan"),
				Some(document) => {
					log::info!("{}", scan.run(&document).await);
				}
			}
			let pending = CHANGES_DURING_SCAN.with_borrow_mut(std::mem::take);
			for (stored_key, new_value) in pending {
				scan.apply_change(&stored_key, new_value.as_ref());
			}
			SCAN.with_borrow_mut(|slot| *slot = Some(scan));

			if !RESCAN_REQUESTED.replace(false) {
				break;
			}
		}
		SCAN_IN_FLIGHT.set(false);
	});
}

fn document() -> Option<Document> {
	web_sys::window()?.document()
}
