//! The browser API ports.

#[derive(Debug, Clone, PartialEq)]
pub enum BrowserStorageError {
	ContextInvalidated,
	Browser(String),
	Json(String),
}

pub trait BrowserStorage {
	async fn get_all(&self) -> Result<serde_json::Value, BrowserStorageError>;

	async fn insert(
		&self,
		stored_key: &str,
		value: &serde_json::Value,
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
