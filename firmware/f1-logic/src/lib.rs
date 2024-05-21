#![no_std]
#![no_main]

#[main]
fn main() {
    let a = 5;
    let b = 10;
    let sum = add(a, b);
}

fn add(x: i32, y: i32) -> i32 {
    x + y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(0, 0), 0);
        assert_eq!(add(-1, 1), 0);
        assert_eq!(add(-2, -3), -5);
    }
}
