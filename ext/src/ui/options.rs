//! The options page: stored ratings, with search, delete, export and import.

use crate::browser::{BrowserStorage as _, PlatformStorage, js_error_message};
use crate::ui::score_band_class;
use crate::ui::tokens::{SCORE_CSS, TOKENS_CSS};
use leptos::html;
use leptos::prelude::{
	ClassAttribute as _, ElementChild as _, For, Get as _, GetUntracked as _, IntoView, Memo,
	NodeRef, NodeRefAttribute as _, OnAttribute as _, RwSignal, Set as _, Update as _, With as _,
	WithUntracked as _, component, event_target_value, view,
};
use nommetric_core::backup::{export_backup, import_backup};
use nommetric_core::key::ItemKey;
use nommetric_core::lookup_key::dish_name_to_lookup_key;
use nommetric_core::rating::Rating;
use std::collections::BTreeMap;
use wasm_bindgen::JsValue;

#[derive(Clone, PartialEq)]
struct RatingRow {
	stored_key: String,
	restaurant_slug: String,
	dish_name: String,
	record: Result<Rating, String>,
	search_keys: [String; 3], // Restaurant, dish, comment
}

#[derive(Clone)]
struct DeletedRating {
	stored_key: String,
	value: serde_json::Value,
	dish_name: String,
}

#[component]
pub fn OptionsPage() -> impl IntoView {
	let stored = RwSignal::new(BTreeMap::<String, serde_json::Value>::new());
	let load_error = RwSignal::new(None::<String>);
	let query = RwSignal::new(String::new());
	let status = RwSignal::new(String::new());
	let last_deleted = RwSignal::new(None::<DeletedRating>);
	let download_link = NodeRef::<html::A>::new();
	let file_input = NodeRef::<html::Input>::new();

	PlatformStorage::default().on_changed(move |stored_key, new_value| {
		if ItemKey::decode(stored_key).is_some() {
			stored.update(|stored| match new_value {
				Some(value) => {
					stored.insert(stored_key.to_string(), value.clone());
				}
				None => {
					stored.remove(stored_key);
				}
			});
		}
	});
	wasm_bindgen_futures::spawn_local(async move {
		match PlatformStorage::default().get_all().await {
			Ok(serde_json::Value::Object(all_keys)) => stored.set(
				all_keys
					.into_iter()
					.filter(|(stored_key, _value)| ItemKey::decode(stored_key).is_some())
					.collect(),
			),
			Ok(_) => {}
			Err(err) => load_error.set(Some(err.to_string())),
		}
	});

	let all_rows = Memo::new(move |_| {
		let mut rows: Vec<RatingRow> = stored.with(|stored| {
			stored
				.iter()
				.map(|(stored_key, value)| {
					let record = Rating::load_value(value);
					let restaurant_slug = ItemKey::decode(stored_key)
						.map(|key| key.restaurant_slug().to_string())
						.unwrap_or_default();
					let dish_name = match &record {
						Ok(record) => record.dish_name.clone(),
						Err(_reason) => value
							.get("dish_name")
							.and_then(serde_json::Value::as_str)
							.unwrap_or(stored_key)
							.to_string(),
					};
					let comment = record.as_ref().map_or("", |record| &record.comment);
					RatingRow {
						stored_key: stored_key.clone(),
						search_keys: [&restaurant_slug, &dish_name, comment]
							.map(dish_name_to_lookup_key),
						restaurant_slug,
						dish_name,
						record,
					}
				})
				.collect()
		});
		rows.sort_by_cached_key(|row| {
			(
				row.restaurant_slug.clone(),
				dish_name_to_lookup_key(&row.dish_name),
			)
		});
		rows
	});
	let visible_rows = Memo::new(move |_| {
		let query = dish_name_to_lookup_key(&query.get());
		all_rows.with(|rows| {
			rows.iter()
				.filter(|row| row.search_keys.iter().any(|field| field.contains(&query)))
				.cloned()
				.collect::<Vec<_>>()
		})
	});

	let delete = move |stored_key: String, dish_name: String| {
		if let Some(value) = stored.with_untracked(|stored| stored.get(&stored_key).cloned()) {
			let deleted = DeletedRating {
				stored_key,
				value,
				dish_name,
			};
			wasm_bindgen_futures::spawn_local(async move {
				let removed = PlatformStorage::default()
					.remove(std::slice::from_ref(&deleted.stored_key))
					.await;
				match removed {
					Ok(()) => last_deleted.set(Some(deleted)),
					Err(err) => status.set(format!("Couldn't delete: {}", err)),
				}
			});
		}
	};
	let undo = move |_event| {
		if let Some(deleted) = last_deleted.get_untracked() {
			last_deleted.set(None);
			wasm_bindgen_futures::spawn_local(async move {
				let inserted = PlatformStorage::default()
					.insert(&deleted.stored_key, &deleted.value)
					.await;
				if let Err(err) = inserted {
					status.set(format!("Couldn't undo: {}", err));
				}
			});
		}
	};

	let export = move |_event| {
		wasm_bindgen_futures::spawn_local(async move {
			match PlatformStorage::default().get_all().await {
				Ok(all_keys) => {
					let now_ms = js_sys::Date::now();
					let backup_text =
						serde_json::to_string_pretty(&export_backup(&all_keys, now_ms))
							.expect("backup is not plain JSON");
					if let Some(link) = download_link.get_untracked() {
						link.set_href(&format!(
							"data:application/json;charset=utf-8,{}",
							js_sys::encode_uri_component(&backup_text)
						));
						link.set_download(&format!("nommetric-backup-{}.json", local_date(now_ms)));
						link.click();
					}
				}
				Err(err) => status.set(format!("Couldn't export: {}", err)),
			}
		});
	};
	let import = move |_event| {
		if let Some(input) = file_input.get_untracked() {
			let picked = input.files().and_then(|files| files.get(0));

			input.set_value(""); // So picking the same file again triggers change again
			if let Some(file) = picked {
				wasm_bindgen_futures::spawn_local(
					async move { status.set(import_file(file).await) },
				);
			}
		}
	};

	let empty_message = move || {
		if let Some(err) = load_error.get() {
			format!("Couldn't load ratings: {}", err)
		} else if stored.with(BTreeMap::is_empty) {
			"No ratings yet".to_string()
		} else if visible_rows.with(Vec::is_empty) {
			"Nothing matches".to_string()
		} else {
			String::new()
		}
	};

	view! {
		<style>{TOKENS_CSS}{include_str!("options.css")}{SCORE_CSS}</style>
		<header>
			<h1>"Nommetric"</h1>
			<input
				type="search"
				placeholder="Search restaurant, dish or comment"
				on:input=move |event| query.set(event_target_value(&event))
			/>
			<button type="button" on:click=export>
				"Export"
			</button>
			<label class="button">
				"Import"
				<input
					type="file"
					accept=".json,application/json"
					node_ref=file_input
					on:change=import
				/>
			</label>
			<a node_ref=download_link></a>
		</header>
		<p class="status">{move || status.get()}</p>
		{move || {
			last_deleted
				.get()
				.map(|deleted| {
					view! {
						<p class="undo">
							"Deleted " {deleted.dish_name} ". " <button type="button" on:click=undo>
								"Undo"
							</button>
						</p>
					}
				})
		}}
		<table>
			<thead>
				<tr>
					<th>"Restaurant"</th>
					<th>"Dish"</th>
					<th>"Score"</th>
					<th>"Comment"</th>
					<th>"Updated"</th>
					<th></th>
				</tr>
			</thead>
			<tbody>
				<For
					each=move || visible_rows.get()
					key=|row| {
						(
							row.stored_key.clone(),
							row.record.as_ref().map_or(0, |record| record.updated_at_ms.to_bits()),
						)
					}
					let:row
				>
					{
						let (score_class, score_text, comment, updated) = match &row.record {
							Ok(record) => {
								(
									record.score.map_or("commented", score_band_class),
									record
										.score
										.map_or("·".to_string(), |score| score.to_string()),
									record.comment.clone(),
									local_date(record.updated_at_ms),
								)
							}
							Err(reason) => {
								(
									"unreadable",
									"/".to_string(),
									format!("This version can't read this record: {}", reason),
									String::new(),
								)
							}
						};
						let stored_key = row.stored_key;
						let dish_name = row.dish_name.clone();
						view! {
							<tr>
								<td>{row.restaurant_slug}</td>
								<td>{row.dish_name}</td>
								<td>
									<span class=format!("chip {}", score_class)>{score_text}</span>
								</td>
								<td class="comment">{comment}</td>
								<td>{updated}</td>
								<td>
									<button
										type="button"
										on:click=move |_event| delete(
											stored_key.clone(),
											dish_name.clone(),
										)
									>
										"Delete"
									</button>
								</td>
							</tr>
						}
					}
				</For>
			</tbody>
		</table>
		<p class="empty">{empty_message}</p>
	}
}

async fn import_file(file: web_sys::File) -> String {
	let backup_text = match wasm_bindgen_futures::JsFuture::from(file.text()).await {
		Ok(text) => text.as_string().unwrap_or_default(),
		Err(err) => return format!("Couldn't read the file: {}", js_error_message(&err)),
	};
	let storage = PlatformStorage::default();
	let all_keys = match storage.get_all().await {
		Ok(all_keys) => all_keys,
		Err(err) => return format!("Not imported: {}", err),
	};
	match import_backup(&backup_text, &all_keys) {
		Err(reason) => format!("Not imported: {}", reason),
		Ok(writes) if writes.is_empty() => {
			"Nothing to import: every rating in the file is already here.".to_string()
		}
		Ok(writes) => match storage.insert_many(&writes).await {
			Ok(()) => format!("Imported {} rating(s).", writes.len()),
			Err(err) => format!("Not imported: {}", err),
		},
	}
}

fn local_date(ms: f64) -> String {
	let date = js_sys::Date::new(&JsValue::from_f64(ms));
	format!(
		"{:04}-{:02}-{:02}",
		date.get_full_year(),
		date.get_month() + 1,
		date.get_date()
	)
}
