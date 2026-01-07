pub fn quartile(data: &[f64], q: f64) -> f64 {
    let mut sorted: Vec<f64> = data.to_vec();
    if sorted.is_empty() {
        return 0.0;
    }
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    let pos = q * (sorted.len() - 1) as f64;
    let base = pos.floor() as usize;
    let rest = pos - base as f64;
    if base + 1 < sorted.len() {
        sorted[base] + rest * (sorted[base + 1] - sorted[base])
    } else {
        sorted[base]
    }
}

pub fn jitter(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let mut sum_diff = 0.0;
    for i in 1..data.len() {
        sum_diff += (data[i] - data[i - 1]).abs();
    }
    sum_diff / (data.len() - 1) as f64
}

pub fn median(data: &[f64]) -> f64 {
    let mut sorted: Vec<f64> = data.to_vec();
    if sorted.is_empty() {
        return 0.0;
    }
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== quartile tests ====================

    #[test]
    fn test_quartile_empty_data() {
        assert_eq!(quartile(&[], 0.25), 0.0);
        assert_eq!(quartile(&[], 0.5), 0.0);
        assert_eq!(quartile(&[], 0.75), 0.0);
    }

    #[test]
    fn test_quartile_single_element() {
        let data = [42.0];
        assert_eq!(quartile(&data, 0.0), 42.0);
        assert_eq!(quartile(&data, 0.25), 42.0);
        assert_eq!(quartile(&data, 0.5), 42.0);
        assert_eq!(quartile(&data, 0.75), 42.0);
        assert_eq!(quartile(&data, 1.0), 42.0);
    }

    #[test]
    fn test_quartile_two_elements() {
        let data = [10.0, 20.0];
        assert_eq!(quartile(&data, 0.0), 10.0);
        assert_eq!(quartile(&data, 0.5), 15.0);
        assert_eq!(quartile(&data, 1.0), 20.0);
    }

    #[test]
    fn test_quartile_standard_quartiles() {
        // Data: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10
        let data: Vec<f64> = (1..=10).map(|x| x as f64).collect();

        // Q1 (25th percentile)
        let q1 = quartile(&data, 0.25);
        assert!((q1 - 3.25).abs() < 0.001, "Q1 expected ~3.25, got {}", q1);

        // Q2 (50th percentile / median)
        let q2 = quartile(&data, 0.5);
        assert!((q2 - 5.5).abs() < 0.001, "Q2 expected ~5.5, got {}", q2);

        // Q3 (75th percentile)
        let q3 = quartile(&data, 0.75);
        assert!((q3 - 7.75).abs() < 0.001, "Q3 expected ~7.75, got {}", q3);
    }

    #[test]
    fn test_quartile_unsorted_input() {
        // Input is unsorted, function should handle it
        let data = [5.0, 1.0, 9.0, 3.0, 7.0];
        // Sorted: 1, 3, 5, 7, 9
        assert_eq!(quartile(&data, 0.0), 1.0);
        assert_eq!(quartile(&data, 0.5), 5.0);
        assert_eq!(quartile(&data, 1.0), 9.0);
    }

    #[test]
    fn test_quartile_with_duplicates() {
        let data = [5.0, 5.0, 5.0, 10.0, 10.0];
        // Sorted: 5, 5, 5, 10, 10
        assert_eq!(quartile(&data, 0.0), 5.0);
        assert_eq!(quartile(&data, 0.5), 5.0);
        assert_eq!(quartile(&data, 1.0), 10.0);
    }

    #[test]
    fn test_quartile_interpolation() {
        let data = [0.0, 100.0];
        // Should interpolate between values
        assert_eq!(quartile(&data, 0.25), 25.0);
        assert_eq!(quartile(&data, 0.75), 75.0);
    }

    // ==================== jitter tests ====================

    #[test]
    fn test_jitter_empty_data() {
        assert_eq!(jitter(&[]), 0.0);
    }

    #[test]
    fn test_jitter_single_element() {
        assert_eq!(jitter(&[42.0]), 0.0);
    }

    #[test]
    fn test_jitter_two_elements() {
        let data = [10.0, 20.0];
        assert_eq!(jitter(&data), 10.0);
    }

    #[test]
    fn test_jitter_constant_values() {
        // No variation = zero jitter
        let data = [5.0, 5.0, 5.0, 5.0, 5.0];
        assert_eq!(jitter(&data), 0.0);
    }

    #[test]
    fn test_jitter_alternating_values() {
        // Alternating between 0 and 10
        let data = [0.0, 10.0, 0.0, 10.0, 0.0];
        // Differences: |10-0| + |0-10| + |10-0| + |0-10| = 10+10+10+10 = 40
        // Average: 40 / 4 = 10
        assert_eq!(jitter(&data), 10.0);
    }

    #[test]
    fn test_jitter_increasing_sequence() {
        // Steadily increasing
        let data = [1.0, 2.0, 3.0, 4.0, 5.0];
        // Differences: 1+1+1+1 = 4, average = 4/4 = 1
        assert_eq!(jitter(&data), 1.0);
    }

    #[test]
    fn test_jitter_decreasing_sequence() {
        // Steadily decreasing
        let data = [5.0, 4.0, 3.0, 2.0, 1.0];
        // Differences: |-1|+|-1|+|-1|+|-1| = 4, average = 4/4 = 1
        assert_eq!(jitter(&data), 1.0);
    }

    #[test]
    fn test_jitter_realistic_latency() {
        // Simulating realistic latency measurements
        let data = [50.0, 52.0, 48.0, 51.0, 49.0];
        // Differences: |2| + |-4| + |3| + |-2| = 2+4+3+2 = 11
        // Average: 11 / 4 = 2.75
        assert!((jitter(&data) - 2.75).abs() < 0.001);
    }

    // ==================== median tests ====================

    #[test]
    fn test_median_empty_data() {
        assert_eq!(median(&[]), 0.0);
    }

    #[test]
    fn test_median_single_element() {
        assert_eq!(median(&[42.0]), 42.0);
    }

    #[test]
    fn test_median_two_elements() {
        let data = [10.0, 20.0];
        assert_eq!(median(&data), 15.0);
    }

    #[test]
    fn test_median_odd_count() {
        // Odd number of elements - middle element is median
        let data = [1.0, 3.0, 5.0, 7.0, 9.0];
        assert_eq!(median(&data), 5.0);
    }

    #[test]
    fn test_median_even_count() {
        // Even number of elements - average of two middle elements
        let data = [1.0, 3.0, 5.0, 7.0];
        assert_eq!(median(&data), 4.0); // (3 + 5) / 2
    }

    #[test]
    fn test_median_unsorted_input() {
        // Input is unsorted
        let data = [9.0, 1.0, 5.0, 3.0, 7.0];
        // Sorted: 1, 3, 5, 7, 9 -> median = 5
        assert_eq!(median(&data), 5.0);
    }

    #[test]
    fn test_median_with_duplicates() {
        let data = [5.0, 5.0, 5.0, 10.0, 10.0];
        // Sorted: 5, 5, 5, 10, 10 -> median = 5
        assert_eq!(median(&data), 5.0);
    }

    #[test]
    fn test_median_negative_values() {
        let data = [-10.0, -5.0, 0.0, 5.0, 10.0];
        assert_eq!(median(&data), 0.0);
    }

    #[test]
    fn test_median_with_decimals() {
        let data = [1.5, 2.5, 3.5, 4.5];
        // (2.5 + 3.5) / 2 = 3.0
        assert_eq!(median(&data), 3.0);
    }

    #[test]
    fn test_median_large_dataset() {
        // Test with larger dataset
        let data: Vec<f64> = (1..=100).map(|x| x as f64).collect();
        // Median of 1..100 (even count) = (50 + 51) / 2 = 50.5
        assert_eq!(median(&data), 50.5);
    }
}
