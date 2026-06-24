// Redaction: удаление секретов из строк перед записью в лог.

/// Применяет все redaction-правила к строке.
pub fn redact(input: &str) -> String {
    let mut result = input.to_string();

    // 1. Authorization: Bearer <token>
    let re = regex::Regex::new(r"(?i)(Authorization:\s*Bearer\s+)[^\s,;]+").unwrap();
    result = re.replace_all(&result, "$1[REDACTED]").to_string();

    // 2. api_key, api_secret, apikey
    let re = regex::Regex::new(r"(?i)(api_key|api_secret|apikey)\s*[=:](\s*)[^\s,;&]+").unwrap();
    result = re.replace_all(&result, "$1=$2[REDACTED]").to_string();

    // 3. token, secret, password (в параметрах)
    let re = regex::Regex::new(r"(?i)(token|secret|password)\s*[=:](\s*)[^\s,;&]+").unwrap();
    result = re.replace_all(&result, "$1=$2[REDACTED]").to_string();

    // 4. SSH private key в stdout
    let re = regex::Regex::new(
        r"-----BEGIN\s+(RSA|DSA|EC|OPENSSH)\s+PRIVATE\s+KEY-----[\s\S]*?-----END\s+(RSA|DSA|EC|OPENSSH)\s+PRIVATE\s+KEY-----",
    )
    .unwrap();
    result = re.replace_all(&result, "[SSH KEY REDACTED]").to_string();

    // 5. GitHub token в URL (https://token@github.com/...)
    let re = regex::Regex::new(r"https?://[^@/:]+:[^@]+@").unwrap();
    result = re.replace_all(&result, "https://[REDACTED]@").to_string();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_bearer_token() {
        let input = "Authorization: Bearer sk-1234567890abcdef";
        let result = redact(input);
        assert!(result.contains("[REDACTED]"));
        assert!(!result.contains("sk-1234567890abcdef"));
    }

    #[test]
    fn test_redact_api_key() {
        let input = "api_key=my-secret-key-123";
        let result = redact(input);
        assert!(result.contains("[REDACTED]"));
        assert!(!result.contains("my-secret-key-123"));
    }

    #[test]
    fn test_redact_api_key_colon() {
        let input = "api_key: my-secret-key-123";
        let result = redact(input);
        assert!(result.contains("[REDACTED]"));
    }

    #[test]
    fn test_redact_password() {
        let input = "password=super_secret!";
        let result = redact(input);
        assert!(result.contains("[REDACTED]"));
        assert!(!result.contains("super_secret!"));
    }

    #[test]
    fn test_redact_ssh_key() {
        let input = "some text\n-----BEGIN RSA PRIVATE KEY-----\nbase64data123\n-----END RSA PRIVATE KEY-----\nmore text";
        let result = redact(input);
        assert!(result.contains("[SSH KEY REDACTED]"));
        assert!(!result.contains("base64data123"));
    }

    #[test]
    fn test_redact_github_token_url() {
        let input = "https://x-access-token:ghp_abc123@github.com/user/repo.git";
        let result = redact(input);
        assert!(result.contains("[REDACTED]"));
        assert!(!result.contains("ghp_abc123"));
    }

    #[test]
    fn test_redact_clean_text_unchanged() {
        let input = "hello world ls -la";
        assert_eq!(redact(input), input);
    }

    #[test]
    fn test_redact_empty() {
        assert_eq!(redact(""), "");
    }
}
