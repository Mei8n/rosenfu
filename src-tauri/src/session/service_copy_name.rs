use std::collections::HashSet;

const MAX_NAME_CHARS: usize = 80;
const LEGACY_PREFIX_LIMIT: usize = 74;
const COPY_SUFFIX: &str = " のコピー";

/// Keep the original first-copy spelling, then pick the first available
/// readable number without exceeding the same scalar count as name editing.
pub(super) fn service_copy_name<'a>(
    source: &str,
    existing: impl IntoIterator<Item = &'a str>,
) -> String {
    let taken: HashSet<_> = existing.into_iter().collect();
    let mut number = 1usize;
    loop {
        let suffix = if number == 1 {
            COPY_SUFFIX.to_owned()
        } else {
            format!("{COPY_SUFFIX} {number}")
        };
        let limit = LEGACY_PREFIX_LIMIT.min(MAX_NAME_CHARS - suffix.chars().count());
        let mut name: String = source.chars().take(limit).collect();
        name.push_str(&suffix);
        if !taken.contains(name.as_str()) {
            return name;
        }
        // At most `taken.len()` distinct candidates can be occupied.
        number += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_new_first_copy_and_skips_occupied_numbers() {
        assert_eq!(
            service_copy_name("普通", ["普通", "普通 のコピー 2"]),
            "普通 のコピー"
        );
        assert_eq!(
            service_copy_name("普通", ["普通 のコピー", "普通 のコピー 2"]),
            "普通 のコピー 3"
        );
        assert_eq!(
            service_copy_name("普通 のコピー", ["普通 のコピー"]),
            "普通 のコピー のコピー"
        );
    }

    #[test]
    fn numbered_copies_of_long_japanese_names_fit_editor_limit() {
        let source = "通勤快速".repeat(20);
        let mut names = Vec::<String>::new();
        for index in 1..=11 {
            let name = service_copy_name(&source, names.iter().map(String::as_str));
            assert!(name.chars().count() <= MAX_NAME_CHARS);
            assert!(name.starts_with("通勤快速"));
            if index > 1 {
                assert!(name.ends_with(&format!("のコピー {index}")));
            }
            names.push(name);
        }
    }
}
