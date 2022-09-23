/// Statical information such as mean.
#[derive(Default, Debug)]
#[allow(dead_code)]
pub struct Mean {
    /// Sorted population excluding NaN(!finite).
    sorted_population: Vec<f64>, // 母集団

    /// Count of !finite.
    nan_count: usize,

    /// Median of all population.
    median: f64, // 中央値

    // Median absolute deviation.
    mad: f64, // 平均絶対偏差
    /// Count of outlier.
    outlier_count: usize, // 外れ値
    /// Lower threshold of outlier for Hampel Identifier.
    outlier_lower: f64,
    /// Upper threshold of outlier for Hampel Identifier.
    outlier_upper: f64,

    /// Mean of all population. (μ)
    mean: f64, // 平均値
    /// Mean of the population excluding outlier.
    mean_excluding_outlier: f64,

    /// Standard deviation of all population. (σ)
    stdev: f64, // 標準偏差
    /// Standard deviation of the population excluding outlier.
    stdev_excluding_outlier: f64,
}

#[allow(dead_code)]
impl Mean {
    /// Statistical calculation and construction.
    fn new(population: Vec<f64>) -> Self {
        if population.is_empty() {
            return Self::default();
        };

        let sorted = sort_only_finite(&population);
        let count = sorted.len();
        if count == 0 {
            return Self {
                outlier_count: population.len(),
                ..Default::default()
            };
        }

        let median = sorted[count / 2];
        let sum: f64 = sorted.iter().sum();
        let mean = sum / (count as f64);

        let mut variance = 0.0; // 分散
        let mut mad = 0.0; // 中央絶対偏差
        for r in &sorted {
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
        let lower = median - 3.0 * coefficient * mad;
        let upper = median + 3.0 * coefficient * mad;
        let min = *sorted.first().unwrap();
        let max = *sorted.last().unwrap();

        let outlier_count: usize = if lower <= min && max <= upper {
            0
        } else {
            sorted.iter().fold(0, |s, r| {
                let x = *r;
                if lower <= x && x <= upper {
                    s
                } else {
                    s + 1
                }
            })
        };
        let mean_excluding_outlier = if lower <= min && max <= upper {
            mean
        } else {
            sorted.iter().fold(0.0, |s, r| {
                let x = *r;
                if lower <= x && x <= upper {
                    s + x
                } else {
                    s
                }
            }) / (count - outlier_count) as f64
        };
        let stdev_excluding_outlier = if lower <= min && max <= upper {
            standard_deviation
        } else {
            let variance_excluding_outlier = sorted.iter().fold(0.0, |s, r| {
                let x = *r;
                if lower <= x && x <= upper {
                    s + (x - mean_excluding_outlier).powi(2)
                } else {
                    s
                }
            }) / (count - outlier_count) as f64;
            variance_excluding_outlier.sqrt()
        };

        // construction.
        Self {
            sorted_population: sorted,
            nan_count: population.len() - count,
            median,
            mad,
            outlier_count,
            outlier_lower: lower,
            outlier_upper: upper,
            mean,
            mean_excluding_outlier,
            stdev: standard_deviation,
            stdev_excluding_outlier,
        }
    }

    /// The number of samples is len().
    fn count(&self) -> usize {
        self.sorted_population.len()
    }
    /// The minimum of samples
    fn min(&self) -> f64 {
        *self.sorted_population.first().unwrap()
    }
    /// The maximum of samples is last().
    fn max(&self) -> f64 {
        *self.sorted_population.last().unwrap()
    }

    /// Has outlier?
    fn has_outlier(&self) -> bool {
        0 < self.outlier_count
    }

    /// The coefficient of variation is divided by mean.
    fn calc_cv(&self) -> f64 {
        self.stdev / self.mean
    }
    /// The coefficient of variation excluding outlier is divided by mean_excluding_outlier.
    fn calc_cv_excluding_outlier(&self) -> f64 {
        self.stdev_excluding_outlier / self.mean_excluding_outlier
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
            while x == sorted[index] {
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
    fn mean_calculate_normal() {
        let population = vec![3.0, 2.9, 3.1, 2.95, 3.05];
        let mean = Mean::new(population);
        assert_eq!(mean.sorted_population, vec![2.9, 2.95, 3.0, 3.05, 3.1]);
        assert_eq!(mean.nan_count, 0);
        assert_eq!(mean.median, 3.0);
        assert_ulps_eq!(mean.mad, 0.06);
        assert_eq!(mean.outlier_count, 0);
        assert_ulps_eq!(mean.outlier_lower, 3.0 - 3.0 * 1.4826 * 0.06);
        assert_ulps_eq!(mean.outlier_upper, 3.0 + 3.0 * 1.4826 * 0.06);
        assert_ulps_eq!(mean.mean, 3.0);
        assert_ulps_eq!(mean.mean_excluding_outlier, mean.mean);
        assert_ulps_eq!(mean.stdev, 0.07071067811865475);
        assert_ulps_eq!(mean.stdev_excluding_outlier, mean.stdev);
        assert_eq!(mean.count(), 5);
        assert_eq!(mean.min(), 2.9);
        assert_eq!(mean.max(), 3.1);
        assert_eq!(mean.has_outlier(), false);
    }

    #[test]
    fn mean_calculate_outlier() {
        let population = vec![0.0, 3.0, 2.9, 3.1, 2.95, 3.05, 10.0];
        let mean = Mean::new(population);
        assert_eq!(
            mean.sorted_population,
            vec![0.0, 2.9, 2.95, 3.0, 3.05, 3.1, 10.0]
        );
        assert_eq!(mean.nan_count, 0);
        assert_eq!(mean.median, 3.0);
        assert_ulps_eq!(mean.mad, 1.4714285714285715);
        assert_eq!(mean.outlier_count, 1);
        assert_ulps_eq!(mean.outlier_lower, 3.0 - 3.0 * 1.4826 * 1.4714285714285715);
        assert_ulps_eq!(mean.outlier_upper, 3.0 + 3.0 * 1.4826 * 1.4714285714285715);
        assert_ulps_eq!(mean.mean, 3.5714285714285716);
        assert_ulps_eq!(mean.mean_excluding_outlier, 2.5);
        assert_ulps_eq!(mean.stdev, 2.8218354137052035);
        assert_ulps_eq!(mean.stdev_excluding_outlier, 1.1198958284888227);
        assert_eq!(mean.count(), 7);
        assert_eq!(mean.min(), 0.0);
        assert_eq!(mean.max(), 10.0);
        assert_eq!(mean.has_outlier(), true);
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
}
