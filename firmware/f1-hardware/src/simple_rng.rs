use rand_core::{impls, CryptoRng, Error, RngCore};

pub struct SimpleRng {
    seed: u64,
}

impl SimpleRng {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }
}

impl RngCore for SimpleRng {
    fn next_u32(&mut self) -> u32 {
        // Simple linear congruential generator (LCG) algorithm
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.seed >> 32) as u32
    }

    fn next_u64(&mut self) -> u64 {
        // Generate two 32-bit values and combine them into a 64-bit value
        let upper = self.next_u32() as u64;
        let lower = self.next_u32() as u64;
        (upper << 32) | lower
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        impls::fill_bytes_via_next(self, dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        Ok(self.fill_bytes(dest))
    }
}

impl CryptoRng for SimpleRng {}
