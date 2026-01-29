use regex::Regex;

/// Extracts tracking-number-like strings from arbitrary text.
/// This is intentionally carrier-agnostic.
pub fn extract_candidates(text: &str) -> Vec<String> {
    // Broad heuristic:
    // - Starts and ends with alphanumeric
    // - Allows internal spaces or dashes
    // - Length between 10 and 34 characters (after trimming)
    //
    // NOTE: We extract raw matches first, normalize later.
    let re = Regex::new(
        r"\b[A-Z0-9][A-Z0-9\- ]{12,32}[A-Z0-9]\b"
    ).expect("invalid tracking regex");

    re.find_iter(&text.to_uppercase())
        .map(|m| m.as_str().trim().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_basic_tracking_numbers() {
        let text = "Your tracking number is 1Z999AA10123456784.";
        let result = extract_candidates(text);

        assert_eq!(result, vec!["1Z999AA10123456784"]);
    }

    #[test]
    fn handles_spaces_and_dashes() {
        let text = "USPS: 9400 1000 0000 0000 0000 00";
        let result = extract_candidates(text);

        assert_eq!(result, vec!["9400 1000 0000 0000 0000 00"]);
    }

    #[test]
    fn extracts_multiple_candidates() {
        let text = r#"
            First package: 1Z999AA10123456784
            Second package: JD014600003828392837
        "#;

        let result = extract_candidates(text);

        assert_eq!(
            result,
            vec![
                "1Z999AA10123456784",
                "JD014600003828392837"
            ]
        );
    }

    #[test]
    fn ignores_short_numbers() {
        let text = "Order #12345 has shipped";
        let result = extract_candidates(text);

        assert!(result.is_empty());
    }

    #[test]
    fn ignores_most_phone_numbers() {
        let text = "Call me at 555-123-4567";
        let result = extract_candidates(text);

        assert!(result.is_empty());
    }
}
