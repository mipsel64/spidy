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
