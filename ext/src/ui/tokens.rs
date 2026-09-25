use std::cell::OnceCell;
use web_sys::{CssStyleSheet, ShadowRoot};

const BADGE_CSS: &str = include_str!("badge.css");

thread_local! {
	static SHEET: OnceCell<CssStyleSheet> = const { OnceCell::new() };
}

pub fn adopt(root: &ShadowRoot) {
	SHEET.with(|cell| {
		let sheets = js_sys::Array::new();
		sheets.push(cell.get_or_init(build_sheet));
		root.set_adopted_style_sheets(&sheets);
	});
}

fn build_sheet() -> CssStyleSheet {
	let sheet = CssStyleSheet::new().expect("failed to create stylesheet");
	if let Err(err) = sheet.replace_sync(BADGE_CSS) {
		log::error!("badge stylesheet rejected: {:?}", err);
	}
	sheet
}
