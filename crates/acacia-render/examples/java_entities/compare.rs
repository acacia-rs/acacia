//! Divergence bookkeeping: the first few per case are printed, all are counted.

use serde_json::Value;

const PER_CASE: usize = 4;

#[derive(Default)]
pub struct Report {
    cases: usize,
    lines: Vec<String>,
}

pub struct Case<'a> {
    report: &'a mut Report,
    name: String,
    found: usize,
}

impl Report {
    pub fn case(&mut self, name: &str) -> Case<'_> {
        self.cases += 1;
        Case { report: self, name: name.to_owned(), found: 0 }
    }

    pub fn print(&self) {
        println!("CASES\t{}", self.cases);
        for line in &self.lines {
            println!("{line}");
        }
    }
}

impl Case<'_> {
    pub fn diverge(&mut self, field: &str, expected: impl std::fmt::Display, got: impl std::fmt::Display) {
        self.found += 1;
        if self.found <= PER_CASE {
            self.report.lines.push(format!("DIVERGENCE\t{}\t{field}\t{expected}\t{got}", self.name));
        }
    }

    /// Numbers within `tolerance` (absolute below 1, relative above).
    pub fn numbers(&mut self, field: &str, expected: &[f32], got: &[f32], tolerance: f32) {
        if expected.len() != got.len() {
            return self.diverge(&format!("{field}.len"), expected.len(), got.len());
        }
        let off = expected.iter().zip(got).position(|(a, b)| (a - b).abs() > tolerance * a.abs().max(1.0));
        if off.is_some() {
            self.diverge(field, format!("{expected:?}"), format!("{got:?}"));
        }
    }
}

pub fn floats(value: &Value) -> Vec<f32> {
    value.as_array().into_iter().flatten().map(|v| v.as_f64().map_or_else(|| f32::from(u8::from(v.as_bool().unwrap_or(false))), |f| f as f32)).collect()
}
