use std::collections::HashSet;
use std::hash::Hash;

/// Finds the first duplicate element in a slice.
///
/// Returns `None` if all elements are unique, otherwise returns
/// the first element encountered that appears more than once.
///
/// # Time Complexity
/// O(n) where n is the length of the slice.
///
/// # Space Complexity
/// O(n) for the hash set of seen elements.
pub fn find_duplicate<T>(v: &[T]) -> Option<&T>
where
    T: Eq + Hash,
{
    let mut seen = HashSet::new();
    v.iter().find(|&item| !seen.insert(item))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_duplicate_none() {
        let v = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(find_duplicate(&v), None);
    }

    #[test]
    fn test_find_duplicate_some() {
        let v = vec![1, 2, 3, 2, 5];
        assert_eq!(find_duplicate(&v), Some(&2));
    }

    #[test]
    fn test_find_duplicate_first_occurrence() {
        let v = vec![1, 2, 3, 2, 3, 1];
        assert_eq!(find_duplicate(&v), Some(&2));
    }

    #[test]
    fn test_find_duplicate_empty() {
        let v: Vec<i32> = vec![];
        assert_eq!(find_duplicate(&v), None);
    }
}
