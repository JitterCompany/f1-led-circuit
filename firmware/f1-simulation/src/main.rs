mod led_data;
mod driver_info;

use iced::alignment;
use iced::executor;
use iced::theme::{self, Theme};
use iced::time;
use iced::widget::{button, container, row, text, column};
use iced::{
    Alignment, Application, Command, Element, Length, Settings, Subscription,
    widget::canvas::{self, Canvas, Path, Frame, Program}, Color, Point, Size, mouse, Renderer
};
use reqwest::Client;
use serde::Deserialize;
use std::time::{Duration, Instant};
use led_data::{LedCoordinate, LED_DATA, UpdateFrame};
use driver_info::DRIVERS;
use std::f32;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use tokio::time::sleep;

#[derive(Debug, Deserialize)]
struct LocationData {
    x: f32,
    y: f32,
    date: String,
    driver_number: u32,
}

pub fn main() -> iced::Result {
    RaceSimulation::run(Settings::default())
}

struct RaceSimulation {
    elapsed_time: Duration,
    state: SimulationState,
    is_led_on: bool,
    processed_update_frames: Vec<UpdateFrame>,
    frames_to_visualize: Vec<UpdateFrame>,
    fetched_update_frames: Vec<UpdateFrame>,
    current_visualization_frame_index: usize,
    http_client: Client,
    driver_numbers: Vec<u32>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    next_data_fetch_start_time: DateTime<Utc>,
    next_data_fetch_end_time: DateTime<Utc>,
    application_start_time: Instant,
}

enum SimulationState {
    IdleState,
    FetchingDataState,
    VisualizingState { last_tick: Instant },
}

#[derive(Debug, Clone)]
enum SimulationMessage {
    ToggleSimulation,
    ResetSimulation,
    SimulationTick(Instant),
    ToggleLed,
    DriverDataFetched(Result<Vec<UpdateFrame>, String>),
    FetchNextDataBatch,
}

impl Application for RaceSimulation {
    type Message = SimulationMessage;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (RaceSimulation, Command<SimulationMessage>) {
        let start_time = DateTime::parse_from_rfc3339("2023-08-27T12:58:56.200Z").unwrap().with_timezone(&Utc);
        let end_time = start_time + ChronoDuration::minutes(5); // Fetch 5 minutes of data
        let next_data_fetch_start_time = end_time + ChronoDuration::milliseconds(1);
        let next_data_fetch_end_time = next_data_fetch_start_time + ChronoDuration::minutes(5);

        (
            RaceSimulation {
                elapsed_time: Duration::default(),
                state: SimulationState::IdleState,
                is_led_on: false,
                processed_update_frames: vec![],
                frames_to_visualize: vec![],
                fetched_update_frames: vec![],
                current_visualization_frame_index: 0,
                http_client: Client::new(),
                driver_numbers: vec![
                    1, 2, 4, 10, 11, 14, 16, 18, 20, 22, 23, 24, 27, 31, 40, 44, 55, 63, 77, 81,
                ],
                start_time,
                end_time,
                next_data_fetch_start_time,
                next_data_fetch_end_time,
                application_start_time: Instant::now(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("F1-LED-CIRCUIT")
    }

    fn update(&mut self, message: SimulationMessage) -> Command<SimulationMessage> {
        match message {
            SimulationMessage::ToggleSimulation => match self.state {
                SimulationState::IdleState => {
                    println!("[{}] Data fetching started", self.application_start_time.elapsed().as_secs());
                    self.state = SimulationState::FetchingDataState;
                    self.processed_update_frames.clear();
                    self.fetched_update_frames.clear();
                    self.current_visualization_frame_index = 0;
                    return Command::perform(fetch_and_process_driver_data(self.http_client.clone(), self.driver_numbers.clone(), self.start_time, self.end_time), SimulationMessage::DriverDataFetched);
                }
                SimulationState::FetchingDataState => {
                    self.state = SimulationState::IdleState;
                    self.is_led_on = false;
                }
                SimulationState::VisualizingState { .. } => {
                    self.state = SimulationState::IdleState;
                    self.is_led_on = false;
                }
            },
            SimulationMessage::SimulationTick(now) => {
                if let SimulationState::VisualizingState { last_tick } = &mut self.state {
                    self.elapsed_time += now - *last_tick;
                    *last_tick = now;

                    if self.current_visualization_frame_index < self.frames_to_visualize.len() {
                        self.current_visualization_frame_index += 1;
                        println!("[{}] Visualizing frame index {}", self.application_start_time.elapsed().as_secs(), self.current_visualization_frame_index);
                    } else {
                        self.current_visualization_frame_index = 0; // Restart visualization if we reach the end
                    }
                }
            }
            SimulationMessage::ResetSimulation => {
                self.elapsed_time = Duration::default();
                self.is_led_on = false;
                self.current_visualization_frame_index = 0;
                self.processed_update_frames.clear();
                self.frames_to_visualize.clear();
                self.fetched_update_frames.clear();
                println!("[{}] Resetting all frames", self.application_start_time.elapsed().as_secs());
            }
            SimulationMessage::ToggleLed => {
                if !self.frames_to_visualize.is_empty() {
                    self.is_led_on = !self.is_led_on;
                    println!("[{}] Toggling LED state to {}", self.application_start_time.elapsed().as_secs(), self.is_led_on);
                }
            }
            SimulationMessage::DriverDataFetched(Ok(new_frames)) => {
                println!("[{}] Data fetching ended with {} frames", self.application_start_time.elapsed().as_secs(), new_frames.len());
                
                // Append new data to frames_to_visualize without clearing
                self.fetched_update_frames.extend(new_frames);
                self.frames_to_visualize.append(&mut self.fetched_update_frames);
                self.fetched_update_frames.clear();
                
                if !self.frames_to_visualize.is_empty() {
                    self.state = SimulationState::VisualizingState {
                        last_tick: Instant::now(),
                    };
                    println!("[{}] Visualization started with {} frames", self.application_start_time.elapsed().as_secs(), self.frames_to_visualize.len());
                    return Command::perform(wait_and_fetch_next_batch(self.next_data_fetch_start_time, self.next_data_fetch_end_time), |_| SimulationMessage::FetchNextDataBatch);
                } else {
                    self.state = SimulationState::IdleState;
                }
            }
            SimulationMessage::DriverDataFetched(Err(_)) => {
                println!("[{}] Data fetching failed", self.application_start_time.elapsed().as_secs());
                self.state = SimulationState::IdleState;
            }
            SimulationMessage::FetchNextDataBatch => {
                let new_start_time = self.next_data_fetch_start_time;
                let new_end_time = self.next_data_fetch_end_time;
                self.next_data_fetch_start_time = new_end_time + ChronoDuration::milliseconds(1);
                self.next_data_fetch_end_time = self.next_data_fetch_start_time + ChronoDuration::minutes(5);
                println!("[{}] Data fetching started for new time range", self.application_start_time.elapsed().as_secs());
                return Command::perform(fetch_and_process_driver_data(self.http_client.clone(), self.driver_numbers.clone(), new_start_time, new_end_time), SimulationMessage::DriverDataFetched);
            }
        }

        Command::none()
    }

    fn subscription(&self) -> Subscription<SimulationMessage> {
        let tick = match self.state {
            SimulationState::IdleState | SimulationState::FetchingDataState => Subscription::none(),
            SimulationState::VisualizingState { .. } => {
                time::every(Duration::from_millis(100)).map(SimulationMessage::SimulationTick)
            }
        };

        let blink = match self.state {
            SimulationState::IdleState | SimulationState::FetchingDataState => Subscription::none(),
            SimulationState::VisualizingState { .. } => {
                time::every(Duration::from_millis(100)).map(|_| SimulationMessage::ToggleLed)
            }
        };

        Subscription::batch(vec![tick, blink])
    }

    fn view(&self) -> Element<SimulationMessage> {
        if let SimulationState::FetchingDataState = self.state {
            return container(
                text("DOWNLOADING DATA...")
                    .size(50)
                    .horizontal_alignment(alignment::Horizontal::Center)
                    .vertical_alignment(alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .into();
        }

        const MINUTE: u64 = 60;
        const HOUR: u64 = 60 * MINUTE;

        let seconds = self.elapsed_time.as_secs();

        let duration = text(format!(
            "{:0>2}:{:0>2}:{:0>2}.{:0>2}",
            seconds / HOUR,
            (seconds % HOUR) / MINUTE,
            seconds % MINUTE,
            self.elapsed_time.subsec_millis() / 10,
        ))
        .size(40);

        let button = |label| {
            button(
                text(label).horizontal_alignment(alignment::Horizontal::Center),
            )
            .padding(10)
            .width(80)
        };

        let toggle_button = {
            let label = match self.state {
                SimulationState::IdleState | SimulationState::FetchingDataState => "Start",
                SimulationState::VisualizingState { .. } => "Stop",
            };

            button(label).on_press(SimulationMessage::ToggleSimulation)
        };

        let reset_button = button("Reset")
            .style(theme::Button::Destructive)
            .on_press(SimulationMessage::ResetSimulation);

        let content = row![
            container(duration).padding(10),
            container(toggle_button).padding(10),
            container(reset_button).padding(10)
        ]
        .align_items(Alignment::Center)
        .spacing(20);

        let canvas = Canvas::new(LedCircuitGraph {
            led_coordinates: LED_DATA.to_vec(),
            is_led_on: self.is_led_on,
            visualization_frames: self.frames_to_visualize.clone(),
            current_visualization_frame_index: self.current_visualization_frame_index,
        })
        .width(Length::Fill)
        .height(Length::Fill);

        container(column![canvas, content])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .padding(20)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Light
    }
}

struct LedCircuitGraph {
    led_coordinates: Vec<LedCoordinate>,
    is_led_on: bool,
    visualization_frames: Vec<UpdateFrame>,
    current_visualization_frame_index: usize,
}

impl<Message> Program<Message> for LedCircuitGraph {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        _renderer: &Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(_renderer, bounds.size());

        let (min_x, max_x, min_y, max_y) = self.led_coordinates.iter().fold(
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN),
            |(min_x, max_x, min_y, max_y), led| {
                (
                    min_x.min(led.x_led),
                    max_x.max(led.x_led),
                    min_y.min(led.y_led),
                    max_y.max(led.y_led),
                )
            },
        );

        let width = max_x - min_x;
        let height = max_y - min_y;

        // Apply padding
        let padding = 50.0;
        let scale_x = (bounds.width - 2.0 * padding) / width;
        let scale_y = (bounds.height - 2.0 * padding) / height;

        // Draw the LED rectangles
        if !self.visualization_frames.is_empty() {
            let frame_data = &self.visualization_frames[self.current_visualization_frame_index];

            for led in &self.led_coordinates {
                let x = (led.x_led - min_x) * scale_x + padding;
                let y = bounds.height - (led.y_led - min_y) * scale_y - padding;

                let color = frame_data
                    .led_states
                    .iter()
                    .find(|(num, _)| *num == led.led_number)
                    .map(|(_, col)| Color::from_rgb8(col.0, col.1, col.2))
                    .unwrap_or(Color::from_rgb(0.0, 0.0, 0.0));

                let point = Path::rectangle(Point::new(x, y), Size::new(10.0, 10.0));
                frame.fill(&point, color);
            }
        }

        vec![frame.into_geometry()]
    }
}

async fn fetch_and_process_driver_data(client: Client, driver_numbers: Vec<u32>, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Vec<UpdateFrame>, String> {
    let session_key = "9149";

    let mut all_data: Vec<LocationData> = Vec::new();

    for driver_number in driver_numbers {
        let mut fetched_entries = 0;

        while fetched_entries < 20 {
            let url = format!(
                "https://api.openf1.org/v1/location?session_key={}&driver_number={}&date>{}&date<{}",
                session_key, driver_number, start_time.to_rfc3339(), end_time.to_rfc3339(),
            );
            //eprintln!("url: {}", url);
            let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
            if resp.status().is_success() {
                let data: Vec<LocationData> = resp.json().await.map_err(|e| e.to_string())?;
                let valid_data: Vec<LocationData> = data.into_iter().filter(|d| d.x != 0.0 && d.y != 0.0).collect();
                fetched_entries += valid_data.len();
                all_data.extend(valid_data);
                println!("[{}] Fetched {} entries for driver {}", Utc::now(), fetched_entries, driver_number);
            } else {
                eprintln!(
                    "Failed to fetch data for driver {}: HTTP {}",
                    driver_number,
                    resp.status()
                );
                break;
            }
        }
    }

    // Sort the data by the date field
    all_data.sort_by_key(|d| d.date.clone());

    let mut update_frames = Vec::<UpdateFrame>::new();
    let mut current_frame: Option<UpdateFrame> = None;

    for data in all_data {
        let timestamp = DateTime::parse_from_rfc3339(&data.date).map_err(|e| e.to_string())?.timestamp_millis() as u64;
        let x = data.x;
        let y = data.y;
        let driver_number = data.driver_number;

        let driver = match DRIVERS.iter().find(|d| d.number == driver_number) {
            Some(d) => d,
            None => {
                eprintln!("Driver not found for number: {}", driver_number);
                continue;
            }
        };

        let color = driver.color;

        let nearest_led = LED_DATA.iter()
            .min_by(|a, b| {
                let dist_a = ((a.x_led - x).powi(2) + (a.y_led - y).powi(2)).sqrt();
                let dist_b = ((b.x_led - x).powi(2) + (b.y_led - y).powi(2)).sqrt();
                dist_a.partial_cmp(&dist_b).unwrap()
            })
            .unwrap();

        if let Some(frame) = &mut current_frame {
            if frame.timestamp == timestamp {
                frame.set_led_state(nearest_led.led_number, color);
            } else {
                update_frames.push(frame.clone());
                current_frame = Some(UpdateFrame::new(timestamp));
                current_frame.as_mut().unwrap().set_led_state(nearest_led.led_number, color);
                println!("[{}] Created new frame for timestamp {}", Utc::now(), timestamp);
            }
        } else {
            current_frame = Some(UpdateFrame::new(timestamp));
            current_frame.as_mut().unwrap().set_led_state(nearest_led.led_number, color);
            println!("[{}] Created initial frame for timestamp {}", Utc::now(), timestamp);
        }
    }

    if let Some(frame) = current_frame {
        update_frames.push(frame);
    }

    println!("[{}] Total update frames created: {}", Utc::now(), update_frames.len());

    Ok(update_frames)
}

async fn wait_and_fetch_next_batch(start_time: DateTime<Utc>, end_time: DateTime<Utc>) {
    let visualization_time = Duration::from_secs(60);
    println!("[{}] Waiting for {} seconds before next fetch", Utc::now(), visualization_time.as_secs());
    sleep(visualization_time).await;
}
