//! Page sizes and page maths for paged library views (constitution IV; 005 research R1).
//!
//! Scenes are shown a page at a time. A page is fetched by number at one of Stash's page sizes
//! plus our default of 50 (Stash paginates with `LIMIT/OFFSET`, and deep pages cost the same as the
//! first).

use crate::error::AppError;

/// The page sizes offered: Stash's, plus 50.
pub const PAGE_SIZES: [u32; 8] = [20, 40, 50, 60, 120, 250, 500, 1000];
/// Every new tab starts here.
pub const DEFAULT_PAGE_SIZE: u32 = 50;

/// Only the listed sizes are accepted.
pub fn validate_page_size(size: u32) -> Result<(), AppError> {
    if PAGE_SIZES.contains(&size) {
        Ok(())
    } else {
        Err(AppError::Internal {
            message: format!("page size {size} isn't one of {PAGE_SIZES:?}"),
        })
    }
}

/// The 1-based page holding item `index` (0-based).
pub fn page_of(index: u32, size: u32) -> u32 {
    index / size.max(1) + 1
}

/// How many pages hold `count` items.
pub fn page_count(count: u32, size: u32) -> u32 {
    count.div_ceil(size.max(1))
}

/// The last page that exists (page 1 for an empty library).
pub fn last_page(count: u32, size: u32) -> u32 {
    page_count(count, size).max(1)
}
