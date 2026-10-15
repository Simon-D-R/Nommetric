//! The browser API ports.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum BrowserStorageError {
	ContextInvalidated,
	Browser(String),
	Json(String),
}

impl std::fmt::Display for BrowserStorageError {
	fn fmt(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			BrowserStorageError::ContextInvalidated => {
				write!(out, "the extension was reloaded; reload this page")
			}
			BrowserStorageError::Browser(message) | BrowserStorageError::Json(message) => {
				write!(out, "{}", message)
			}
		}
	}
}

pub trait BrowserStorage {
	async fn get_all(&self) -> Result<serde_json::Value, BrowserStorageError>;

	async fn insert(
		&self,
		stored_key: &str,
		value: &serde_json::Value,
	) -> Result<(), BrowserStorageError> {
		self.insert_many(&BTreeMap::from([(stored_key.to_string(), value.clone())]))
			.await
	}

	async fn insert_many(
		&self,
		values: &BTreeMap<String, serde_json::Value>,
	) -> Result<(), BrowserStorageError>;

	async fn remove(&self, keys: &[String]) -> Result<(), BrowserStorageError>;

	fn on_changed(&self, handler: impl FnMut(&str, Option<&serde_json::Value>) + 'static);
}

pub trait BrowserRuntime {
	fn resource_url(&self, path: &str) -> String;
}

pub fn js_error_message(js_error: &wasm_bindgen::JsValue) -> String {
	js_error
		.as_string()
		.or_else(|| {
			js_sys::Reflect::get(js_error, &wasm_bindgen::JsValue::from_str("message"))
				.ok()
				.and_then(|message| message.as_string())
		})
		.unwrap_or_else(|| format!("{js_error:?}"))
}

#[cfg(not(feature = "firefox"))]
pub type PlatformStorage = crate::chromium::ChromiumStorage;

#[cfg(not(feature = "firefox"))]
pub type PlatformRuntime = crate::chromium::ChromiumRuntime;
