// Copyright © ArkBig
//! This file provides statistical calculations.

/// Statistics data such as mean.
#[derive(Default, Debug)]
#[allow(dead_code)]
pub struct Stats {
    /// Sorted samples excluding NaN(!finite).
    sorted_samples: Vec<f64>, // 母集団

    /// Count of !finite.
    pub nan_count: usize,

    // Median absolute deviation.
    pub mad: f64, // 平均絶対偏差
    /// Count of outlier.
    pub outlier_count: usize, // 外れ値
    /// Lower control limit for Hampel Identifier.
    pub lcl: f64, // 下限管理限界
    /// Upper control limit for Hampel Identifier.
    pub ucl: f64, // 上限管理限界

    /// Mean of all samples. (μ)
    pub mean: f64, // 平均値
    /// Mean of the samples excluding outlier.
    pub mean_excluding_outlier: f64,

    /// Standard deviation of all samples. (σ)
    pub stdev: f64, // 標準偏差
    /// Standard deviation of the samples excluding outlier.
    pub stdev_excluding_outlier: f64,
}

#[allow(dead_code)]
impl Stats {
    /// Statistical calculation and construction.
    pub fn new(samples: &[f64]) -> Self {
        let sorted = sort_only_finite(samples);
        let nan_count = samples.len() - sorted.len();
        let mut instance = Self {
            sorted_samples: sorted,
            nan_count,
            ..Default::default()
        };
        instance.calc();
        instance
    }

    /// Recalculate from self.sorted_samples.
    fn calc(&mut self) {
        // Add, but not remove. So, the values remain unchanged.
        if self.sorted_samples.is_empty() {
            return;
        };

        let sorted = &self.sorted_samples;
        let count = sorted.len();

        let median = sorted[count / 2];
        let sum: f64 = sorted.iter().sum();
        let mean = sum / (count as f64);

        let mut variance = 0.0; // 分散
        let mut mad = 0.0; // 中央絶対偏差
        for r in sorted {
            let x = *r;
            // It's probably in the range of not overflowing, so divide it later.
            variance += (x - mean).powi(2);
            mad += (x - median).abs();
        }
        variance /= count as f64;
        mad /= count as f64;
        let standard_deviation = variance.sqrt(); // 標準偏差

        // Hampel Identifier of outlier detection.
        let coefficient = 1.4826;
        let lcl = median - 3.0 * coefficient * mad; // 下限管理限界
        let ucl = median + 3.0 * coefficient * mad; // 上限管理限界
        let min = *sorted.first().unwrap_or(&0.0);
        let max = *sorted.last().unwrap_or(&0.0);

        let outlier_count: usize = if lcl <= min && max <= ucl {
            0
        } else {
            sorted.iter().fold(0, |s, r| {
                let x = *r;
                if lcl <= x && x <= ucl {
                    s
                } else {
                    s + 1
                }
            })
        };
        let mean_excluding_outlier = if lcl <= min && max <= ucl {
            mean
        } else {
            sorted.iter().fold(0.0, |s, r| {
                let x = *r;
                if lcl <= x && x <= ucl {
                    s + x
                } else {
                    s
                }
            }) / (count - outlier_count) as f64
        };
        let stdev_excluding_outlier = if lcl <= min && max <= ucl {
            standard_deviation
        } else {
            let variance_excluding_outlier = sorted.iter().fold(0.0, |s, r| {
                let x = *r;
                if lcl <= x && x <= ucl {
                    s + (x - mean_excluding_outlier).powi(2)
                } else {
                    s
                }
            }) / (count - outlier_count) as f64;
            variance_excluding_outlier.sqrt()
        };

        // Reconstruction
        self.mad = mad;
        self.outlier_count = outlier_count;
        self.lcl = lcl;
        self.ucl = ucl;
        self.mean = mean;
        self.mean_excluding_outlier = mean_excluding_outlier;
        self.stdev = standard_deviation;
        self.stdev_excluding_outlier = stdev_excluding_outlier;
    }

    /// Add to sample value
    pub fn add(&mut self, val: f64) {
        if !val.is_finite() {
            self.nan_count += 1;
            return;
        }

        let index = bisect_right(&self.sorted_samples, val, 0, self.sorted_samples.len());
        self.sorted_samples.insert(index, val);
        self.calc();
    }

    /// The number of samples is len().
    pub fn count(&self) -> usize {
        self.sorted_samples.len()
    }

    pub fn count_excluding_outlier(&self) -> usize {
        self.sorted_samples.len() - self.outlier_count
    }

    /// The middle of samples
    pub fn median(&self) -> f64 {
        *self
            .sorted_samples
            .get(self.sorted_samples.len() / 2)
            .unwrap_or(&0.0)
    }
    /// The minimum of samples
    pub fn min(&self) -> f64 {
        *self.sorted_samples.first().unwrap_or(&0.0)
    }
    /// The maximum of samples.
    pub fn max(&self) -> f64 {
        *self.sorted_samples.last().unwrap_or(&0.0)
    }

    pub fn min_excluding_outlier(&self) -> f64 {
        *self
            .sorted_samples
            .iter()
            .find(|x| self.lcl <= **x)
            .unwrap_or(&0.0)
    }
    pub fn max_excluding_outlier(&self) -> f64 {
        *self
            .sorted_samples
            .iter()
            .filter(|x| **x <= self.ucl)
            .nth_back(0)
            .unwrap_or(&0.0)
    }
    pub fn median_excluding_outlier(&self) -> f64 {
        let mut excluding_outlier = self
            .sorted_samples
            .iter()
            .filter(|x| self.lcl <= **x && **x <= self.ucl);
        let count = excluding_outlier.clone().count();
        *excluding_outlier.nth(count / 2).unwrap_or(&0.0)
    }

    /// Has outlier?
    pub fn has_outlier(&self) -> bool {
        0 < self.outlier_count
    }

    /// The coefficient of variation is divided by mean.
    pub fn calc_cv(&self) -> f64 {
        if 0.0 < self.mean {
            self.stdev / self.mean
        } else if 0.0 < self.stdev {
            100.0
        } else {
            0.0
        }
    }

    pub fn calc_cv_excluding_outlier(&self) -> f64 {
        if 0.0 < self.mean_excluding_outlier {
            self.stdev_excluding_outlier / self.mean_excluding_outlier
        } else if 0.0 < self.stdev_excluding_outlier {
            100.0
        } else {
            0.0
        }
    }
}

fn sort_only_finite(data: &[f64]) -> Vec<f64> {
    let mut sorted: Vec<f64> = Vec::with_capacity(data.len());
    for r in data {
        let x = *r;
        if !x.is_finite() {
            continue;
        }
        let ins_index = bisect_right(&sorted, x, 0, sorted.len());
        sorted.insert(ins_index, x);
    }
    sorted
}

fn bisect_right(sorted: &[f64], x: f64, lo: usize, hi: usize) -> usize {
    if hi <= lo {
        hi
    } else if hi + 7 <= lo {
        search_right(sorted, x, lo, hi)
    } else {
        let mid = lo + (hi - lo) / 2;
        if x < sorted[mid] {
            bisect_right(sorted, x, lo, mid)
        } else if sorted[mid] < x {
            bisect_right(sorted, x, mid + 1, hi)
        } else {
            let mut index = mid + 1;
            while index < sorted.len() && x == sorted[index] {
                index += 1;
            }
            index
        }
    }
}

fn search_right(sorted: &[f64], x: f64, lo: usize, hi: usize) -> usize {
    for i in (lo..hi).rev() {
        if sorted[i] <= x {
            return i + 1;
        }
    }
    lo
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::*;

    #[test]
    fn stats_calculate_normal() {
        let samples = vec![3.0, 2.9, 3.1, 2.95, 3.05];
        let stats = Stats::new(&samples);
        assert_eq!(stats.sorted_samples, vec![2.9, 2.95, 3.0, 3.05, 3.1]);
        assert_eq!(stats.nan_count, 0);
        assert_ulps_eq!(stats.mad, 0.06);
        assert_eq!(stats.outlier_count, 0);
        assert_ulps_eq!(stats.lcl, 3.0 - 3.0 * 1.4826 * 0.06);
        assert_ulps_eq!(stats.ucl, 3.0 + 3.0 * 1.4826 * 0.06);
        assert_ulps_eq!(stats.mean, 3.0);
        assert_ulps_eq!(stats.mean_excluding_outlier, stats.mean);
        assert_ulps_eq!(stats.stdev, 0.07071067811865475);
        assert_ulps_eq!(stats.stdev_excluding_outlier, stats.stdev);
        assert_eq!(stats.count(), 5);
        assert_eq!(stats.median(), 3.0);
        assert_eq!(stats.min(), 2.9);
        assert_eq!(stats.max(), 3.1);
        assert_eq!(stats.has_outlier(), false);
    }

    #[test]
    fn stats_calculate_outlier() {
        let samples = vec![0.0, 3.0, 2.9, 3.1, 2.95, 3.05, 10.0];
        let stats = Stats::new(&samples);
        assert_eq!(
            stats.sorted_samples,
            vec![0.0, 2.9, 2.95, 3.0, 3.05, 3.1, 10.0]
        );
        assert_eq!(stats.nan_count, 0);
        assert_ulps_eq!(stats.mad, 1.4714285714285715);
        assert_eq!(stats.outlier_count, 1);
        assert_ulps_eq!(stats.lcl, 3.0 - 3.0 * 1.4826 * 1.4714285714285715);
        assert_ulps_eq!(stats.ucl, 3.0 + 3.0 * 1.4826 * 1.4714285714285715);
        assert_ulps_eq!(stats.mean, 3.5714285714285716);
        assert_ulps_eq!(stats.mean_excluding_outlier, 2.5);
        assert_ulps_eq!(stats.stdev, 2.8218354137052035);
        assert_ulps_eq!(stats.stdev_excluding_outlier, 1.1198958284888227);
        assert_eq!(stats.count(), 7);
        assert_eq!(stats.median(), 3.0);
        assert_eq!(stats.min(), 0.0);
        assert_eq!(stats.max(), 10.0);
        assert_eq!(stats.has_outlier(), true);
    }

    #[test]
    fn stats_add() {
        let samples = vec![0.0, 3.0, 2.9, 3.1, 2.95, 3.05, 10.0];
        let mut stats = Stats::new(&samples);
        assert_eq!(
            stats.sorted_samples,
            vec![0.0, 2.9, 2.95, 3.0, 3.05, 3.1, 10.0]
        );
        assert_eq!(stats.nan_count, 0);
        assert_ulps_eq!(stats.mad, 1.4714285714285715);
        assert_eq!(stats.outlier_count, 1);
        assert_ulps_eq!(stats.lcl, 3.0 - 3.0 * 1.4826 * 1.4714285714285715);
        assert_ulps_eq!(stats.ucl, 3.0 + 3.0 * 1.4826 * 1.4714285714285715);
        assert_ulps_eq!(stats.mean, 3.5714285714285716);
        assert_ulps_eq!(stats.mean_excluding_outlier, 2.5);
        assert_ulps_eq!(stats.stdev, 2.8218354137052035);
        assert_ulps_eq!(stats.stdev_excluding_outlier, 1.1198958284888227);
        assert_eq!(stats.count(), 7);
        assert_eq!(stats.median(), 3.0);
        assert_eq!(stats.min(), 0.0);
        assert_eq!(stats.max(), 10.0);
        assert_eq!(stats.has_outlier(), true);

        stats.add(f64::INFINITY);
        assert_eq!(
            stats.sorted_samples,
            vec![0.0, 2.9, 2.95, 3.0, 3.05, 3.1, 10.0]
        );
        assert_eq!(stats.nan_count, 1);

        stats.add(2.95);
        assert_eq!(
            stats.sorted_samples,
            vec![0.0, 2.9, 2.95, 2.95, 3.0, 3.05, 3.1, 10.0]
        );
        assert_eq!(stats.nan_count, 1);
        assert_ulps_eq!(stats.mad, 1.29375);
        assert_eq!(stats.outlier_count, 1);
        assert_ulps_eq!(stats.lcl, 3.0 - 3.0 * 1.4826 * 1.29375);
        assert_ulps_eq!(stats.ucl, 3.0 + 3.0 * 1.4826 * 1.29375);
        assert_ulps_eq!(stats.mean, 3.4937500000000004);
        assert_ulps_eq!(stats.mean_excluding_outlier, 2.5642857142857145);
        assert_ulps_eq!(stats.stdev, 2.647574066480483);
        assert_ulps_eq!(stats.stdev_excluding_outlier, 1.0487115515561687);
        assert_eq!(stats.count(), 8);
        assert_eq!(stats.median(), 3.0);
        assert_eq!(stats.min(), 0.0);
        assert_eq!(stats.max(), 10.0);
        assert_eq!(stats.has_outlier(), true);

        stats.add(f64::NAN);
        assert_eq!(
            stats.sorted_samples,
            vec![0.0, 2.9, 2.95, 2.95, 3.0, 3.05, 3.1, 10.0]
        );
        assert_eq!(stats.nan_count, 2);
    }

    #[test]
    fn bisect_right_all() {
        let sorted = vec![
            0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0,
            10.0, 10.0, 10.0, 10.0, 20.0, 20.0,
        ];
        assert_eq!(0, bisect_right(&sorted, -1.0, 0, sorted.len()));
        assert_eq!(1, bisect_right(&sorted, 0.0, 0, sorted.len()));
        assert_eq!(10, bisect_right(&sorted, 9.0, 0, sorted.len()));
        assert_eq!(20, bisect_right(&sorted, 10.0, 0, sorted.len()));
        assert_eq!(22, bisect_right(&sorted, 30.0, 0, sorted.len()));
    }

    #[test]
    fn search_right_all() {
        let sorted = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(0, search_right(&sorted, -1.0, 0, sorted.len()));
        assert_eq!(1, search_right(&sorted, 0.0, 0, sorted.len()));
        assert_eq!(5, search_right(&sorted, 4.0, 0, sorted.len()));
        assert_eq!(6, search_right(&sorted, 5.0, 0, sorted.len()));
    }

    #[test]
    fn search_right_partial() {
        let sorted = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        // left side
        assert_eq!(0, search_right(&sorted, -1.0, 0, 3));
        assert_eq!(1, search_right(&sorted, 0.0, 0, 3));
        assert_eq!(3, search_right(&sorted, 4.0, 0, 3));
        // middle range
        assert_eq!(2, search_right(&sorted, -1.0, 2, 5));
        assert_eq!(3, search_right(&sorted, 2.0, 2, 5));
        assert_eq!(5, search_right(&sorted, 5.0, 2, 5));
        // right side
        assert_eq!(3, search_right(&sorted, 0.0, 3, sorted.len()));
        assert_eq!(5, search_right(&sorted, 4.0, 3, sorted.len()));
        assert_eq!(6, search_right(&sorted, 5.0, 3, sorted.len()));
    }

    #[test]
    fn empty_samples() {
        let samples = vec![];
        let stats = Stats::new(&samples);
        assert_eq!(stats.sorted_samples, vec![]);
        assert_eq!(stats.nan_count, 0);
        assert_ulps_eq!(stats.mad, 0.0);
        assert_eq!(stats.outlier_count, 0);
        assert_ulps_eq!(stats.lcl, 0.0);
        assert_ulps_eq!(stats.ucl, 0.0);
        assert_ulps_eq!(stats.mean, 0.0);
        assert_ulps_eq!(stats.mean_excluding_outlier, stats.mean);
        assert_ulps_eq!(stats.stdev, 0.0);
        assert_ulps_eq!(stats.stdev_excluding_outlier, stats.stdev);
        assert_eq!(stats.count(), 0);
        assert_eq!(stats.median(), 0.0);
        assert_eq!(stats.min(), 0.0);
        assert_eq!(stats.max(), 0.0);
        assert_eq!(stats.has_outlier(), false);
    }
}
