use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Entry {
    x: i32,
    y: i32,
    z: i32,
    driver_number: u16,
    date: heapless::String<32>,
    session_key: u16,
    meeting_key: u16,
}

pub struct DriverInfo {
    pub number: u32,
    pub name: &'static str,
    pub team: &'static str,
    pub color: (u8, u8, u8),
}

pub const DRIVERS: &[DriverInfo] = &[
    DriverInfo {
        number: 1,
        name: "Max Verstappen",
        team: "Red Bull",
        color: (30, 65, 255),
    },
    DriverInfo {
        number: 2,
        name: "Logan Sargeant",
        team: "Williams",
        color: (0, 82, 255),
    },
    DriverInfo {
        number: 4,
        name: "Lando Norris",
        team: "McLaren",
        color: (255, 135, 0),
    },
    DriverInfo {
        number: 10,
        name: "Pierre Gasly",
        team: "Alpine",
        color: (2, 144, 240),
    },
    DriverInfo {
        number: 11,
        name: "Sergio Perez",
        team: "Red Bull",
        color: (30, 65, 255),
    },
    DriverInfo {
        number: 14,
        name: "Fernando Alonso",
        team: "Aston Martin",
        color: (0, 110, 120),
    },
    DriverInfo {
        number: 16,
        name: "Charles Leclerc",
        team: "Ferrari",
        color: (220, 0, 0),
    },
    DriverInfo {
        number: 18,
        name: "Lance Stroll",
        team: "Aston Martin",
        color: (0, 110, 120),
    },
    DriverInfo {
        number: 20,
        name: "Kevin Magnussen",
        team: "Haas",
        color: (160, 207, 205),
    },
    DriverInfo {
        number: 22,
        name: "Yuki Tsunoda",
        team: "AlphaTauri",
        color: (60, 130, 200),
    },
    DriverInfo {
        number: 23,
        name: "Alex Albon",
        team: "Williams",
        color: (0, 82, 255),
    },
    DriverInfo {
        number: 24,
        name: "Zhou Guanyu",
        team: "Stake F1",
        color: (165, 160, 155),
    },
    DriverInfo {
        number: 27,
        name: "Nico Hulkenberg",
        team: "Haas",
        color: (160, 207, 205),
    },
    DriverInfo {
        number: 31,
        name: "Esteban Ocon",
        team: "Alpine",
        color: (2, 144, 240),
    },
    DriverInfo {
        number: 40,
        name: "Liam Lawson",
        team: "AlphaTauri",
        color: (60, 130, 200),
    },
    DriverInfo {
        number: 44,
        name: "Lewis Hamilton",
        team: "Mercedes",
        color: (0, 210, 190),
    },
    DriverInfo {
        number: 55,
        name: "Carlos Sainz",
        team: "Ferrari",
        color: (220, 0, 0),
    },
    DriverInfo {
        number: 63,
        name: "George Russell",
        team: "Mercedes",
        color: (0, 210, 190),
    },
    DriverInfo {
        number: 77,
        name: "Valtteri Bottas",
        team: "Stake F1",
        color: (165, 160, 155),
    },
    DriverInfo {
        number: 81,
        name: "Oscar Piastri",
        team: "McLaren",
        color: (255, 135, 0),
    },
];
