use regex::Regex;
use std::collections::HashSet;

///
/// A naive implementation of n-gram search index for a text search.
pub(crate) struct Trigram {}

impl Trigram {

    fn clean<S: ToString>(text: S) -> String {
        let re = Regex::new(r"[\s\-_&]").unwrap();
        let clean = text.to_string();
        let clean = clean.to_lowercase();
        let clean = clean.trim().to_string();
        let clean = re.replace_all(clean.as_str(), "").to_string();
        return clean
    }

    ///
    /// Search bounds for the index for the provided query.
    /// I.e., provides with a 3-char part of that query that can be used to scan though index.
    pub(crate) fn search_bound<S: ToString>(query: S) -> Option<String> {
        let clean = Trigram::clean(query);
        let clean = clean.chars().collect::<Vec<_>>();
        if clean.len() < 3 {
            if clean.is_empty() {
                return None;
            }
            return Some(clean.iter().collect::<String>())
        }
        return Some(clean[0..3].iter().collect::<String>())
    }

    ///
    /// Splits the source text in parts up to 3-characters to use as an index.
    pub(crate) fn extract<S: ToString>(text: S) -> Vec<String> {
        let clean = Trigram::clean(text);
        if clean.len() < 3 {
            if clean.is_empty() {
                return vec![]
            }
            return vec![clean]
        }

        let mut results = HashSet::new();
        let clean = clean.chars().collect::<Vec<_>>();

        for i in 0..clean.len() {
            let onegram = clean[i].to_string();
            results.insert(onegram);
            if i > 0 {
                let twogram = clean[(i-1)..(i+1)].iter().collect::<String>();
                results.insert(twogram);
            }
            if i > 1 {
                let trigram = clean[(i-2)..(i+1)].iter().collect::<String>();
                results.insert(trigram);
            }
        }

        results.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Trigram;

    #[test]
    fn clean_removes_spaces() {
        let act = Trigram::clean("test test test");
        assert_eq!("testtesttest".to_string(), act);

        let act = Trigram::clean("  test     test test  ");
        assert_eq!("testtesttest".to_string(), act);
        let act = Trigram::clean("test\ttest test");
        assert_eq!("testtesttest".to_string(), act);
    }

    #[test]
    fn clean_removes_newlines() {
        let act = Trigram::clean("test\ntest test\n");
        assert_eq!("testtesttest".to_string(), act);
    }

    #[test]
    fn clean_removes_specialcharacters() {
        let act = Trigram::clean("test-test_test");
        assert_eq!("testtesttest".to_string(), act);
    }

    #[test]
    fn clean_uses_same_case() {
        let act = Trigram::clean("Test TEST test");
        assert_eq!("testtesttest".to_string(), act);
    }

    #[test]
    fn for_no_text_no_trigrams() {
        let act = Trigram::extract(" ");
        assert!(act.is_empty());
    }

    #[test]
    fn for_small_text_single_trigram() {
        let act = Trigram::extract("HI");
        assert_eq!(vec!["hi".to_string()], act);
    }

    #[test]
    fn extract_trigrams() {
        let mut act = Trigram::extract("test test test");
        let mut exp: Vec<String> = vec![
            "t", "e", "s",
            "te", "es", "st", "tt",
            "tes", "est", "stt", "tte",
        ].iter().map(|c| c.to_string()).collect();
        act.sort();
        exp.sort();
        assert_eq!(exp, act);
    }

    #[test]
    fn extract_unicode_trigrams() {
        let mut act = Trigram::extract("Привет-Мир");
        let mut exp: Vec<String> = vec![
            "п", "р", "и", "в", "е", "т", "м",
            "пр", "ри", "ив", "ве", "ет", "тм", "ми", "ир",
            "при", "рив", "иве", "вет", "етм", "тми", "мир",
        ].iter().map(|c| c.to_string()).collect();
        act.sort();
        exp.sort();
        assert_eq!(exp, act);
    }

    #[test]
    fn no_search_bound_for_empty() {
        let act = Trigram::search_bound("");
        assert_eq!(None, act);
    }

    #[test]
    fn no_search_bound_for_special() {
        let act = Trigram::search_bound("-");
        assert_eq!(None, act);
    }

    #[test]
    fn search_bound_for_short() {
        let act = Trigram::search_bound("A");
        assert_eq!(Some("a".to_string()), act);

        let act = Trigram::search_bound("Ab");
        assert_eq!(Some("ab".to_string()), act);

        let act = Trigram::search_bound("Abc");
        assert_eq!(Some("abc".to_string()), act);
    }

    #[test]
    fn search_bound_for_short_unicode() {
        let act = Trigram::search_bound("й");
        assert_eq!(Some("й".to_string()), act);

        let act = Trigram::search_bound("Йц");
        assert_eq!(Some("йц".to_string()), act);

        let act = Trigram::search_bound("ЙЦЖ");
        assert_eq!(Some("йцж".to_string()), act);
    }

    #[test]
    fn search_bound_for_long() {
        let act = Trigram::search_bound("John Smith");
        assert_eq!(Some("joh".to_string()), act);
    }

    #[test]
    fn search_bound_for_long_unicode() {
        let act = Trigram::search_bound("Иван Кузнецов");
        assert_eq!(Some("ива".to_string()), act);
    }
}