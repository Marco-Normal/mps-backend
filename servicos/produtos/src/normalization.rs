use regex::Regex;
use unicode_normalization::UnicodeNormalization;

pub fn normalize_string(value: &str) -> String {
    let value = value.trim();
    let value = value
        .nfd()
        .filter_map(|c| {
            if c.is_ascii() {
                Some(c.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect::<String>();
    println!("{value}");

    let collapsed = Regex::new(r"\s+").unwrap().replace_all(&value, " ");
    collapsed.trim().to_string()
}

#[cfg(test)]
mod tests {
    use crate::normalization::normalize_string;

    #[test]
    fn test_basic() {
        assert_eq!(
            normalize_string("Auto Falante  Pionner"),
            String::from("auto falante pionner")
        );
        assert_eq!(
            normalize_string("Película G05"),
            String::from("pelicula g05")
        );
        assert_eq!(normalize_string("  Rádio MP5"), String::from("radio mp5"));
    }
}
