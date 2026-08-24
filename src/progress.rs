use std::{
    io::{self, Write},
    thread,
    time::Duration,
};

const FRAME_DELAY_MS: u64 = 35;

const LARGING_FRAMES: &[&str] = &[
    "( . )( . )",
    "( o )( o )",
    "( O )( O )",
    "( 0 )( 0 )",
    "(  O  )(  O  )",
    "(   O   )(   O   )",
];

const UNLARGING_FRAMES: &[&str] = &[
    "(   O   )(   O   )",
    "(  O  )(  O  )",
    "( 0 )( 0 )",
    "( O )( O )",
    "( o )( o )",
    "( . )( . )",
];

fn render_progress(label: &str, shape: &str, progress: u8) {
    const TRAIL_WIDTH: usize = 12;

    let progress = progress.min(100);
    let position = ((progress as usize) * TRAIL_WIDTH) / 100;

    let mut trail = String::with_capacity(TRAIL_WIDTH * 2);

    for i in 0..TRAIL_WIDTH {
        if i < position {
            trail.push('·');
            trail.push(' ');
        } else {
            trail.push(' ');
            trail.push(' ');
        }
    }

    print!("\r{label} {shape} {trail}{progress:>3}%");

    let _ = io::stdout().flush();
}

pub fn animate_larging() {
    for progress in 0..=100 {
        let frame_index = ((progress as usize) * (LARGING_FRAMES.len() - 1)) / 100;

        render_progress("larging...", LARGING_FRAMES[frame_index], progress);

        thread::sleep(Duration::from_millis(FRAME_DELAY_MS));
    }

    println!();
}

pub fn animate_unlarging() {
    for progress in (0..=100).rev() {
        let normalized = 100 - progress;

        let frame_index = ((normalized as usize) * (UNLARGING_FRAMES.len() - 1)) / 100;

        render_progress("unlarging...", UNLARGING_FRAMES[frame_index], progress);

        thread::sleep(Duration::from_millis(FRAME_DELAY_MS));
    }

    println!();
}
