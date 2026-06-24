// Request Matcher: нормализация + exact normalized match.

/// Нормализует запрос: lowercase, trim, collapse whitespace.
pub fn normalize(request: &str) -> String {
    let trimmed = request.trim().to_lowercase();
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_was_space = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }
    result.trim().to_string()
}

/// Хеш нормализованного запроса (SHA-256 hex).
pub fn hash_normalized(normalized: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_lowercase() {
        assert_eq!(normalize("List Files"), "list files");
    }

    #[test]
    fn test_normalize_trim() {
        assert_eq!(normalize("  hello world  "), "hello world");
    }

    #[test]
    fn test_normalize_collapse_whitespace() {
        assert_eq!(normalize("hello    world"), "hello world");
    }

    #[test]
    fn test_normalize_tabs_newlines() {
        assert_eq!(normalize("hello\n\tworld"), "hello world");
    }

    #[test]
    fn test_normalize_empty() {
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn test_hash_consistency() {
        let h1 = hash_normalized("list files");
        let h2 = hash_normalized("list files");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_hash_differs() {
        let h1 = hash_normalized("list files");
        let h2 = hash_normalized("list dirs");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_normalize_then_hash() {
        let h = hash_normalized(&normalize("  List    Files  "));
        assert_eq!(h, hash_normalized("list files"));
    }
}
