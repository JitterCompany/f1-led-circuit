use f1_logic::data_frame::UpdateFrame;

fn main() -> std::io::Result<()> {
    let bytes = include_bytes!("./output.bin");

    println!("Length of bytes: {}", bytes.len());

    // loop over bytes and deserialize in chunks
    let mut all_frames = Vec::new();
    for chunk in bytes.chunks(UpdateFrame::SERIALIZED_SIZE) {
        let frame = UpdateFrame::try_from_bytes(chunk).unwrap();
        all_frames.push(frame);
    }

    println!("Number of frames: {}", all_frames.len());

    Ok(())
}
