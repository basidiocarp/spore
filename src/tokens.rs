//! Token estimation utilities for tracking LLM token consumption.
//!
//! Provides a fast approximation of token count using the ~4 chars = 1 token
//! heuristic. Used by mycelium for savings tracking and hyphae for memory sizing.

// ─────────────────────────────────────────────────────────────────────────────
// Estimate Tokens
// ─────────────────────────────────────────────────────────────────────────────

/// Estimate token count from text using the ~4 chars = 1 token heuristic.
///
/// This is a fast approximation suitable for tracking purposes.
/// For precise counts, integrate with your LLM's tokenizer API.
///
/// # Formula
///
/// `tokens = ceil(bytes / 4)`
///
/// # Examples
///
/// ```
/// use spore::tokens::estimate;
///
/// assert_eq!(estimate(""), 0);
/// assert_eq!(estimate("abcd"), 1);   // 4 chars = 1 token
/// assert_eq!(estimate("abcde"), 2);  // 5 chars = ceil(1.25) = 2
/// assert_eq!(estimate("hello world"), 3); // 11 chars = ceil(2.75) = 3
/// ```
#[must_use]
pub fn estimate(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Calculate token savings percentage between original and filtered text.
///
/// Returns a value between 0.0 and 100.0. Returns 0.0 if the original is empty.
///
/// # Examples
///
/// ```
/// use spore::tokens::savings_percent;
///
/// assert_eq!(savings_percent("12345678", "12"), 50.0); // 1 token / 2 tokens = 50% remaining = 50% saved
/// assert_eq!(savings_percent("", ""), 0.0);
/// ```
#[must_use]
pub fn savings_percent(original: &str, filtered: &str) -> f64 {
    let orig_tokens = estimate(original);
    if orig_tokens == 0 {
        return 0.0;
    }
    let filt_tokens = estimate(filtered);
    #[allow(
        clippy::cast_precision_loss,
        reason = "token counts are well within f64 precision"
    )]
    let pct = filt_tokens as f64 / orig_tokens as f64 * 100.0;
    100.0 - pct
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_empty() {
        assert_eq!(estimate(""), 0);
    }

    #[test]
    fn test_estimate_exact_boundary() {
        assert_eq!(estimate("abcd"), 1);
        assert_eq!(estimate("abcdefgh"), 2);
    }

    #[test]
    fn test_estimate_rounds_up() {
        assert_eq!(estimate("a"), 1);
        assert_eq!(estimate("ab"), 1);
        assert_eq!(estimate("abc"), 1);
        assert_eq!(estimate("abcde"), 2);
    }

    #[test]
    fn test_estimate_longer_text() {
        // "hello world" = 11 bytes → ceil(11/4) = 3
        assert_eq!(estimate("hello world"), 3);
    }

    #[test]
    fn test_savings_percent_75() {
        // 8 bytes → 2 tokens, 2 bytes → 1 token, savings = 50%
        let pct = savings_percent("12345678", "12");
        assert!((pct - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_savings_percent_zero_original() {
        assert!((savings_percent("", "anything") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_savings_percent_empty_filtered() {
        assert!((savings_percent("some text", "") - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_savings_percent_no_savings() {
        let text = "hello";
        assert!((savings_percent(text, text) - 0.0).abs() < f64::EPSILON);
    }
}
