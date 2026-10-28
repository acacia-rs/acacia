//! Java's orderings the HUD sorts with. Ported from Horizon's `hud_overlays.rs` (MIT).

use std::cmp::Ordering;

/// `Arrays.sort(T[], Comparator)`: below `MIN_MERGE` TimSort is a natural run then a binary
/// insertion sort, which matters when the comparator is not a total order (effects). Longer
/// slices fall back to a stable sort.
pub fn java_sort<T>(a: &mut [T], cmp: impl Fn(&T, &T) -> Ordering) {
    let n = a.len();
    if n < 2 {
        return;
    }
    if n >= 32 {
        a.sort_by(cmp);
        return;
    }
    let mut run = 2;
    if cmp(&a[1], &a[0]) == Ordering::Less {
        while run < n && cmp(&a[run], &a[run - 1]) == Ordering::Less {
            run += 1;
        }
        a[..run].reverse();
    } else {
        while run < n && cmp(&a[run], &a[run - 1]) != Ordering::Less {
            run += 1;
        }
    }
    for start in run..n {
        let (mut left, mut right) = (0, start);
        while left < right {
            let mid = (left + right) / 2;
            if cmp(&a[start], &a[mid]) == Ordering::Less {
                right = mid;
            } else {
                left = mid + 1;
            }
        }
        a[left..=start].rotate_right(1);
    }
}

/// `String.CASE_INSENSITIVE_ORDER` over UTF-16 code units.
pub fn compare_ignore_case(a: &str, b: &str) -> Ordering {
    fn single(mut it: impl Iterator<Item = char>, c: char) -> char {
        match (it.next(), it.next()) {
            (Some(m), None) => m,
            _ => c,
        }
    }
    let (x, y): (Vec<u16>, Vec<u16>) = (a.encode_utf16().collect(), b.encode_utf16().collect());
    for (&p, &q) in x.iter().zip(&y) {
        if p == q {
            continue;
        }
        let (Some(c1), Some(c2)) = (char::from_u32(u32::from(p)), char::from_u32(u32::from(q))) else {
            return p.cmp(&q);
        };
        let (u1, u2) = (single(c1.to_uppercase(), c1), single(c2.to_uppercase(), c2));
        if u1 != u2 {
            let (l1, l2) = (single(u1.to_lowercase(), u1), single(u2.to_lowercase(), u2));
            if l1 != l2 {
                return (l1 as u32).cmp(&(l2 as u32));
            }
        }
    }
    x.len().cmp(&y.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_insensitive_order() {
        assert_eq!(compare_ignore_case("bob", "Bobby"), Ordering::Less);
        assert_eq!(compare_ignore_case("Zed", "alex"), Ordering::Greater);
        assert_eq!(compare_ignore_case("_under", "a"), Ordering::Less);
        assert_eq!(compare_ignore_case("ABC", "abc"), Ordering::Equal);
    }

    #[test]
    fn sorts_stably_like_timsort() {
        let mut v = [(1, 'a'), (0, 'b'), (1, 'c'), (0, 'd')];
        java_sort(&mut v, |a, b| a.0.cmp(&b.0));
        assert_eq!(v, [(0, 'b'), (0, 'd'), (1, 'a'), (1, 'c')]);
    }
}
