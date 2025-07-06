use clap::Parser;
use log::info;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct SweepParams {
    /// RX VGA (baseband) gain, 0-62dB, 2dB steps
    #[arg(short = 'g', long, default_value_t = 14)]
    pub gain: u8,

    /// RX LNA (IF) gain, 0-40dB, 8dB steps
    #[arg(short = 'l', long, default_value_t = 32)]
    pub lna_gain: u8,

    /// FFT bin width (frequency resolution) in Hz, 2445-5000000
    #[arg(short = 'w', long, default_value_t = 10000)]
    pub bin_width: u32,

    /// Minimum frequency in MHz
    #[arg(long, default_value_t = 2400)]
    pub freq_min: u32,

    /// Maximum frequency in MHz
    #[arg(long, default_value_t = 2500)]
    pub freq_max: u32,

    /// RX RF amplifier 1=Enable, 0=Disable
    #[arg(short = 'a', long, default_value_t = 0)]
    pub amp_enable: u8,

    /// Antenna port power, 1=Enable, 0=Disable
    #[arg(short = 'p', long, default_value_t = 0)]
    pub antenna_enable: u8,

    /// Maximum dB value for visualization
    #[arg(long, default_value_t = -10.0)]
    pub max_db: f32,

    /// Minimum dB value for visualization
    #[arg(long, default_value_t = -90.0)]
    pub min_db: f32,
}

impl SweepParams {
    pub fn PrettyPrint(&self) {
        info!("Sweep Parameters:");
        info!(
            "RX gain: IF {} dB, BB {} dB, RF AMP {}",
            self.lna_gain,
            self.gain,
            if self.amp_enable == 1 {
                "ON (14dB)"
            } else {
                "OFF"
            }
        );
        info!("FFT bin width: {}", self.bin_width);
        info!("Minimum frequency: {}", self.freq_min);
        info!("Maximum frequency: {}", self.freq_max);
        info!(
            "Expected image width: ~{}",
            ((self.freq_max - self.freq_min) as f32 * 1_000_000.0) / (self.bin_width as f32)
        );
        info!("Antenna port power: {}", self.antenna_enable);
        info!("Visualization dB range: {} to {}", self.min_db, self.max_db);
    }
}
