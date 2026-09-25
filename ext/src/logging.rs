use log::{Level, LevelFilter, Log, Metadata, Record};
use wasm_bindgen::JsValue;

static CONSOLE: ConsoleLogger = ConsoleLogger;

struct ConsoleLogger;

impl Log for ConsoleLogger {
	fn enabled(&self, metadata: &Metadata) -> bool {
		metadata.level() <= log::max_level()
	}

	fn log(&self, record: &Record) {
		if !self.enabled(record.metadata()) {
			return;
		}
		let line = JsValue::from_str(&format!("nommetric: {}", record.args()));
		match record.level() {
			Level::Error => web_sys::console::error_1(&line),
			Level::Warn => web_sys::console::warn_1(&line),
			Level::Info => web_sys::console::log_1(&line),
			Level::Debug | Level::Trace => web_sys::console::debug_1(&line),
		}
	}

	fn flush(&self) {}
}

pub fn init() {
	if log::set_logger(&CONSOLE).is_ok() {
		log::set_max_level(if cfg!(debug_assertions) {
			LevelFilter::Debug
		} else {
			LevelFilter::Info
		});
	}
}
