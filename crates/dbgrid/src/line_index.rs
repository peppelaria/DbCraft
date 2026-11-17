//! Prefix sums over line sizes with sparse custom sizes.
//! Derived from GridCraft `model::sheet::LineIndex` (MIT OR Apache-2.0), see NOTICE.md.

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct LineIndex {
    default: f64,
    max: u32,
    /// (line, start position of that line, its size), sorted by line.
    lines: Vec<(u32, f64, f64)>,
}

impl LineIndex {
    /// `max` is the number of lines on the axis (may be 0).
    pub fn new(custom: &BTreeMap<u32, f32>, default: f32, max: u32) -> LineIndex {
        let default = f64::from(default.max(0.0));
        let mut lines = Vec::with_capacity(custom.len());
        let mut pos = 0.0;
        let mut prev = 0u32;
        for (&line, &size) in custom.range(..max) {
            pos += f64::from(line - prev) * default;
            let size = f64::from(size.max(0.0));
            lines.push((line, pos, size));
            pos += size;
            prev = line + 1;
        }
        LineIndex { default, max, lines }
    }

    pub fn uniform(size: f32, max: u32) -> LineIndex {
        LineIndex { default: f64::from(size.max(0.0)), max, lines: Vec::new() }
    }

    pub fn len(&self) -> u32 {
        self.max
    }

    pub fn is_empty(&self) -> bool {
        self.max == 0
    }

    /// Start position of `line` (`line == len()` gives the total extent).
    pub fn start(&self, line: u32) -> f64 {
        match self.lines.binary_search_by(|e| e.0.cmp(&line)) {
            Ok(i) => self.lines.get(i).map_or(0.0, |e| e.1),
            Err(0) => f64::from(line) * self.default,
            Err(i) => match self.lines.get(i - 1) {
                Some(&(l, p, s)) => p + s + f64::from(line - l - 1) * self.default,
                None => f64::from(line) * self.default,
            },
        }
    }

    pub fn size(&self, line: u32) -> f64 {
        match self.lines.binary_search_by(|e| e.0.cmp(&line)) {
            Ok(i) => self.lines.get(i).map_or(self.default, |e| e.2),
            Err(_) => self.default,
        }
    }

    pub fn total(&self) -> f64 {
        self.start(self.max)
    }

    /// The line containing `pos` (clamped to the axis). Returns 0 on an empty axis.
    pub fn at(&self, pos: f64) -> u32 {
        if pos <= 0.0 || self.max == 0 {
            return 0;
        }
        let last = self.max - 1;
        let i = self.lines.partition_point(|e| e.1 <= pos);
        let (base_line, base_pos) = match i.checked_sub(1).and_then(|k| self.lines.get(k)) {
            Some(&(l, p, s)) => {
                if pos < p + s {
                    return l.min(last);
                }
                (l + 1, p + s)
            }
            None => (0, 0.0),
        };
        if self.default <= 0.0 {
            return base_line.min(last);
        }
        let r = f64::from(base_line) + ((pos - base_pos) / self.default).floor();
        if r >= f64::from(last) { last } else { r as u32 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_positions() {
        let li = LineIndex::uniform(20.0, 100);
        assert_eq!(li.start(3), 60.0);
        assert_eq!(li.at(59.9), 2);
        assert_eq!(li.at(1e9), 99);
        assert_eq!(li.total(), 2000.0);
    }

    #[test]
    fn custom_sizes() {
        let mut m = BTreeMap::new();
        m.insert(1, 100.0);
        let li = LineIndex::new(&m, 50.0, 5);
        assert_eq!(li.start(2), 150.0);
        assert_eq!(li.at(120.0), 1);
        assert_eq!(li.at(151.0), 2);
        assert_eq!(li.size(1), 100.0);
    }

    #[test]
    fn empty_axis() {
        let li = LineIndex::uniform(20.0, 0);
        assert_eq!(li.at(100.0), 0);
        assert!(li.is_empty());
    }
}
