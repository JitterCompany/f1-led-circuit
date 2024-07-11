//Handmade dataset - sample of 10 frames

#[derive(Debug)]
pub struct DriverData {
    pub driver_number: u32,
    pub led_num: u32,
}

#[derive(Debug)]
pub struct UpdateFrame {
    pub drivers: [Option<DriverData>; 20],
}

#[derive(Debug)]
pub struct VisualizationData {
    pub update_rate_ms: u32,
    pub frames: [UpdateFrame; 10],
}

pub const VISUALIZATION_DATA: VisualizationData = VisualizationData {
    update_rate_ms: 500,
    frames: [
        //Frame 1
        UpdateFrame {
            drivers: [
                Some(DriverData {
                    driver_number: 1,
                    led_num: 25,
                }),
                Some(DriverData {
                    driver_number: 2,
                    led_num: 29,
                }),
                Some(DriverData {
                    driver_number: 4,
                    led_num: 47,
                }),
                Some(DriverData {
                    driver_number: 10,
                    led_num: 40,
                }),
                Some(DriverData {
                    driver_number: 11,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 14,
                    led_num: 13,
                }),
                Some(DriverData {
                    driver_number: 16,
                    led_num: 65,
                }),
                Some(DriverData {
                    driver_number: 18,
                    led_num: 11,
                }),
                Some(DriverData {
                    driver_number: 20,
                    led_num: 53,
                }),
                Some(DriverData {
                    driver_number: 22,
                    led_num: 9,
                }),
                Some(DriverData {
                    driver_number: 23,
                    led_num: 50,
                }),
                Some(DriverData {
                    driver_number: 24,
                    led_num: 85,
                }),
                Some(DriverData {
                    driver_number: 27,
                    led_num: 39,
                }),
                Some(DriverData {
                    driver_number: 31,
                    led_num: 95,
                }),
                Some(DriverData {
                    driver_number: 40,
                    led_num: 18,
                }),
                Some(DriverData {
                    driver_number: 44,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 55,
                    led_num: 75,
                }),
                Some(DriverData {
                    driver_number: 63,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 77,
                    led_num: 57,
                }),
                Some(DriverData {
                    driver_number: 81,
                    led_num: 60,
                }),
            ],
        },
        //Frame 2
        UpdateFrame {
            drivers: [
                Some(DriverData {
                    driver_number: 1,
                    led_num: 25,
                }),
                Some(DriverData {
                    driver_number: 2,
                    led_num: 30,
                }),
                Some(DriverData {
                    driver_number: 4,
                    led_num: 47,
                }),
                Some(DriverData {
                    driver_number: 10,
                    led_num: 41,
                }),
                Some(DriverData {
                    driver_number: 11,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 14,
                    led_num: 14,
                }),
                Some(DriverData {
                    driver_number: 16,
                    led_num: 66,
                }),
                Some(DriverData {
                    driver_number: 18,
                    led_num: 11,
                }),
                Some(DriverData {
                    driver_number: 20,
                    led_num: 53,
                }),
                Some(DriverData {
                    driver_number: 22,
                    led_num: 9,
                }),
                Some(DriverData {
                    driver_number: 23,
                    led_num: 50,
                }),
                Some(DriverData {
                    driver_number: 24,
                    led_num: 86,
                }),
                Some(DriverData {
                    driver_number: 27,
                    led_num: 40,
                }),
                Some(DriverData {
                    driver_number: 31,
                    led_num: 96,
                }),
                Some(DriverData {
                    driver_number: 40,
                    led_num: 19,
                }),
                Some(DriverData {
                    driver_number: 44,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 55,
                    led_num: 75,
                }),
                Some(DriverData {
                    driver_number: 63,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 77,
                    led_num: 57,
                }),
                Some(DriverData {
                    driver_number: 81,
                    led_num: 60,
                }),
            ],
        },
        //Frame 3
        UpdateFrame {
            drivers: [
                Some(DriverData {
                    driver_number: 1,
                    led_num: 26,
                }),
                Some(DriverData {
                    driver_number: 2,
                    led_num: 30,
                }),
                Some(DriverData {
                    driver_number: 4,
                    led_num: 47,
                }),
                Some(DriverData {
                    driver_number: 10,
                    led_num: 42,
                }),
                Some(DriverData {
                    driver_number: 11,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 14,
                    led_num: 14,
                }),
                Some(DriverData {
                    driver_number: 16,
                    led_num: 66,
                }),
                Some(DriverData {
                    driver_number: 18,
                    led_num: 11,
                }),
                Some(DriverData {
                    driver_number: 20,
                    led_num: 54,
                }),
                Some(DriverData {
                    driver_number: 22,
                    led_num: 10,
                }),
                Some(DriverData {
                    driver_number: 23,
                    led_num: 50,
                }),
                Some(DriverData {
                    driver_number: 24,
                    led_num: 86,
                }),
                Some(DriverData {
                    driver_number: 27,
                    led_num: 41,
                }),
                Some(DriverData {
                    driver_number: 31,
                    led_num: 96,
                }),
                Some(DriverData {
                    driver_number: 40,
                    led_num: 19,
                }),
                Some(DriverData {
                    driver_number: 44,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 55,
                    led_num: 76,
                }),
                Some(DriverData {
                    driver_number: 63,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 77,
                    led_num: 57,
                }),
                Some(DriverData {
                    driver_number: 81,
                    led_num: 61,
                }),
            ],
        },
        //Frame 4
        UpdateFrame {
            drivers: [
                Some(DriverData {
                    driver_number: 1,
                    led_num: 26,
                }),
                Some(DriverData {
                    driver_number: 2,
                    led_num: 31,
                }),
                Some(DriverData {
                    driver_number: 4,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 10,
                    led_num: 42,
                }),
                Some(DriverData {
                    driver_number: 11,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 14,
                    led_num: 14,
                }),
                Some(DriverData {
                    driver_number: 16,
                    led_num: 66,
                }),
                Some(DriverData {
                    driver_number: 18,
                    led_num: 11,
                }),
                Some(DriverData {
                    driver_number: 20,
                    led_num: 54,
                }),
                Some(DriverData {
                    driver_number: 22,
                    led_num: 10,
                }),
                Some(DriverData {
                    driver_number: 23,
                    led_num: 50,
                }),
                Some(DriverData {
                    driver_number: 24,
                    led_num: 87,
                }),
                Some(DriverData {
                    driver_number: 27,
                    led_num: 41,
                }),
                Some(DriverData {
                    driver_number: 31,
                    led_num: 1,
                }),
                Some(DriverData {
                    driver_number: 40,
                    led_num: 20,
                }),
                Some(DriverData {
                    driver_number: 44,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 55,
                    led_num: 76,
                }),
                Some(DriverData {
                    driver_number: 63,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 77,
                    led_num: 57,
                }),
                Some(DriverData {
                    driver_number: 81,
                    led_num: 61,
                }),
            ],
        },
        //Frame 5
        UpdateFrame {
            drivers: [
                Some(DriverData {
                    driver_number: 1,
                    led_num: 26,
                }),
                Some(DriverData {
                    driver_number: 2,
                    led_num: 31,
                }),
                Some(DriverData {
                    driver_number: 4,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 10,
                    led_num: 43,
                }),
                Some(DriverData {
                    driver_number: 11,
                    led_num: 49,
                }),
                Some(DriverData {
                    driver_number: 14,
                    led_num: 15,
                }),
                Some(DriverData {
                    driver_number: 16,
                    led_num: 67,
                }),
                Some(DriverData {
                    driver_number: 18,
                    led_num: 12,
                }),
                Some(DriverData {
                    driver_number: 20,
                    led_num: 54,
                }),
                Some(DriverData {
                    driver_number: 22,
                    led_num: 10,
                }),
                Some(DriverData {
                    driver_number: 23,
                    led_num: 50,
                }),
                Some(DriverData {
                    driver_number: 24,
                    led_num: 87,
                }),
                Some(DriverData {
                    driver_number: 27,
                    led_num: 42,
                }),
                Some(DriverData {
                    driver_number: 31,
                    led_num: 1,
                }),
                Some(DriverData {
                    driver_number: 40,
                    led_num: 20,
                }),
                Some(DriverData {
                    driver_number: 44,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 55,
                    led_num: 77,
                }),
                Some(DriverData {
                    driver_number: 63,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 77,
                    led_num: 57,
                }),
                Some(DriverData {
                    driver_number: 81,
                    led_num: 61,
                }),
            ],
        },
        //Frame 6
        UpdateFrame {
            drivers: [
                Some(DriverData {
                    driver_number: 1,
                    led_num: 26,
                }),
                Some(DriverData {
                    driver_number: 2,
                    led_num: 31,
                }),
                Some(DriverData {
                    driver_number: 4,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 10,
                    led_num: 43,
                }),
                Some(DriverData {
                    driver_number: 11,
                    led_num: 49,
                }),
                Some(DriverData {
                    driver_number: 14,
                    led_num: 15,
                }),
                Some(DriverData {
                    driver_number: 16,
                    led_num: 67,
                }),
                Some(DriverData {
                    driver_number: 18,
                    led_num: 12,
                }),
                Some(DriverData {
                    driver_number: 20,
                    led_num: 54,
                }),
                Some(DriverData {
                    driver_number: 22,
                    led_num: 10,
                }),
                Some(DriverData {
                    driver_number: 23,
                    led_num: 50,
                }),
                Some(DriverData {
                    driver_number: 24,
                    led_num: 87,
                }),
                Some(DriverData {
                    driver_number: 27,
                    led_num: 42,
                }),
                Some(DriverData {
                    driver_number: 31,
                    led_num: 1,
                }),
                Some(DriverData {
                    driver_number: 40,
                    led_num: 20,
                }),
                Some(DriverData {
                    driver_number: 44,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 55,
                    led_num: 77,
                }),
                Some(DriverData {
                    driver_number: 63,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 77,
                    led_num: 57,
                }),
                Some(DriverData {
                    driver_number: 81,
                    led_num: 62,
                }),
            ],
        },
        //Frame 7
        UpdateFrame {
            drivers: [
                Some(DriverData {
                    driver_number: 1,
                    led_num: 26,
                }),
                Some(DriverData {
                    driver_number: 2,
                    led_num: 31,
                }),
                Some(DriverData {
                    driver_number: 4,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 10,
                    led_num: 43,
                }),
                Some(DriverData {
                    driver_number: 11,
                    led_num: 49,
                }),
                Some(DriverData {
                    driver_number: 14,
                    led_num: 15,
                }),
                Some(DriverData {
                    driver_number: 16,
                    led_num: 67,
                }),
                Some(DriverData {
                    driver_number: 18,
                    led_num: 12,
                }),
                Some(DriverData {
                    driver_number: 20,
                    led_num: 54,
                }),
                Some(DriverData {
                    driver_number: 22,
                    led_num: 10,
                }),
                Some(DriverData {
                    driver_number: 23,
                    led_num: 50,
                }),
                Some(DriverData {
                    driver_number: 24,
                    led_num: 87,
                }),
                Some(DriverData {
                    driver_number: 27,
                    led_num: 42,
                }),
                Some(DriverData {
                    driver_number: 31,
                    led_num: 1,
                }),
                Some(DriverData {
                    driver_number: 40,
                    led_num: 21,
                }),
                Some(DriverData {
                    driver_number: 44,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 55,
                    led_num: 77,
                }),
                Some(DriverData {
                    driver_number: 63,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 77,
                    led_num: 58,
                }),
                Some(DriverData {
                    driver_number: 81,
                    led_num: 62,
                }),
            ],
        },
        //Frame 8
        UpdateFrame {
            drivers: [
                Some(DriverData {
                    driver_number: 1,
                    led_num: 27,
                }),
                Some(DriverData {
                    driver_number: 2,
                    led_num: 32,
                }),
                Some(DriverData {
                    driver_number: 4,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 10,
                    led_num: 44,
                }),
                Some(DriverData {
                    driver_number: 11,
                    led_num: 49,
                }),
                Some(DriverData {
                    driver_number: 14,
                    led_num: 16,
                }),
                Some(DriverData {
                    driver_number: 16,
                    led_num: 67,
                }),
                Some(DriverData {
                    driver_number: 18,
                    led_num: 13,
                }),
                Some(DriverData {
                    driver_number: 20,
                    led_num: 54,
                }),
                Some(DriverData {
                    driver_number: 22,
                    led_num: 10,
                }),
                Some(DriverData {
                    driver_number: 23,
                    led_num: 50,
                }),
                Some(DriverData {
                    driver_number: 24,
                    led_num: 88,
                }),
                Some(DriverData {
                    driver_number: 27,
                    led_num: 43,
                }),
                Some(DriverData {
                    driver_number: 31,
                    led_num: 2,
                }),
                Some(DriverData {
                    driver_number: 40,
                    led_num: 21,
                }),
                Some(DriverData {
                    driver_number: 44,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 55,
                    led_num: 78,
                }),
                Some(DriverData {
                    driver_number: 63,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 77,
                    led_num: 58,
                }),
                Some(DriverData {
                    driver_number: 81,
                    led_num: 62,
                }),
            ],
        },
        //Frame 9
        UpdateFrame {
            drivers: [
                Some(DriverData {
                    driver_number: 1,
                    led_num: 27,
                }),
                Some(DriverData {
                    driver_number: 2,
                    led_num: 32,
                }),
                Some(DriverData {
                    driver_number: 4,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 10,
                    led_num: 44,
                }),
                Some(DriverData {
                    driver_number: 11,
                    led_num: 49,
                }),
                Some(DriverData {
                    driver_number: 14,
                    led_num: 16,
                }),
                Some(DriverData {
                    driver_number: 16,
                    led_num: 67,
                }),
                Some(DriverData {
                    driver_number: 18,
                    led_num: 13,
                }),
                Some(DriverData {
                    driver_number: 20,
                    led_num: 54,
                }),
                Some(DriverData {
                    driver_number: 22,
                    led_num: 11,
                }),
                Some(DriverData {
                    driver_number: 23,
                    led_num: 50,
                }),
                Some(DriverData {
                    driver_number: 24,
                    led_num: 88,
                }),
                Some(DriverData {
                    driver_number: 27,
                    led_num: 43,
                }),
                Some(DriverData {
                    driver_number: 31,
                    led_num: 2,
                }),
                Some(DriverData {
                    driver_number: 40,
                    led_num: 22,
                }),
                Some(DriverData {
                    driver_number: 44,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 55,
                    led_num: 79,
                }),
                Some(DriverData {
                    driver_number: 63,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 77,
                    led_num: 58,
                }),
                Some(DriverData {
                    driver_number: 81,
                    led_num: 63,
                }),
            ],
        },
        //Frame 10
        UpdateFrame {
            drivers: [
                Some(DriverData {
                    driver_number: 1,
                    led_num: 27,
                }),
                Some(DriverData {
                    driver_number: 2,
                    led_num: 32,
                }),
                Some(DriverData {
                    driver_number: 4,
                    led_num: 48,
                }),
                Some(DriverData {
                    driver_number: 10,
                    led_num: 44,
                }),
                Some(DriverData {
                    driver_number: 11,
                    led_num: 49,
                }),
                Some(DriverData {
                    driver_number: 14,
                    led_num: 17,
                }),
                Some(DriverData {
                    driver_number: 16,
                    led_num: 68,
                }),
                Some(DriverData {
                    driver_number: 18,
                    led_num: 13,
                }),
                Some(DriverData {
                    driver_number: 20,
                    led_num: 54,
                }),
                Some(DriverData {
                    driver_number: 22,
                    led_num: 11,
                }),
                Some(DriverData {
                    driver_number: 23,
                    led_num: 50,
                }),
                Some(DriverData {
                    driver_number: 24,
                    led_num: 88,
                }),
                Some(DriverData {
                    driver_number: 27,
                    led_num: 43,
                }),
                Some(DriverData {
                    driver_number: 31,
                    led_num: 3,
                }),
                Some(DriverData {
                    driver_number: 40,
                    led_num: 22,
                }),
                Some(DriverData {
                    driver_number: 44,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 55,
                    led_num: 79,
                }),
                Some(DriverData {
                    driver_number: 63,
                    led_num: 51,
                }),
                Some(DriverData {
                    driver_number: 77,
                    led_num: 58,
                }),
                Some(DriverData {
                    driver_number: 81,
                    led_num: 63,
                }),
            ],
        },
    ],
};
