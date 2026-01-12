//! Text metrics: word counting, CJK detection, punctuation analysis

/// Count text metrics (word-like tokens, CJK characters, whitespace)
pub fn count_text_metrics(text: &str) -> (usize, usize, usize) {
    let mut word_count = 0;
    let mut cjk_count = 0;
    let mut whitespace_count = 0;
    let mut in_word = false;

    for c in text.chars() {
        if c.is_whitespace() {
            whitespace_count += 1;
            if in_word {
                word_count += 1;
                in_word = false;
            }
        } else if c.is_alphanumeric() {
            in_word = true;

            // Check for CJK
            if is_cjk(c) {
                cjk_count += 1;
            }
        } else {
            if in_word {
                word_count += 1;
                in_word = false;
            }
        }
    }

    if in_word {
        word_count += 1;
    }

    (word_count, cjk_count, whitespace_count)
}

/// Check if a character is CJK (Chinese/Japanese/Korean)
pub fn is_cjk(c: char) -> bool {
    let cp = c as u32;
    // Hiragana, Katakana, CJK Unified Ideographs, common ranges
    (0x3040..=0x309F).contains(&cp)  // Hiragana
        || (0x30A0..=0x30FF).contains(&cp)  // Katakana
        || (0x4E00..=0x9FFF).contains(&cp)  // CJK Unified Ideographs
        || (0x3400..=0x4DBF).contains(&cp)  // CJK Extension A
        || (0xAC00..=0xD7AF).contains(&cp)  // Hangul Syllables
        || (0x1100..=0x11FF).contains(&cp) // Hangul Jamo
}

/// Count punctuation characters
/// Returns: (dots, jp_punct, commas, quotes, colons, semicolons, parens)
pub fn count_punctuation(text: &str) -> (usize, usize, usize, usize, usize, usize, usize) {
    let mut dots = 0;
    let mut jp_punct = 0;
    let mut commas = 0;
    let mut quotes = 0;
    let mut colons = 0;
    let mut semicolons = 0;
    let mut parens = 0;

    for c in text.chars() {
        match c {
            '.' | '?' | '!' => dots += 1,
            '。' | '！' | '？' => jp_punct += 1,
            ',' | '、' => commas += 1,
            '"' | '\u{201C}' | '\u{201D}' | '「' | '」' | '\'' | '\u{2018}' | '\u{2019}' => {
                quotes += 1
            }
            ':' | '：' => colons += 1,
            ';' | '；' => semicolons += 1,
            '(' | ')' | '（' | '）' | '[' | ']' | '{' | '}' => parens += 1,
            _ => {}
        }
    }

    (dots, jp_punct, commas, quotes, colons, semicolons, parens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cjk_detection() {
        assert!(is_cjk('日'));
        assert!(is_cjk('本'));
        assert!(is_cjk('あ'));
        assert!(is_cjk('ア'));
        assert!(!is_cjk('a'));
        assert!(!is_cjk('1'));
    }
}
