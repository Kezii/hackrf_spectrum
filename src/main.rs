use std::{process::Command, sync::mpsc::channel, thread};

/*
2024-05-31, 16:05:22.927896, 0, 5000000, 1000000.00, 20, -14.27, -26.26, -44.80, -53.68, -67.87
2024-05-31, 16:05:22.927896, 10000000, 15000000, 1000000.00, 20, -60.02, -62.46, -67.40, -58.20, -52.93
2024-05-31, 16:05:22.927896, 5000000, 10000000, 1000000.00, 20, -76.03, -66.01, -63.60, -57.54, -63.42
2024-05-31, 16:05:22.927896, 15000000, 20000000, 1000000.00, 20, -61.49, -109.87, -66.22, -43.76, -32.66
2024-05-31, 16:05:22.927896, 20000000, 25000000, 1000000.00, 20, -58.07, -58.97, -58.29, -60.00, -57.92
2024-05-31, 16:05:22.927896, 30000000, 35000000, 1000000.00, 20, -62.59, -63.77, -53.58, -49.85, -48.13
 */

// Date, Time, Hz Low, Hz High, Hz bin width, Num Samples, dB, dB, dB, dB, dB, [...], dB

#[derive(Debug)]
struct SweepLine {
    date: String,
    time: String,
    hz_low: u64,
    hz_high: u64,
    hz_bin_width: f32,
    num_samples: u32,
    db: Vec<f32>,
}

impl SweepLine {
    fn parse_from_line(line: &str) -> Self {
        let parts: Vec<&str> = line.split(",").map(|x| x.trim()).collect();
        let date = parts[0].to_string();
        let time = parts[1].to_string();
        let hz_low = parts[2].parse::<u64>().unwrap();
        let hz_high = parts[3].parse::<u64>().unwrap();
        let hz_bin_width = parts[4].parse::<f32>().unwrap();
        let num_samples = parts[5].parse::<u32>().unwrap();
        let db: Vec<f32> = parts[6..]
            .iter()
            .map(|x| x.parse::<f32>().unwrap())
            .collect();
        Self {
            date,
            time,
            hz_low,
            hz_high,
            hz_bin_width,
            num_samples,
            db,
        }
    }
}

#[derive(Debug)]
struct Sweep {
    hz_low: u64,
    hz_high: u64,
    hz_bin_width: f32,
    db: Vec<f32>,
}

impl Sweep {
    fn from_lines(lines: Vec<SweepLine>) -> Self {
        let hz_low = lines[0].hz_low;
        let hz_high = lines[lines.len() - 1].hz_high;
        let hz_bin_width = lines[0].hz_bin_width;
        let db: Vec<f32> = lines.iter().flat_map(|x| x.db.iter()).cloned().collect();
        Self {
            hz_low,
            hz_high,
            hz_bin_width,
            db,
        }
    }
}

fn run_sweep() -> Sweep {
    let out = Command::new("sh")
        .arg("-c")
        .arg("hackrf_sweep -1 -g14 -l32 -w 100000 -f 0:3000")
        .output()
        .expect("failed to execute process");

    let lines = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|x| SweepLine::parse_from_line(x))
        .collect::<Vec<SweepLine>>();

    let sweep = Sweep::from_lines(lines);

    sweep
}

fn render_image(sweeps: &Vec<Sweep>, max_db: f32, min_db: f32) {
    let width = sweeps[0].db.len();
    let height = sweeps.len();

    let mut imgbuf = image::ImageBuffer::new(width as u32, height as u32);

    let gradient = colorous::INFERNO;

    for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
        let sweep = &sweeps[y as usize];
        let db = sweep.db[x as usize];
        let db = (db - min_db) / (max_db - min_db);
        let db = gradient.eval_continuous(db.into());
        *pixel = image::Rgb([db.r, db.g, db.b]);
    }

    imgbuf.save("/tmp/sweep.tiff").unwrap();
}

fn red_blue_color_map(db: f32, max_db: f32, min_db: f32) -> (u8, u8, u8) {
    let db = (db - min_db) / (max_db - min_db);
    let db = db * 255.0;
    let db = db as u8;
    let r = 255 - db;
    let b = db;
    (r, 0, b)
}

#[show_image::main]
fn main() {
    let sw = run_sweep();

    let max_db = -10.0;
    let min_db = -90.0;

    // loop {
    //     let sw = run_sweep();
    //     sweeps.push(sw);

    //     render_image(&sweeps, max_db, min_db);
    // }

    let (tx, rx) = channel();

    let sender = thread::spawn(move || loop {
        let sw = run_sweep();
        tx.send(sw).expect("Unable to send on channel");
    });

    let receiver = thread::spawn(move || {
        let mut sweeps = Vec::new();
        loop {
            let sw = rx.recv().expect("Unable to receive on channel");
            sweeps.push(sw);
            render_image(&sweeps, max_db, min_db);
            println!("Rendered image {}", sweeps.len());
        }
    });

    sender.join().expect("The sender thread has panicked");
    receiver.join().expect("The receiver thread has panicked");
}
