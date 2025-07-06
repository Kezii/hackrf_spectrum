use clap::Parser;
use hackrf_spectrum_analyzer::{run_sweep, sweep::Sweep, SweepParams};
use log::info;
use std::{process::Command, sync::mpsc::channel, thread};

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

    println!("{}", sw.pretty_print());

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

            sweeps.push(sw);
            render_image(&sweeps, params.max_db, params.min_db);
        }
    });

    sender.join().expect("The sender thread has panicked");
    receiver.join().expect("The receiver thread has panicked");
}
