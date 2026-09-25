use crate::ui::tokens;
use nommetric_core::key::ItemIdentity;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;
use web_sys::{Document, Element, Event, HtmlElement};

pub const CARD_ATTRIBUTE: &str = "data-nommetric";
const HOST_ATTRIBUTE: &str = "data-nommetric-host";
const THEME_ATTRIBUTE: &str = "data-nm-theme";

pub enum BadgeState {
	Rated { score: Option<u8>, comment: String },
	Unrated,
	ReadOnly { reason: ReadOnlyReason },
}

pub enum ReadOnlyReason {
	SameCategoryDuplicate,
	UnreadableRecord,
}

pub struct BadgeMount {
	card: Element,
	host: HtmlElement,
	button: HtmlElement,
	identity: ItemIdentity,
	swallow_click: Closure<dyn FnMut(Event)>,
}

impl BadgeMount {
	pub fn attach(
		card: &Element,
		mount: &HtmlElement,
		identity: ItemIdentity,
		state: &BadgeState,
	) -> Option<BadgeMount> {
		let document = card.owner_document()?;
		card.set_attribute(CARD_ATTRIBUTE, "1").ok()?;

		mount.style().set_property("position", "relative").ok()?;

		let host = create_element(&document, "span")?;
		host.set_attribute(HOST_ATTRIBUTE, "1").ok()?;

		let init = web_sys::ShadowRootInit::new(web_sys::ShadowRootMode::Open);
		let root = host.attach_shadow(&init).ok()?;
		tokens::adopt(&root);

		let button = create_element(&document, "button")?;
		button.set_attribute("type", "button").ok()?;
		button.set_attribute("tabindex", "-1").ok()?;
		root.append_child(&button).ok()?;
		mount.append_child(&host).ok()?;

		let swallow_click = Closure::wrap(Box::new(|event: Event| {
			event.stop_propagation();
			event.prevent_default();
		}) as Box<dyn FnMut(Event)>);
		host.add_event_listener_with_callback("click", swallow_click.as_ref().unchecked_ref())
			.ok()?;

		let badge = BadgeMount {
			card: card.clone(),
			host,
			button,
			identity,
			swallow_click,
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
		let (class, glyph, label) = describe(state);
		self.button.set_class_name(&format!("badge {}", class));
		self.button.set_text_content(Some(&glyph));
		let _ = self.button.set_attribute("aria-label", &label);

		match state {
			BadgeState::Rated { comment, .. } if !comment.trim().is_empty() => {
				let _ = self.button.set_attribute("title", comment.trim());
			}
			_ => {
				let _ = self.button.remove_attribute("title");
			}
		}
	}

	pub fn remove(self) {
		let _ = self.host.remove_event_listener_with_callback(
			"click",
			self.swallow_click.as_ref().unchecked_ref(),
		);
		self.host.remove();
		let _ = self.card.remove_attribute(CARD_ATTRIBUTE);
	}
}

fn create_element(document: &Document, tag: &str) -> Option<HtmlElement> {
	document
		.create_element(tag)
		.ok()?
		.dyn_into::<HtmlElement>()
		.ok()
}

fn describe(state: &BadgeState) -> (&'static str, String, String) {
	match state {
		BadgeState::Rated {
			score: Some(score), ..
		} => (
			"rated",
			score.to_string(),
			format!("Your score: {}, edit", score),
		),
		// A cleared score can leave a surviving comment; falling through to Unrated
		// loses the only cue that a note exists.
		BadgeState::Rated { score: None, .. } => (
			"rated",
			"·".to_string(),
			"Commented, no score, edit".to_string(),
		),
		BadgeState::Unrated => ("unrated", "–".to_string(), "Not rated".to_string()),
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
