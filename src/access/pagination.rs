#[derive(Debug, Clone)]
pub struct Cursor {
    pub offset: u64,
}

#[derive(Debug, Clone)]
/// Pagination options
pub struct PageQuery {
    /// Limit of the page size
    pub limit: usize,
    /// Cursor value to start from
    pub cursor: Option<Cursor>,
}

impl Default for PageQuery {
    fn default() -> Self {
        PageQuery {
            limit: 100,
            cursor: None,
        }
    }
}

#[derive(Debug, Clone)]
/// Result of the query
pub struct PageResult<T> {
    /// Found items
    pub values: Vec<T>,
    /// Cursor to start next page, or None if finished
    pub cursor: Option<Cursor>,
}

