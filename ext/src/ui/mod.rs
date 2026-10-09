pub mod anchor;
pub mod popover;
pub mod tokens;

use wasm_bindgen::JsCast as _;
use web_sys::{Document, HtmlElement};

pub fn create_element(document: &Document, tag: &str) -> Option<HtmlElement> {
	document
		.create_element(tag)
		.ok()?
		.dyn_into::<HtmlElement>()
		.ok()
}
