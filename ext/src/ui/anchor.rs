//! Where the editor popup is rendered, relative to its badge.

const GAP_FROM_ANCHOR: f64 = 6.0;
const VIEWPORT_MARGIN: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
	pub left: f64,
	pub top: f64,
	pub width: f64,
	pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportSize {
	pub width: f64,
	pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopoverSize {
	pub width: f64,
	pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlacementSide {
	Below,
	Above,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
	pub left: f64,
	pub top: f64,
	pub side: PlacementSide,
	pub max_height: f64,
}

pub fn place_popover(anchor: &Rect, popover: &PopoverSize, viewport: &ViewportSize) -> Placement {
	let anchor_bottom = anchor.top + anchor.height;
	let room_below = (viewport.height - VIEWPORT_MARGIN - anchor_bottom - GAP_FROM_ANCHOR).max(0.0);
	let room_above = (anchor.top - GAP_FROM_ANCHOR - VIEWPORT_MARGIN).max(0.0);

	let side = if popover.height <= room_below {
		PlacementSide::Below
	} else if popover.height <= room_above || room_above > room_below {
		PlacementSide::Above
	} else {
		PlacementSide::Below
	};
	let (unclamped_top, max_height) = match side {
		PlacementSide::Below => (anchor_bottom + GAP_FROM_ANCHOR, room_below),
		PlacementSide::Above => {
			let shown_height = popover.height.min(room_above);
			(anchor.top - GAP_FROM_ANCHOR - shown_height, room_above)
		}
	};

	let right_aligned_left = anchor.left + anchor.width - popover.width;
	let left = right_aligned_left
		.min(viewport.width - VIEWPORT_MARGIN - popover.width)
		.max(VIEWPORT_MARGIN);

	Placement {
		left,
		top: unclamped_top.max(VIEWPORT_MARGIN),
		side,
		max_height,
	}
}
