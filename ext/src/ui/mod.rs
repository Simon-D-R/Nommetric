pub mod anchor;
pub mod options;
pub mod popover;
pub mod tokens;

use nommetric_core::rating::RATING_SCALE;
use wasm_bindgen::JsCast as _;
use web_sys::{Document, HtmlElement};

pub fn score_band_class(score: u8) -> &'static str {
	let position = f64::from(score.saturating_sub(RATING_SCALE.min))
		/ f64::from(RATING_SCALE.max - RATING_SCALE.min);
	if position < 0.3333333333333334 {
		"score-low"
	} else if position < 0.6666666666666667 {
		"score-mid"
	} else {
		"score-high"
	}
}

pub fn create_element(document: &Document, tag: &str) -> Option<HtmlElement> {
	document
		.create_element(tag)
		.ok()?
		.dyn_into::<HtmlElement>()
		.ok()
}
