#[derive(Debug, bincode::Encode, bincode::Decode, PartialEq, Default)]
pub struct DriverData {
    pub driver_number: u8,
    pub led_num: u8,
}

pub const NUM_DRIVERS: usize = 20;

#[derive(Debug, bincode::Encode, bincode::Decode, PartialEq, Default)]
pub struct UpdateFrame {
    pub frame: [DriverData; NUM_DRIVERS],
}

impl UpdateFrame {
    pub const SERIALIZED_SIZE: usize = NUM_DRIVERS * 2;

    pub fn to_bytes(&self) -> Result<[u8; Self::SERIALIZED_SIZE], ()> {
        let mut buf = [0u8; Self::SERIALIZED_SIZE];
        let config = bincode::config::standard()
            .with_little_endian()
            .with_fixed_int_encoding();
        if let Ok(l) = bincode::encode_into_slice(&self, &mut buf[..], config) {
            if l == Self::SERIALIZED_SIZE {
                return Ok(buf);
            }
        }
        Err(())
    }

    pub fn try_from_bytes(buf: &[u8]) -> Result<Self, ()> {
        let config = bincode::config::standard()
            .with_little_endian()
            .with_fixed_int_encoding();
        if let Ok((frame, _n)) = bincode::decode_from_slice(buf, config) {
            Ok(frame)
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// If this test fails it means that the size of the UpdateFrame has changed
    /// and UpdateFrame::SERIALIZED_SIZE needs to be updated
    fn ensure_binary_size() {
        assert_eq!(
            UpdateFrame::SERIALIZED_SIZE,
            core::mem::size_of::<UpdateFrame>()
        );
    }

    #[test]
    fn test_encode_decode() {
        let frame = UpdateFrame {
            frame: [
                DriverData {
                    driver_number: 1,
                    led_num: 2,
                },
                DriverData {
                    driver_number: 3,
                    led_num: 4,
                },
                DriverData {
                    driver_number: 5,
                    led_num: 6,
                },
                DriverData {
                    driver_number: 7,
                    led_num: 8,
                },
                DriverData {
                    driver_number: 9,
                    led_num: 10,
                },
                DriverData {
                    driver_number: 11,
                    led_num: 12,
                },
                DriverData {
                    driver_number: 13,
                    led_num: 14,
                },
                DriverData {
                    driver_number: 15,
                    led_num: 16,
                },
                DriverData {
                    driver_number: 17,
                    led_num: 18,
                },
                DriverData {
                    driver_number: 19,
                    led_num: 20,
                },
                DriverData {
                    driver_number: 21,
                    led_num: 22,
                },
                DriverData {
                    driver_number: 23,
                    led_num: 24,
                },
                DriverData {
                    driver_number: 25,
                    led_num: 26,
                },
                DriverData {
                    driver_number: 27,
                    led_num: 28,
                },
                DriverData {
                    driver_number: 29,
                    led_num: 30,
                },
                DriverData {
                    driver_number: 31,
                    led_num: 32,
                },
                DriverData {
                    driver_number: 33,
                    led_num: 34,
                },
                DriverData {
                    driver_number: 35,
                    led_num: 36,
                },
                DriverData {
                    driver_number: 37,
                    led_num: 38,
                },
                DriverData {
                    driver_number: 39,
                    led_num: 40,
                },
            ],
        };

        let bytes = frame.to_bytes().unwrap();
        let decoded = UpdateFrame::try_from_bytes(&bytes).unwrap();
        assert_eq!(frame, decoded);
    }
}
