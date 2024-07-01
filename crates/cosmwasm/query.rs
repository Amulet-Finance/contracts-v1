pub const DEFAULT_PAGE_LIMIT: u32 = 10;

/// Determine the page bounds to be used in a paginated query.
/// Returns `Some((start, end))` or `None` if the proposed start is out of bounds.
/// If no `limit` is provided but a `page` is, the `DEFAULT_PAGE_LIMIT` is used.
pub fn page_bounds(total_count: u32, page: Option<u32>, limit: Option<u32>) -> Option<(u32, u32)> {
    if page.is_none() && limit.is_none() {
        return Some((0, total_count));
    }

    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);

    let start = page.map_or(0, |p| p * limit);

    // out of bounds
    if start >= total_count {
        return None;
    }

    let end = total_count.min(start + limit);

    Some((start, end))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn page_bounds_work() {
        let test_cases = [
            // no limits given, full range returned
            (100, None, None, Some((0, 100))),
            // page given, default limit used
            (100, Some(1), None, Some((10, 20))),
            // page and limit given
            (100, Some(1), Some(20), Some((20, 40))),
            // out of bounds start (valid range is 0-99)
            (100, Some(1), Some(100), None),
            // on the last page, the limit is ignored
            (55, Some(5), None, Some((50, 55))),
        ];

        for (total_count, page, limit, expect) in test_cases {
            assert_eq!(page_bounds(total_count, page, limit), expect);
        }
    }
}
