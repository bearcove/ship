/// Pool of names available for agent assignment.
/// Short, distinct, easy to type in @mentions.
const NAMES: &[&str] = &[
    "Alex", "Aria", "Ash", "Avery",
    "Blake", "Blair", "Briar", "Brook",
    "Cade", "Cal", "Cam", "Casey",
    "Dale", "Dana", "Dex", "Drew",
    "Eden", "Ellis", "Ember", "Emery",
    "Fern", "Finn", "Flynn", "Fox",
    "Glen", "Gray", "Gale", "Greer",
    "Harper", "Hart", "Haven", "Hayes",
    "Indigo", "Iris", "Ivy",
    "Jade", "Jay", "Jesse", "Jordan",
    "Kai", "Kit", "Knox", "Kyle",
    "Lane", "Lark", "Lee", "Linden",
    "Mack", "Mar", "Max", "Mika",
    "Nash", "Nix", "Noel", "Nova",
    "Onyx", "Opal", "Ori",
    "Pace", "Park", "Penn", "Phoenix",
    "Quinn",
    "Rae", "Reed", "Remy", "Riley",
    "Sage", "Sam", "Scout", "Shaw",
    "Tate", "Teal", "Thorne", "Tory",
    "Vale", "Vance", "Vesper", "Vex",
    "Wade", "Wren", "West", "Wilder",
    "Xan",
    "Yael",
    "Zane", "Zara", "Zen",
];

/// Returns the full pool of available agent names.
pub fn name_pool() -> &'static [&'static str] {
    NAMES
}

/// Pick `count` names from the pool, excluding any in `taken`.
/// Returns names in pool order (stable, deterministic).
pub fn pick_names(count: usize, taken: &[&str]) -> Vec<&'static str> {
    NAMES
        .iter()
        .copied()
        .filter(|n| !taken.iter().any(|t| t.eq_ignore_ascii_case(n)))
        .take(count)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_has_enough_names() {
        assert!(NAMES.len() >= 85, "expected at least 85 names, got {}", NAMES.len());
    }

    #[test]
    fn no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for name in NAMES {
            let lower = name.to_lowercase();
            assert!(seen.insert(lower.clone()), "duplicate name: {name}");
        }
    }

    #[test]
    fn pick_excludes_taken() {
        let picked = pick_names(3, &["Alex", "Aria", "Ash"]);
        assert_eq!(picked, vec!["Avery", "Blake", "Blair"]);
    }

    #[test]
    fn pick_excludes_case_insensitive() {
        let picked = pick_names(1, &["alex"]);
        assert_eq!(picked, vec!["Aria"]);
    }

    #[test]
    fn pick_respects_count() {
        let picked = pick_names(2, &[]);
        assert_eq!(picked.len(), 2);
    }
}
