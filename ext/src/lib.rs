use nommetric_core::Url;
use nommetric_core::slug::restaurant_slug;
use wasm_bindgen::prelude::*;

fn log(msg: &str) {
	web_sys::console::log_1(&JsValue::from_str(msg));
}

#[wasm_bindgen]
pub fn start_content() {
	console_error_panic_hook::set_once();

	let href = web_sys::window()
		.and_then(|w| w.location().href().ok())
		.unwrap_or_default();

	match Url::parse(&href)
		.ok()
		.and_then(|parsed| restaurant_slug(&parsed))
	{
		Some(slug) => log(&format!("nommetric: start_content ok: slug={slug}")),
		None => log("nommetric: start_content ok: no restaurant slug, idle"),
	}
}

#[wasm_bindgen]
pub fn start_options() {
	console_error_panic_hook::set_once();
	log("nommetric: start_options ok");
}
