pub fn format_number(n: i64) -> String {
    n.to_string().chars().rev().collect::<Vec<_>>()
        .chunks(3)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(",")
        .chars()
        .rev()
        .collect()
}

pub fn format_gp(n: i64) -> String {
    format!("{} gp", format_number(n))
}

pub fn format_points(n: i64) -> String {
    format!("{} points", format_number(n))
} 