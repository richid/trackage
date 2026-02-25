use regex::Regex;
use tracking_numbers::{track, TrackingResult};

/// Extracts tracking-number-like strings from arbitrary text.
/// This is intentionally carrier-agnostic.
pub fn extract_candidates(text: &str) -> Vec<String> {
    let uppercased = text.to_uppercase();
    let mut results = Vec::new();

    // Pattern 1: contiguous alphanumeric (most carriers)
    let re_contiguous =
        Regex::new(r"\b[A-Z0-9]{12,34}\b").expect("invalid tracking regex");

    // Pattern 2: space-separated digit groups, e.g. USPS "9400 1000 0000 0000 0000 00"
    let re_spaced =
        Regex::new(r"\b\d{2,4}(?: \d{2,4}){3,}\b").expect("invalid spaced tracking regex");

    let mut seen = std::collections::HashSet::new();

    for m in re_contiguous.find_iter(&uppercased) {
        let s = m.as_str().to_string();
        if s.chars().any(|c| c.is_ascii_digit()) && seen.insert(s.clone()) {
            results.push(s);
        }
    }

    for m in re_spaced.find_iter(&uppercased) {
        let s = m.as_str().to_string();
        if seen.insert(s.clone()) {
            results.push(s);
        }
    }

    results
}

/// Extracts candidate strings from text, validates each with the
/// tracking-numbers crate, and returns only confirmed tracking numbers.
pub fn extract_tracking_numbers(text: &str) -> Vec<TrackingResult> {
    let mut seen = std::collections::HashSet::new();
    extract_candidates(text)
        .into_iter()
        .filter_map(|candidate| {
            let cleaned: String = candidate.chars().filter(|c| !c.is_whitespace()).collect();
            track(&cleaned)
        })
        .filter(|result| seen.insert(result.tracking_number.clone()))
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

        assert_eq!(result, vec!["1Z999AA10123456784", "JD014600003828392837"]);
    }

    #[test]
    fn extracts_12_digit_fedex_number() {
        let text = "Your order has shipped. Here is your tracking info FEDEX 986578788855.";
        let result = extract_candidates(text);

        assert_eq!(result, vec!["986578788855"]);
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

    #[test]
    fn validates_real_tracking_numbers() {
        let text = "Your package: 1Z5R89390357567127 is on its way";
        let results = extract_tracking_numbers(text);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tracking_number, "1Z5R89390357567127");
        assert!(!results[0].courier.is_empty());
    }

    #[test]
    fn rejects_candidates_that_fail_validation() {
        let text = "Reference: ABCDEFGHIJKLMNOP";
        let results = extract_tracking_numbers(text);

        assert!(results.is_empty());
    }
}
