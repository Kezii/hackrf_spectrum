use clap::Parser;
use log::info;
use std::{process::Command, sync::mpsc::channel, thread};

mod cli;
mod sweep;

pub use cli::SweepParams;

use crate::sweep::Sweep;

fn run_sweep(params: &SweepParams) -> Sweep {
    /*
    > hackrf_sweep --help
    hackrf_sweep: invalid option -- '-'
    Usage:
            [-h] # this help
            [-d serial_number] # Serial number of desired HackRF
            [-a amp_enable] # RX RF amplifier 1=Enable, 0=Disable
            [-f freq_min:freq_max] # minimum and maximum frequencies in MHz
            [-p antenna_enable] # Antenna port power, 1=Enable, 0=Disable
            [-l gain_db] # RX LNA (IF) gain, 0-40dB, 8dB steps
            [-g gain_db] # RX VGA (baseband) gain, 0-62dB, 2dB steps
            [-w bin_width] # FFT bin width (frequency resolution) in Hz, 2445-5000000
            [-W wisdom_file] # Use FFTW wisdom file (will be created if necessary)
            [-P estimate|measure|patient|exhaustive] # FFTW plan type, default is 'measure'
            [-1] # one shot mode
            [-N num_sweeps] # Number of sweeps to perform
            [-B] # binary output
            [-I] # binary inverse FFT output
            [-n] # keep the same timestamp within a sweep
            -r filename # output file

    Output fields:
            date, time, hz_low, hz_high, hz_bin_width, num_samples, dB, dB, . . .

    */

    let command = format!(
        "hackrf_sweep -1 -g{} -l{} -w {} -f {}:{}{}{}",
        params.gain,
        params.lna_gain,
        params.bin_width,
        params.freq_min,
        params.freq_max,
        if params.amp_enable == 1 { " -a1" } else { "" },
        if params.antenna_enable == 1 {
            " -p1"
        } else {
            ""
        }
    );

    // the hackrf_sweep command returns a number of lines
    // the complete spectrum is the merge of all the lines

    let out = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .expect("failed to execute process");

    let lines = String::from_utf8_lossy(&out.stdout);

    // so we parse all the lines and merge them into a single sweep
    Sweep::from_hackrf_sweep_output(&lines)
}

fn render_image(sweeps: &[Sweep], max_db: f32, min_db: f32) {
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

fn main() {
    env_logger::init();
    let params = SweepParams::parse();

    params.PrettyPrint();
    let sw = run_sweep(&params);

    let (tx, rx) = channel();

    let params_clone = params.clone();
    let sender = thread::spawn(move || loop {
        let sw = run_sweep(&params_clone);
        tx.send(sw).expect("Unable to send on channel");
    });

    let receiver = thread::spawn(move || {
        let mut sweeps = Vec::new();
        loop {
            let sw = rx.recv().expect("Unable to receive on channel");
            sw.pretty_print();
            sweeps.push(sw);
            render_image(&sweeps, params.max_db, params.min_db);
        }
    });

    sender.join().expect("The sender thread has panicked");
    receiver.join().expect("The receiver thread has panicked");
}
