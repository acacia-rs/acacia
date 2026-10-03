//! Line diff by longest common subsequence (inputs are short: packet dumps, packet-name sequences).

pub enum Line<'a> {
    Same(&'a str),
    Left(&'a str),
    Right(&'a str),
}

pub fn lines<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<Line<'a>> {
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            out.push(Line::Same(a[i]));
            (i, j) = (i + 1, j + 1);
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            out.push(Line::Right(b[j]));
            j += 1;
        } else {
            out.push(Line::Left(a[i]));
            i += 1;
        }
    }
    out
}

/// Prints only the changed lines, each with `ctx` unchanged lines around it; `-` = left, `+` = right.
pub fn print(diff: &[Line], ctx: usize) -> usize {
    let changed: Vec<bool> = diff.iter().map(|l| !matches!(l, Line::Same(_))).collect();
    let near = |i: usize| (i.saturating_sub(ctx)..=(i + ctx).min(diff.len() - 1)).any(|k| changed[k]);
    let mut gap = false;
    for (i, line) in diff.iter().enumerate() {
        if !near(i) {
            gap = true;
            continue;
        }
        if std::mem::take(&mut gap) {
            println!("    ...");
        }
        match line {
            Line::Same(s) => println!("      {s}"),
            Line::Left(s) => println!("    - {s}"),
            Line::Right(s) => println!("    + {s}"),
        }
    }
    changed.iter().filter(|&&c| c).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_common_lines_and_marks_the_rest() {
        let d = lines(&["a", "b", "c"], &["a", "x", "c", "d"]);
        let marks: Vec<String> = d
            .iter()
            .map(|l| match l {
                Line::Same(s) => format!("={s}"),
                Line::Left(s) => format!("-{s}"),
                Line::Right(s) => format!("+{s}"),
            })
            .collect();
        assert_eq!(marks, ["=a", "+x", "-b", "=c", "+d"]);
    }
}
