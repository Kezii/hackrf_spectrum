# HackRF Spectrum Analyzer

A spectrum logger using HackRF SDR hardware that generates waterfall visualizations of RF spectrum data.

This software creates a tiff image file with the waterfall representation of the received spectrum.

Uses hackrf_sweep under the hood.

## Requirements

- HackRF One or compatible SDR hardware
- Rust programming environment

## Installation

Clone the repository and build with Cargo

## Usage

```bash
cargo run --release -- [OPTIONS]

Options:
  -g, --gain <GAIN>
          RX VGA (baseband) gain, 0-62dB, 2dB steps [default: 14]
  -l, --lna-gain <LNA_GAIN>
          RX LNA (IF) gain, 0-40dB, 8dB steps [default: 32]
  -w, --bin-width <BIN_WIDTH>
          FFT bin width (frequency resolution) in Hz, 2445-5000000 [default: 100000]
      --freq-min <FREQ_MIN>
          Minimum frequency in MHz [default: 0]
      --freq-max <FREQ_MAX>
          Maximum frequency in MHz [default: 3000]
  -a, --amp-enable <AMP_ENABLE>
          RX RF amplifier 1=Enable, 0=Disable [default: 0]
  -p, --antenna-enable <ANTENNA_ENABLE>
          Antenna port power, 1=Enable, 0=Disable [default: 0]
  -h, --help
          Print help
  -V, --version
          Print version

```

Visualization output is saved to `/tmp/sweep.tiff` and updated in real-time.

## Example

Scan from 2400 MHz to 2500 MHz with 100 kHz resolution:

```bash
cargo run --release -- --freq-min 2400 --freq-max 2500 --bin-width 100000 --gain 20 --lna-gain 16
```