use std::cell::OnceCell;
use web_sys::{CssStyleSheet, ShadowRoot};

pub const TOKENS_CSS: &str = include_str!("tokens.css");
pub const SCORE_CSS: &str = include_str!("score.css");
const BADGE_CSS: &str = concat!(include_str!("badge.css"), include_str!("score.css"));
const POPOVER_CSS: &str = include_str!("popover.css");

struct StyleSheets {
	tokens: CssStyleSheet,
	badge: CssStyleSheet,
	popover: CssStyleSheet,
}

thread_local! {
	static SHEETS: OnceCell<StyleSheets> = const { OnceCell::new() };
}

pub fn adopt_badge_sheets(root: &ShadowRoot) {
	adopt(root, |sheets| [&sheets.tokens, &sheets.badge]);
}

pub fn adopt_popover_sheets(root: &ShadowRoot) {
	adopt(root, |sheets| [&sheets.tokens, &sheets.popover]);
}

fn adopt(root: &ShadowRoot, pick: impl Fn(&StyleSheets) -> [&CssStyleSheet; 2]) {
	SHEETS.with(|cell| {
		let adopted = js_sys::Array::new();
		for sheet in pick(cell.get_or_init(build_sheets)) {
			adopted.push(sheet);
		}
		root.set_adopted_style_sheets(&adopted);
	});
}

fn build_sheets() -> StyleSheets {
	StyleSheets {
		tokens: build_sheet("tokens", TOKENS_CSS),
		badge: build_sheet("badge", BADGE_CSS),
		popover: build_sheet("popover", POPOVER_CSS),
	}
}

fn build_sheet(sheet_name: &str, css: &str) -> CssStyleSheet {
	let sheet = CssStyleSheet::new().expect("failed to create stylesheet");
	if let Err(err) = sheet.replace_sync(css) {
		log::error!("{} stylesheet rejected: {:?}", sheet_name, err);
	}
	sheet
}
