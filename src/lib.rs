use clap::Parser;
use log::info;
use std::{process::Command, sync::mpsc::channel, thread};

mod cli;
pub mod sweep;

pub use cli::SweepParams;

use crate::sweep::Sweep;

pub fn run_sweep(params: &SweepParams) -> Sweep {
    /*
    > hackrf_sweep
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
        params.min_freq,
        params.max_freq,
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
