use crate::ui::popover::{self, EditorSession};
use crate::ui::{create_element, tokens};
use gloo_events::{EventListener, EventListenerOptions};
use nommetric_core::key::{ItemIdentity, ItemKey};
use nommetric_core::rating::Rating;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast as _;
use web_sys::{Element, Event, HtmlElement, KeyboardEvent};

pub const CARD_ATTRIBUTE: &str = "data-nommetric";
const HOST_ATTRIBUTE: &str = "data-nommetric-host";
const THEME_ATTRIBUTE: &str = "data-nm-theme";

#[derive(Clone)]
pub enum BadgeState {
	Rated { key: ItemKey, record: Box<Rating> },
	Unrated,
	ReadOnly { reason: ReadOnlyReason },
}

#[derive(Clone)]
pub enum ReadOnlyReason {
	SameCategoryDuplicate,
	UnreadableRecord,
}

pub struct BadgeMount {
	card: Element,
	host: HtmlElement,
	button: HtmlElement,
	identity: ItemIdentity,
	shown_state: Rc<RefCell<BadgeState>>,
	_listeners: [EventListener; 3],
}

impl BadgeMount {
	pub fn attach(
		card: &Element,
		mount: &HtmlElement,
		identity: ItemIdentity,
		restaurant_slug: &str,
		state: &BadgeState,
	) -> Option<BadgeMount> {
		let document = card.owner_document()?;
		card.set_attribute(CARD_ATTRIBUTE, "1").ok()?;

		mount.style().set_property("position", "relative").ok()?;

		let host = create_element(&document, "span")?;
		host.set_attribute(HOST_ATTRIBUTE, "1").ok()?;

		let init = web_sys::ShadowRootInit::new(web_sys::ShadowRootMode::Open);
		let root = host.attach_shadow(&init).ok()?;
		tokens::adopt_badge_sheets(&root);

		let button = create_element(&document, "button")?;
		button.set_attribute("type", "button").ok()?;
		root.append_child(&button).ok()?;
		mount.append_child(&host).ok()?;

		let shown_state = Rc::new(RefCell::new(state.clone()));
		let open_editor = {
			let shown_state = shown_state.clone();
			let button = button.clone();
			let identity = identity.clone();
			let restaurant_slug = restaurant_slug.to_string();
			EventListener::new_with_options(
				&host,
				"click",
				EventListenerOptions::enable_prevent_default(),
				move |event| {
					event.stop_propagation();
					event.prevent_default();
					let open = |shown| {
						popover::toggle_editor(
							EditorSession {
								identity: identity.clone(),
								restaurant_slug: restaurant_slug.clone(),
								shown,
							},
							&button,
						)
					};
					let state = shown_state.borrow().clone();
					match state {
						BadgeState::Rated { key, record } => open(Some((key, *record))),
						BadgeState::Unrated => open(None),
						BadgeState::ReadOnly { .. } => {}
					}
				},
			)
		};
		// A button activates on keydown for Enter but on keyup for Space; React sees neither.
		let [isolate_keydown, isolate_keyup] = ["keydown", "keyup"].map(|event_type| {
			EventListener::new(&host, event_type, |event: &Event| {
				if event
					.dyn_ref::<KeyboardEvent>()
					.is_some_and(|key_event| matches!(key_event.key().as_str(), "Enter" | " "))
				{
					event.stop_propagation();
				}
			})
		});

		let badge = BadgeMount {
			card: card.clone(),
			host,
			button,
			identity,
			shown_state,
			_listeners: [open_editor, isolate_keydown, isolate_keyup],
		};
		badge.update(state);
		Some(badge)
	}

	pub fn card(&self) -> &Element {
		&self.card
	}

	pub fn identity(&self) -> &ItemIdentity {
		&self.identity
	}

	pub fn is_still_attached(&self) -> bool {
		self.host.is_connected() && self.card.contains(Some(&self.host))
	}

	pub fn set_theme(&self, theme: &str) {
		let _ = self.host.set_attribute(THEME_ATTRIBUTE, theme);
	}

	pub fn update(&self, state: &BadgeState) {
		*self.shown_state.borrow_mut() = state.clone();
		let (class, glyph, label) = describe(state);
		self.button.set_class_name(&format!("badge {}", class));
		self.button.set_text_content(Some(&glyph));
		let _ = self.button.set_attribute("aria-label", &label);

		match state {
			BadgeState::ReadOnly { .. } => {
				let _ = self.button.set_attribute("tabindex", "-1");
				let _ = self.button.set_attribute("aria-disabled", "true");
				let _ = self.button.remove_attribute("aria-haspopup");
				let _ = self.button.set_attribute("title", &label);
			}
			BadgeState::Rated { .. } | BadgeState::Unrated => {
				let _ = self.button.remove_attribute("tabindex");
				let _ = self.button.remove_attribute("aria-disabled");
				let _ = self.button.set_attribute("aria-haspopup", "dialog");
				match state {
					BadgeState::Rated { record, .. } if !record.comment.trim().is_empty() => {
						let _ = self.button.set_attribute("title", record.comment.trim());
					}
					_ => {
						let _ = self.button.remove_attribute("title");
					}
				}
			}
		}
	}

	pub fn remove(self) {
		self.host.remove();
		let _ = self.card.remove_attribute(CARD_ATTRIBUTE);
	}
}

fn describe(state: &BadgeState) -> (&'static str, String, String) {
	match state {
		BadgeState::Rated{record, ..} => {
			match **record {
				Rating { score: Some(score), ..} => (
					"rated",
					score.to_string(),
					format!("Your score: {}, edit", score),
				),
				_ => (// A cleared score can leave a surviving comment
					"rated",
					"·".to_string(),
					"Commented, no score, edit".to_string(),
				)
			}
		},
		BadgeState::Unrated => ("unrated", "+".to_string(), "Not rated, rate".to_string()),
		BadgeState::ReadOnly {
			reason: ReadOnlyReason::SameCategoryDuplicate,
		} => (
			"read-only",
			"–".to_string(),
			"Two dishes in this section share this name and the site doesn't expose an id that separates them, so this one can't be rated.".to_string(),
		),
		BadgeState::ReadOnly {
			reason: ReadOnlyReason::UnreadableRecord,
		} => (
			"read-only",
			"–".to_string(),
			"This dish has a saved rating that this version can't read, so it is left untouched."
				.to_string(),
		),
	}
}
