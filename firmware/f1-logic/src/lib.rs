#![no_std]
#![no_main]

use core::panic::PanicInfo;
use riscv_rt::entry;

#[entry]
fn main() -> ! {
    let a = 5;
    let b = 10;
    let _sum = add(a, b);
    loop {}
}

fn add(x: i32, y: i32) -> i32 {
    x + y
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// Conditionally compile tests only when std is available
#[cfg(test)]
mod tests {
    use super::*;

    // Use std for testing purposes
    extern crate std;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(0, 0), 0);
        assert_eq!(add(-1, 1), 0);
        assert_eq!(add(-2, -3), -5);
    }
}
