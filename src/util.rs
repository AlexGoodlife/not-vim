pub fn clamp(a: i64, min: i64, max: i64) -> i64 {
    if a < min {
        min
    } else if a > max {
        max
    } else {
        a
    }
}

