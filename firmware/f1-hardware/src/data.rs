use heapless::Vec;

#[derive(Debug)]
pub struct DriverData {
    pub driver_number: u32,
    pub led_num: u32,
}

#[derive(Debug)]
pub struct UpdateFrame {
    pub drivers: Vec<DriverData, 2048>,
}

#[derive(Debug)]
pub struct VisualizationData {
    pub update_rate_ms: u32,
    pub frames: Vec<UpdateFrame, 2048>,
}


pub const VISUALIZATION_DATA: VisualizationData = VisualizationData {
    update_rate_ms: 1000,
    frames: {
        UpdateFrame {
            drivers: [
                DriverData { driver_number: 1, led_num: 1 },
                DriverData { driver_number: 2, led_num: 2 },
                DriverData { driver_number: 3, led_num: 3 },
            ],
        },
        UpdateFrame {
            drivers: [
                DriverData { driver_number: 4, led_num: 4 },
                DriverData { driver_number: 5, led_num: 5 },
                DriverData { driver_number: 6, led_num: 6 },
            ],
        },
    },
};

