//! Page sizes and page maths (005 research R1; data-model "Page size").

use stash_core::scenes::paging::{
    last_page, page_count, page_of, validate_page_size, DEFAULT_PAGE_SIZE, PAGE_SIZES,
};

#[test]
fn the_page_sizes_are_stashs_plus_50_and_50_is_the_default() {
    assert_eq!(PAGE_SIZES, [20, 40, 50, 60, 120, 250, 500, 1000]);
    assert_eq!(DEFAULT_PAGE_SIZE, 50);
}

#[test]
fn only_the_listed_sizes_are_accepted() {
    for size in PAGE_SIZES {
        assert!(validate_page_size(size).is_ok(), "{size}");
    }
    for size in [0, 1, 55, 100, 2000] {
        assert!(validate_page_size(size).is_err(), "{size}");
    }
}

#[test]
fn pages_depend_on_the_size() {
    assert_eq!(page_count(36_350, 50), 727);
    assert_eq!(page_count(36_350, 1000), 37);
    assert_eq!(page_count(50, 50), 1);
    assert_eq!(page_count(51, 50), 2);
    assert_eq!(page_count(0, 50), 0);
    // 0-based index → 1-based page.
    assert_eq!(page_of(0, 50), 1);
    assert_eq!(page_of(49, 50), 1);
    assert_eq!(page_of(1_250, 50), 26);
    assert_eq!(page_of(1_849, 120), 16);
}

#[test]
fn the_last_page_is_never_zero() {
    assert_eq!(last_page(36_350, 50), 727);
    assert_eq!(last_page(120, 50), 3);
    assert_eq!(last_page(0, 50), 1, "an empty library still has page 1");
}
