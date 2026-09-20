use std::collections::VecDeque;

/// Fixed-length raw and chart-ready histories sharing one normalization range.
#[derive(Debug)]
pub(super) struct TemperatureHistory {
    raw: VecDeque<f64>,
    normalized: VecDeque<f64>,
    floor: f64,
    ceiling: f64,
}

impl TemperatureHistory {
    pub fn new(length: usize) -> Self {
        assert!(length > 0);
        Self {
            raw: VecDeque::from(vec![0.0; length]),
            normalized: VecDeque::from(vec![0.0; length]),
            floor: 0.0,
            ceiling: 100.0,
        }
    }

    pub fn raw(&self) -> &VecDeque<f64> {
        &self.raw
    }

    pub fn chart_samples(&self) -> &VecDeque<f64> {
        &self.normalized
    }

    pub fn push_back(&mut self, value: f64) {
        let removed = self.raw.pop_front().unwrap();
        self.raw.push_back(value);
        let previous = self.ceiling;
        if value > self.ceiling {
            self.ceiling = value;
        } else if removed == self.ceiling && self.ceiling > self.floor.max(100.0) {
            self.ceiling = self.find_ceiling();
        }
        if self.ceiling != previous && self.floor != 0.0 {
            self.recompute();
        } else {
            self.normalized.pop_front();
            self.normalized.push_back(self.normalize(value));
        }
    }

    pub fn set_floor(&mut self, floor: f64) {
        if self.floor != floor {
            self.floor = floor;
            self.ceiling = self.find_ceiling();
            self.recompute();
        }
    }

    pub fn clear(&mut self) {
        self.raw.iter_mut().for_each(|value| *value = 0.0);
        self.ceiling = self.floor.max(100.0);
        self.recompute();
    }

    fn find_ceiling(&self) -> f64 {
        self.raw
            .iter()
            .copied()
            .fold(self.floor.max(100.0), f64::max)
    }

    fn normalize(&self, value: f64) -> f64 {
        if self.floor == 0.0 {
            value
        } else if value <= self.floor || self.ceiling <= self.floor {
            0.0
        } else {
            (value - self.floor) / (self.ceiling - self.floor) * 100.0
        }
    }

    fn recompute(&mut self) {
        for index in 0..self.raw.len() {
            self.normalized[index] = self.normalize(self.raw[index]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_matches_full_normalization(history: &TemperatureHistory) {
        let ceiling = history
            .raw
            .iter()
            .copied()
            .fold(history.floor.max(100.0), f64::max);
        let expected: VecDeque<_> = history
            .raw
            .iter()
            .map(|&value| {
                if history.floor == 0.0 {
                    value
                } else if value <= history.floor || ceiling <= history.floor {
                    0.0
                } else {
                    (value - history.floor) / (ceiling - history.floor) * 100.0
                }
            })
            .collect();
        assert_eq!(history.chart_samples(), &expected);
        assert_eq!(history.ceiling, ceiling);
    }

    #[test]
    fn incremental_history_matches_full_calculation_through_range_changes() {
        let mut history = TemperatureHistory::new(3);
        for floor in [50.0, 120.0, 0.0, -10.0, 75.0] {
            history.set_floor(floor);
            assert_matches_full_normalization(&history);
            // Duplicate peaks, new peaks, expiration, and all samples below floor.
            for value in [80.0, 120.0, 120.0, 110.0, 90.0, 85.0, 70.0, 0.0, 0.0, 0.0] {
                history.push_back(value);
                assert_matches_full_normalization(&history);
            }
            history.clear();
            assert_matches_full_normalization(&history);
        }
    }

    #[test]
    fn unchanged_range_preserves_existing_samples_and_storage() {
        let mut history = TemperatureHistory::new(3);
        history.set_floor(50.0);
        history.push_back(60.0);
        history.push_back(70.0);
        let previous = history.normalized[2];
        let capacity = history.normalized.capacity();
        history.push_back(80.0);
        assert_eq!(history.normalized[1], previous);
        assert_eq!(history.normalized.capacity(), capacity);
        history.push_back(150.0);
        assert_eq!(history.normalized.capacity(), capacity);
        history.set_floor(40.0);
        assert_eq!(history.normalized.capacity(), capacity);
        assert_matches_full_normalization(&history);
    }
}
