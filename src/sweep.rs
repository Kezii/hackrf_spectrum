/*
2024-05-31, 16:05:22.927896, 0, 5000000, 1000000.00, 20, -14.27, -26.26, -44.80, -53.68, -67.87
2024-05-31, 16:05:22.927896, 10000000, 15000000, 1000000.00, 20, -60.02, -62.46, -67.40, -58.20, -52.93
2024-05-31, 16:05:22.927896, 5000000, 10000000, 1000000.00, 20, -76.03, -66.01, -63.60, -57.54, -63.42
2024-05-31, 16:05:22.927896, 15000000, 20000000, 1000000.00, 20, -61.49, -109.87, -66.22, -43.76, -32.66
2024-05-31, 16:05:22.927896, 20000000, 25000000, 1000000.00, 20, -58.07, -58.97, -58.29, -60.00, -57.92
2024-05-31, 16:05:22.927896, 30000000, 35000000, 1000000.00, 20, -62.59, -63.77, -53.58, -49.85, -48.13
 */

// Date, Time, Hz Low, Hz High, Hz bin width, Num Samples, dB, dB, dB, dB, dB, [...], dB

use log::info;

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

// single line of the output of hackrf_sweep
// this is not the complete spectrum, but a single line
impl SweepLine {
    fn from_line(line: &str) -> Self {
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

// the complete spectrum
#[derive(Debug)]
pub struct Sweep {
    pub hz_low: u64,
    pub hz_high: u64,
    pub hz_bin_width: f32,
    pub db: Vec<f32>,
}

impl Sweep {
    fn from_lines(lines: Vec<SweepLine>) -> Self {
        #[cfg(debug_assertions)]
        for line in &lines {
            assert!(line.hz_bin_width == lines[0].hz_bin_width);
        }

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

    pub fn from_hackrf_sweep_output(output: &str) -> Self {
        let lines = output
            .lines()
            .map(SweepLine::from_line)
            .collect::<Vec<SweepLine>>();

        Self::from_lines(lines)
    }

    pub fn pretty_print(&self) -> String{
        format!(
            "Sweep: {} Hz - {} Hz, {} Hz bin width, {} samples, max {} dB, min {} dB",
            self.hz_low,
            self.hz_high,
            self.hz_bin_width,
            self.db.len(),
            self.db
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap(),
            self.db
                .iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap()
        )
    }
}
