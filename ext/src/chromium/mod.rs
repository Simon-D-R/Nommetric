//! chrome.storage.local access, using web-extensions-sys.

use crate::browser::{BrowserRuntime, BrowserStorage, BrowserStorageError, js_error_message};
use gloo_utils::format::JsValueSerdeExt;
use std::cell::Cell;
use wasm_bindgen::prelude::*;

#[derive(Default)]
pub struct ChromiumStorage;

impl BrowserStorage for ChromiumStorage {
	async fn get_all(&self) -> Result<serde_json::Value, BrowserStorageError> {
		still_connected()?;
		let stored = local_storage()
			.get(&JsValue::NULL)
			.await
			.map_err(classify_error)?;
		value_from_js(&stored)
	}

	async fn insert(
		&self,
		stored_key: &str,
		value: &serde_json::Value,
	) -> Result<(), BrowserStorageError> {
		still_connected()?;
		let items = js_sys::Object::new();
		js_sys::Reflect::set(&items, &JsValue::from_str(stored_key), &value_to_js(value)?)
			.expect("a fresh object accepts a property"); // Only fails on exotic targets.
		local_storage()
			.set(items.as_ref())
			.await
			.map_err(classify_error)?;
		Ok(())
	}

	async fn remove(&self, keys: &[String]) -> Result<(), BrowserStorageError> {
		still_connected()?;
		let list = js_sys::Array::new();
		for key in keys {
			list.push(&JsValue::from_str(key));
		}
		local_storage()
			.remove(list.as_ref())
			.await
			.map_err(classify_error)?;
		Ok(())
	}

	fn on_changed(&self, mut handler: impl FnMut(&str, Option<&serde_json::Value>) + 'static) {
		let listener = Closure::wrap(Box::new(move |changes: JsValue, area: JsValue| {
			if area.as_string().as_deref() != Some("local") {
				return;
			}
			match value_from_js(&changes) {
				Ok(changes) => {
					if let Some(changed_keys) = changes.as_object() {
						for (stored_key, change) in changed_keys {
							handler(stored_key, change.get("newValue"));
						}
					}
				}
				Err(err) => log::error!("unreadable change payload: {err:?}"),
			}
		}) as Box<dyn FnMut(JsValue, JsValue)>);

		web_extensions_sys::chrome()
			.storage()
			.on_changed()
			.add_listener(listener.as_ref().unchecked_ref()); // Closure is always a JS function.
		listener.forget();
	}
}

#[wasm_bindgen]
extern "C" {
	#[wasm_bindgen(js_namespace = ["chrome", "runtime"], js_name = getURL)]
	fn runtime_get_url(path: &str) -> String;
}

#[derive(Default)]
pub struct ChromiumRuntime;

impl BrowserRuntime for ChromiumRuntime {
	fn resource_url(&self, path: &str) -> String {
		runtime_get_url(path)
	}
}

thread_local! {
	static CONTEXT_INVALIDATED: Cell<bool> = const { Cell::new(false) };
}

fn local_storage() -> web_extensions_sys::StorageArea {
	web_extensions_sys::chrome().storage().local()
}

fn still_connected() -> Result<(), BrowserStorageError> {
	if CONTEXT_INVALIDATED.get() {
		return Err(BrowserStorageError::ContextInvalidated);
	}
	Ok(())
}

fn classify_error(thrown: JsValue) -> BrowserStorageError {
	let message = js_error_message(&thrown);
	if message.contains("Extension context invalidated") {
		if !CONTEXT_INVALIDATED.replace(true) {
			log::error!("extension context invalidated, reload the page");
		}
		return BrowserStorageError::ContextInvalidated;
	}
	BrowserStorageError::Browser(message)
}

fn value_to_js(value: &serde_json::Value) -> Result<JsValue, BrowserStorageError> {
	JsValue::from_serde(value).map_err(|err| BrowserStorageError::Json(err.to_string()))
}

fn value_from_js(value: &JsValue) -> Result<serde_json::Value, BrowserStorageError> {
	value
		.into_serde()
		.map_err(|err| BrowserStorageError::Json(err.to_string()))
}
