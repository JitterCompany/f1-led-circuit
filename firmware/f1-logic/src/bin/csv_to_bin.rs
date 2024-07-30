use std::fs::File;
use std::io::{BufWriter, Write};

use f1_logic::data_frame::{DriverData, UpdateFrame, NUM_DRIVERS};

fn main() -> std::io::Result<()> {
    // Read the CSV file
    let file_path = "zandvoort_grouped_10hz.csv";
    let mut rdr = csv::Reader::from_path(file_path)?;

    let headers = rdr.headers()?.clone();
    let max_num_frames = 2 * 3600 * 10;
    let mut frames = Vec::with_capacity(max_num_frames);

    for result in rdr.records() {
        let record = result.unwrap();
        let mut frame: [DriverData; NUM_DRIVERS] = Default::default();

        for (j, field) in record.iter().skip(1).enumerate() {
            let driver_number: u8 = headers[j + 1].parse().unwrap();
            let led_num: u8 = field.parse().unwrap();
            frame[j] = DriverData {
                driver_number,
                led_num,
            };
        }

        frames.push(UpdateFrame { frame });
    }

    let mut all_bytes = Vec::with_capacity(3_000_000);
    for frame in frames {
        let frame_bytes = frame.to_bytes().unwrap();
        all_bytes.extend_from_slice(&frame_bytes);
    }

    // Output Binary format
    let bin_file = File::create("output.bin")?;
    let mut writer = BufWriter::new(bin_file);
    // Write contents of all_bytes to bin_file
    writer.write_all(&all_bytes).unwrap();
    writer.flush().unwrap();

    Ok(())
}
