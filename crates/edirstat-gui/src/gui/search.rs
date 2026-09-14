//! Filter syntax: name tokens, globs (`*.iso`), and size operators (`<100m`, `a>=1g`).

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchQuery {
    pub name: String,
    pub glob: Option<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub min_allocated: Option<u64>,
    pub max_allocated: Option<u64>,
}

impl SearchQuery {
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        let mut query = Self::default();
        let mut name_parts = Vec::new();
        for token in raw.split_whitespace() {
            if let Some(glob) = token.strip_prefix("*.") {
                if !glob.is_empty() && !glob.contains('/') {
                    query.glob = Some(glob.to_ascii_lowercase());
                    continue;
                }
            }
            if let Some((field, op, rest)) = split_size_token(token) {
                if let Some(bytes) = parse_size_literal(rest) {
                    match (field, op) {
                        (SizeField::Size, SizeOp::Lt) => query.max_size = Some(bytes.saturating_sub(1)),
                        (SizeField::Size, SizeOp::Le) => query.max_size = Some(bytes),
                        (SizeField::Size, SizeOp::Gt) => query.min_size = Some(bytes.saturating_add(1)),
                        (SizeField::Size, SizeOp::Ge) => query.min_size = Some(bytes),
                        (SizeField::Size, SizeOp::Eq) => {
                            query.min_size = Some(bytes);
                            query.max_size = Some(bytes);
                        }
                        (SizeField::Allocated, SizeOp::Lt) => {
                            query.max_allocated = Some(bytes.saturating_sub(1));
                        }
                        (SizeField::Allocated, SizeOp::Le) => query.max_allocated = Some(bytes),
                        (SizeField::Allocated, SizeOp::Gt) => {
                            query.min_allocated = Some(bytes.saturating_add(1));
                        }
                        (SizeField::Allocated, SizeOp::Ge) => query.min_allocated = Some(bytes),
                        (SizeField::Allocated, SizeOp::Eq) => {
                            query.min_allocated = Some(bytes);
                            query.max_allocated = Some(bytes);
                        }
                    }
                    continue;
                }
            }
            name_parts.push(token);
        }
        query.name = name_parts.join(" ");
        query
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
            && self.glob.is_none()
            && self.min_size.is_none()
            && self.max_size.is_none()
            && self.min_allocated.is_none()
            && self.max_allocated.is_none()
    }

    #[must_use]
    pub fn matches_sizes(&self, size: u64, allocated: u64) -> bool {
        if self.min_size.is_some_and(|m| size < m) {
            return false;
        }
        if self.max_size.is_some_and(|m| size > m) {
            return false;
        }
        if self.min_allocated.is_some_and(|m| allocated < m) {
            return false;
        }
        if self.max_allocated.is_some_and(|m| allocated > m) {
            return false;
        }
        true
    }

    #[must_use]
    pub fn matches_name(&self, name: &str, case_sensitive: bool) -> bool {
        if let Some(ext) = &self.glob {
            let hay = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
            if hay != *ext {
                return false;
            }
        }
        if self.name.is_empty() {
            return true;
        }
        if case_sensitive {
            name.contains(&self.name)
        } else {
            crate::arena::contains_case_insensitive(name, &self.name.to_lowercase())
        }
    }
}

#[derive(Clone, Copy)]
enum SizeField {
    Size,
    Allocated,
}

#[derive(Clone, Copy)]
enum SizeOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
}

fn split_size_token(token: &str) -> Option<(SizeField, SizeOp, &str)> {
    let (field, rest) = if let Some(rest) = token.strip_prefix("a") {
        if rest.starts_with(['<', '>', '=']) {
            (SizeField::Allocated, rest)
        } else {
            return None;
        }
    } else {
        (SizeField::Size, token)
    };
    let (op, rest) = if let Some(rest) = rest.strip_prefix("<=") {
        (SizeOp::Le, rest)
    } else if let Some(rest) = rest.strip_prefix(">=") {
        (SizeOp::Ge, rest)
    } else if let Some(rest) = rest.strip_prefix('<') {
        (SizeOp::Lt, rest)
    } else if let Some(rest) = rest.strip_prefix('>') {
        (SizeOp::Gt, rest)
    } else if let Some(rest) = rest.strip_prefix('=') {
        (SizeOp::Eq, rest)
    } else {
        return None;
    };
    Some((field, op, rest))
}

fn parse_size_literal(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (num, mul) = match raw.as_bytes().last().copied().map(|b| b.to_ascii_lowercase()) {
        Some(b'k') => (&raw[..raw.len() - 1], 1024u64),
        Some(b'm') => (&raw[..raw.len() - 1], 1024 * 1024),
        Some(b'g') => (&raw[..raw.len() - 1], 1024 * 1024 * 1024),
        Some(b't') => (&raw[..raw.len() - 1], 1024 * 1024 * 1024 * 1024),
        _ => (raw, 1u64),
    };
    let value: u64 = num.parse().ok()?;
    value.checked_mul(mul)
}

#[cfg(test)]
mod tests {
    use super::SearchQuery;

    #[test]
    fn test_parse_glob_and_size() {
        let q = SearchQuery::parse("*.iso <100m a>=1g");
        assert_eq!(q.glob.as_deref(), Some("iso"));
        assert_eq!(q.max_size, Some(100 * 1024 * 1024 - 1));
        assert_eq!(q.min_allocated, Some(1024 * 1024 * 1024));
        assert!(q.name.is_empty());
    }

    #[test]
    fn test_parse_name_and_eq() {
        let q = SearchQuery::parse("report =10k");
        assert_eq!(q.name, "report");
        assert_eq!(q.min_size, Some(10 * 1024));
        assert_eq!(q.max_size, Some(10 * 1024));
    }
}
