#[derive(Debug)]
pub struct DriverData {
    pub driver_number: u32,
    pub led_num: u32,
}

#[derive(Debug)]
pub struct UpdateFrame {
    pub drivers: [Option<DriverData>; 20]
}

#[derive(Debug)]
pub struct VisualizationData {
    pub update_rate_ms: u32,
    pub frames: [UpdateFrame; 2],
}


pub const VISUALIZATION_DATA: VisualizationData = VisualizationData {
    update_rate_ms: 1000,
    frames: [
        UpdateFrame {
            drivers: [
                Some(DriverData { driver_number: 1, led_num: 1 }),
                Some(DriverData { driver_number: 2, led_num: 2 }),
                Some(DriverData { driver_number: 3, led_num: 3 }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
        },
        UpdateFrame {
            drivers: [
                Some(DriverData { driver_number: 4, led_num: 4 }),
                Some(DriverData { driver_number: 5, led_num: 5 }),
                Some(DriverData { driver_number: 6, led_num: 6 }),
                Some(DriverData { driver_number: 7, led_num: 7 }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
        },
    ],
};

