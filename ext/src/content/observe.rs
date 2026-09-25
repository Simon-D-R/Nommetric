use crate::content::scan;
use crate::current_restaurant_slug;
use gloo_timers::callback::Timeout;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::Closure;
use web_sys::{MutationObserver, MutationObserverInit, Node};

const DEBOUNCE_MS: u32 = 150;

thread_local! {
	static DEBOUNCE: RefCell<Option<Timeout>> = const { RefCell::new(None) };
}

pub fn install(target: &Node, on_restaurant_change: impl Fn(String) + 'static) {
	let on_restaurant_change = Rc::new(on_restaurant_change);
	let batch = Closure::wrap(
		Box::new(move |_records: js_sys::Array, _observer: JsValue| {
			let on_restaurant_change = Rc::clone(&on_restaurant_change);
			let tick = Timeout::new(DEBOUNCE_MS, move || {
				match changed_restaurant() {
					Some(slug) => on_restaurant_change(slug),
					None => scan::request_scan(),
				};
			});
			DEBOUNCE.with_borrow_mut(|pending| *pending = Some(tick));
		}) as Box<dyn FnMut(js_sys::Array, JsValue)>,
	);

	match MutationObserver::new(batch.as_ref().unchecked_ref()) {
		Err(err) => log::error!("no MutationObserver: {:?}", err),
		Ok(observer) => {
			let options = MutationObserverInit::new();
			options.set_child_list(true);
			options.set_subtree(true);
			match observer.observe_with_options(target, &options) {
				Ok(()) => batch.forget(),
				Err(err) => log::error!("observer refused the menu container: {:?}", err),
			}
		}
	}
}

fn changed_restaurant() -> Option<String> {
	let slug = current_restaurant_slug()?;
	let known = scan::installed_restaurant()?;
	(slug != known).then_some(slug)
}
