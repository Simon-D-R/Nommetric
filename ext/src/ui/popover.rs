//! The one score and comment editor, anchored to whichever badge opened it.

use crate::browser::PlatformStorage;
use crate::site::deliveroo::declared_theme;
use crate::store::{StoreError, write_edit};
use crate::ui::anchor::{PlacementSide, PopoverSize, Rect, ViewportSize, place_popover};
use crate::ui::{create_element, tokens};
use gloo_events::{EventListener, EventListenerOptions};
use leptos::html;
use leptos::prelude::{
	AriaAttributes as _, ClassAttribute as _, Effect, ElementChild as _, For, Get as _,
	GetUntracked as _, GlobalAttributes as _, IntoView, NodeRef, NodeRefAttribute as _,
	OnAttribute as _, PropAttribute as _, RwSignal, Set as _, StoredValue, WithValue as _,
	component, event_target_value, view,
};
use nommetric_core::edit::{DishEdit, EditRefusal, plan_edit};
use nommetric_core::key::{ItemIdentity, ItemKey};
use nommetric_core::rating::{RATING_SCALE, Rating};
use std::any::Any;
use std::cell::{Cell, OnceCell, RefCell};
use wasm_bindgen::JsCast as _;
use web_sys::{Event, HtmlElement, KeyboardEvent};

const HEADING_ID: &str = "nm-editor-heading";

#[derive(Clone)]
pub struct EditorSession {
	pub identity: ItemIdentity,
	pub restaurant_slug: String,
	pub shown: Option<(ItemKey, Rating)>,
}

struct EditorPopover {
	host: HtmlElement,
	wrapper: HtmlElement,
	mounted_form: RefCell<Option<Box<dyn Any>>>,
	_key_isolation: [EventListener; 3],
}

thread_local! {
	static EDITOR: OnceCell<EditorPopover> = const { OnceCell::new() };
	static ANCHOR: RefCell<Option<HtmlElement>> = const { RefCell::new(None) };
	static SESSION_LISTENERS: RefCell<Vec<EventListener>> = const { RefCell::new(Vec::new()) };
	static SESSION_GENERATION: Cell<u64> = const { Cell::new(0) };
}

pub fn toggle_editor(session: EditorSession, anchor: &HtmlElement) {
	let open_on_this_anchor = ANCHOR.with_borrow(|open_anchor| {
		open_anchor
			.as_ref()
			.is_some_and(|open_anchor| open_anchor.is_same_node(Some(anchor)))
	});
	if open_on_this_anchor {
		close_editor(true);
		return;
	}

	let started_ms = js_sys::Date::now();
	EDITOR.with(|cell| {
		let first_open = cell.get().is_none();
		if first_open {
			match create_editor() {
				Some(editor) => {
					let _ = cell.set(editor);
				}
				None => log::error!("the editor popover could not be created"),
			}
		}
		if let Some(editor) = cell.get() {
			SESSION_GENERATION.set(SESSION_GENERATION.get() + 1);
			ANCHOR.set(Some(anchor.clone()));
			let _ = editor
				.host
				.set_attribute("data-nm-theme", &declared_theme(&gloo_utils::document()));

			// Dropping the previous form unmounts it. Closing only hides it, because Save
			// and Cancel close from inside the form's own handlers.
			editor.mounted_form.replace(None);
			let mounted = leptos::mount::mount_to(editor.wrapper.clone(), move || {
				view! { <EditorForm session=session /> }
			});
			editor.mounted_form.replace(Some(Box::new(mounted)));

			if !editor.host.matches(":popover-open").unwrap_or(false) {
				let _ = editor.host.show_popover();
			}
			let host = editor.host.clone();
			SESSION_LISTENERS.set(vec![
				EventListener::new_with_options(
					&gloo_utils::document(),
					"pointerdown",
					EventListenerOptions::run_in_capture_phase(),
					move |event| {
						let inside_anchor = ANCHOR.with_borrow(|anchor| {
							anchor
								.as_ref()
								.is_some_and(|anchor| event_path_contains(event, anchor))
						});
						if !inside_anchor && !event_path_contains(event, &host) {
							close_editor(false);
						}
					},
				),
				EventListener::new_with_options(
					&gloo_utils::document(),
					"keydown",
					EventListenerOptions::run_in_capture_phase(),
					|event| {
						if event
							.dyn_ref::<KeyboardEvent>()
							.is_some_and(|key_event| key_event.key() == "Escape")
						{
							close_editor(true);
						}
					},
				),
				EventListener::new_with_options(
					&gloo_utils::window(),
					"scroll",
					EventListenerOptions::run_in_capture_phase(),
					|_event| follow_anchor(),
				),
				EventListener::new(&gloo_utils::window(), "resize", |_event| follow_anchor()),
			]);
			place(editor, anchor);

			if first_open {
				log::debug!(
					"editor first open took {:.0} ms",
					js_sys::Date::now() - started_ms
				);
			}
		}
	});
}

pub fn close_editor(restore_focus: bool) {
	SESSION_GENERATION.set(SESSION_GENERATION.get() + 1);
	let dismissal_listeners = SESSION_LISTENERS.take();
	let anchor = ANCHOR.take();
	EDITOR.with(|cell| {
		if let Some(editor) = cell.get() {
			let _ = editor.host.hide_popover();
		}
	});
	if restore_focus && let Some(anchor) = &anchor {
		focus_without_scrolling(anchor);
	}
	drop(dismissal_listeners);
}

fn create_editor() -> Option<EditorPopover> {
	let document = gloo_utils::document();
	let host = create_element(&document, "div")?;
	host.set_popover(Some("manual")).ok()?;
	// Inline, because the page's own rules on `div` or `[popover]` beat `:host`.
	host.set_attribute(
		"style",
		"position: fixed; inset: auto; top: 0; left: 0; width: clamp(240px, 24%, 400px); \
		 margin: 0; padding: 0; border: 0; background: transparent; overflow: visible;",
	)
	.ok()?;

	let init = web_sys::ShadowRootInit::new(web_sys::ShadowRootMode::Open);
	let root = host.attach_shadow(&init).ok()?;
	tokens::adopt_popover_sheets(&root);

	let wrapper = create_element(&document, "div")?;
	wrapper.set_class_name("editor");
	wrapper.set_attribute("role", "dialog").ok()?;
	wrapper.set_attribute("aria-labelledby", HEADING_ID).ok()?;
	root.append_child(&wrapper).ok()?;
	document.body()?.append_child(&host).ok()?;

	let key_isolation = ["keydown", "keyup", "keypress"]
		.map(|event_type| EventListener::new(&host, event_type, Event::stop_propagation));

	Some(EditorPopover {
		host,
		wrapper,
		mounted_form: RefCell::new(None),
		_key_isolation: key_isolation,
	})
}

fn event_path_contains(event: &Event, element: &HtmlElement) -> bool {
	event.composed_path().includes(element, 0)
}

fn follow_anchor() {
	let anchor = ANCHOR.with_borrow(Clone::clone);
	if let Some(anchor) = anchor {
		if !anchor.is_connected() {
			close_editor(false);
			return;
		}
		EDITOR.with(|cell| {
			if let Some(editor) = cell.get() {
				place(editor, &anchor);
			}
		});
	}
}

fn place(editor: &EditorPopover, anchor: &HtmlElement) {
	if let Some(root) = gloo_utils::document().document_element() {
		let viewport = ViewportSize {
			width: f64::from(root.client_width()),
			height: f64::from(root.client_height()),
		};
		let anchor_box = anchor.get_bounding_client_rect();
		let wrapper_style = editor.wrapper.style();

		// Not `visibility: hidden`: Chromium moves focus off an invisible element.
		let anchor_visible = anchor_box.bottom() > 0.0
			&& anchor_box.top() < viewport.height
			&& anchor_box.right() > 0.0
			&& anchor_box.left() < viewport.width;
		if !anchor_visible {
			let _ = wrapper_style.set_property("opacity", "0");
			let _ = wrapper_style.set_property("pointer-events", "none");
			return;
		}
		let _ = wrapper_style.remove_property("opacity");
		let _ = wrapper_style.remove_property("pointer-events");

		let border_height =
			f64::from(editor.wrapper.offset_height() - editor.wrapper.client_height());
		let placement = place_popover(
			&Rect {
				left: anchor_box.left(),
				top: anchor_box.top(),
				width: anchor_box.width(),
				height: anchor_box.height(),
			},
			&PopoverSize {
				width: editor.host.get_bounding_client_rect().width(),
				height: f64::from(editor.wrapper.scroll_height()) + border_height,
			},
			&viewport,
		);

		let host_style = editor.host.style();
		let _ = host_style.set_property("top", &format!("{}px", placement.top));
		let _ = host_style.set_property("left", &format!("{}px", placement.left));
		let _ = wrapper_style.set_property("max-height", &format!("{}px", placement.max_height));
		let side = match placement.side {
			PlacementSide::Below => "below",
			PlacementSide::Above => "above",
		};
		let _ = editor.wrapper.set_attribute("data-side", side);
	}
}

fn focus_without_scrolling(element: &HtmlElement) {
	let options = web_sys::FocusOptions::new();
	options.set_prevent_scroll(true);
	let _ = element.focus_with_options(&options);
}

#[component]
fn EditorForm(session: EditorSession) -> impl IntoView {
	let shown_record = session.shown.as_ref().map(|(_key, record)| record);
	let score = RwSignal::new(shown_record.and_then(|record| record.score));
	let comment = RwSignal::new(
		shown_record
			.map(|record| record.comment.clone())
			.unwrap_or_default(),
	);
	let saving = RwSignal::new(false);
	let status = RwSignal::new(String::new());
	let dish_name = session.identity.dish_name.clone();
	let session = StoredValue::new(session);

	// Slider position 0 is "no score"; position n is the n-th value on the scale.
	let score_values: Vec<Option<u8>> = std::iter::once(None)
		.chain(
			(RATING_SCALE.min..=RATING_SCALE.max)
				.step_by(usize::from(RATING_SCALE.step))
				.map(Some),
		)
		.collect();
	let last_position = score_values.len() - 1;
	let score_values = StoredValue::new(score_values);
	let slider_position = move || {
		score_values.with_value(|values| {
			values
				.iter()
				.position(|value| *value == score.get())
				.unwrap_or(0)
		})
	};

	let slider = NodeRef::<html::Input>::new();
	Effect::new(move || {
		if let Some(slider) = slider.get() {
			focus_without_scrolling(&slider);
		}
	});

	let save = move || {
		let edit = DishEdit {
			score: score.get_untracked(),
			comment: comment.get_untracked(),
		};
		session.with_value(|session| {
			let shown = session.shown.as_ref().map(|(key, record)| (key, record));
			match plan_edit(
				&session.identity,
				&session.restaurant_slug,
				shown,
				&edit,
				js_sys::Date::now(),
			) {
				Err(EditRefusal::SameCategoryDuplicate) => status.set(
					"two dishes in this section share this name, so this one can't be rated"
						.to_string(),
				),
				Err(EditRefusal::ScoreOutOfRange(score)) => status.set(format!(
					"{} is outside the {}–{} scale",
					score, RATING_SCALE.min, RATING_SCALE.max
				)),
				Ok(write) if write.is_empty() => close_editor(true),
				Ok(write) => {
					saving.set(true);
					status.set(String::new());
					let generation = SESSION_GENERATION.get();
					let dish_name = session.identity.dish_name.clone();
					wasm_bindgen_futures::spawn_local(async move {
						let result = write_edit(&PlatformStorage::default(), &write).await;
						if SESSION_GENERATION.get() != generation {
							if let Err(err) = result {
								log::warn!(
									"saving {:?} failed after its editor closed: {:?}",
									dish_name,
									err
								);
							}
							return;
						}
						match result {
							// The badge repaints from onChanged, like a save from another tab.
							Ok(()) => close_editor(true),
							Err(err) => {
								saving.set(false);
								let reason = match err {
									StoreError::Storage(storage_error) => storage_error.to_string(),
									StoreError::Json(message) => message,
									other => format!("{other:?}"),
								};
								status.set(format!("Couldn't save: {reason}"));
							}
						}
					});
				}
			}
		});
	};

	view! {
		<div on:keydown=move |event: KeyboardEvent| {
			if event.key() == "Enter" && (event.ctrl_key() || event.meta_key()) {
				event.prevent_default();
				save();
			}
		}>
			<h2 id=HEADING_ID class="heading">
				{dish_name}
			</h2>
			<div class="score">
				<input
					node_ref=slider
					type="range"
					min="0"
					max=last_position
					step="1"
					aria-label="Score"
					aria-valuetext=move || {
						score.get().map_or("No score".to_string(), |value| value.to_string())
					}
					class:unscored=move || score.get().is_none()
					prop:value=move || slider_position().to_string()
					on:input=move |event| {
						let position = event_target_value(&event).parse::<usize>().unwrap_or(0);
						score
							.set(
								score_values
									.with_value(|values| values.get(position).copied().flatten()),
							);
					}
				/>
				<div class="ticks" aria-hidden="true">
					<For each=move || 0..=last_position key=|position| *position let:_position>
						<span></span>
					</For>
				</div>
				<div class="tick-labels" aria-hidden="true">
					<For each=move || 0..=last_position key=|position| *position let:position>
						<span>
							{score_values
								.with_value(|values| {
									values[position]
										.map_or("–".to_string(), |value| value.to_string())
								})}
						</span>
					</For>
				</div>
			</div>
			<textarea
				aria-label="Comment"
				prop:value=comment.get_untracked()
				on:input=move |event| comment.set(event_target_value(&event))
			></textarea>
			<div class="actions">
				<button
					type="button"
					class="save"
					disabled=move || saving.get()
					on:click=move |_event| save()
				>
					"Save"
				</button>
				<button type="button" on:click=move |_event| close_editor(true)>
					"Cancel"
				</button>
			</div>
			<p class="status" aria-live="polite">
				{move || status.get()}
			</p>
		</div>
	}
}
