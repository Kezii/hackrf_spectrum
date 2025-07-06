use std::time::{Duration, Instant};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;

use color_eyre::Result;
use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset},
    DefaultTerminal, Frame,
};

use hackrf_spectrum_analyzer::{run_sweep, sweep::Sweep, SweepParams};
use clap::Parser;
use log::info;

// Event type for our application
enum AppEvent {
    Key(KeyCode),
    NewSweep(Sweep),
    Quit,
}

fn main() -> Result<()> {
    env_logger::init();
    color_eyre::install()?;

    let params = SweepParams::parse();

    // Initialize the terminal
    let terminal = ratatui::init();
    
    // Create channel for events
    let (event_tx, event_rx) = channel();
    
    // Clone the transmitter for the sweep thread
    let sweep_tx = event_tx.clone();

    // Start the sweep thread
    let sweep_thread = {
        let params_clone = params.clone();
        thread::spawn(move || {
            loop {
                let sw = run_sweep(&params_clone);
                if sweep_tx.send(AppEvent::NewSweep(sw)).is_err() {
                    break;
                }
                // No sleep here - run as fast as possible
            }
        })
    };
    
    // Start the event handling thread
    let event_thread = {
        let event_tx = event_tx.clone();
        thread::spawn(move || {
            loop {
                // Poll for events
                if let Ok(true) = event::poll(Duration::from_millis(100)) {
                    if let Ok(Event::Key(key)) = event::read() {
                        if key.code == KeyCode::Char('q') {
                            let _ = event_tx.send(AppEvent::Quit);
                            break;
                        } else {
                            let _ = event_tx.send(AppEvent::Key(key.code));
                        }
                    }
                }
            }
        })
    };

    // Run the app
    let app_result = App::new(event_rx, params).run(terminal);
    
    // Clean up
    ratatui::restore();
    
    // Wait for threads to finish
    let _ = sweep_thread.join();
    let _ = event_thread.join();
    
    app_result
}

struct App {
    event_receiver: Receiver<AppEvent>,
    current_sweep: Option<Sweep>,
    params: SweepParams,
    data_points: Vec<(f64, f64)>,
}

impl App {
    fn new(event_receiver: Receiver<AppEvent>, params: SweepParams) -> Self {
        Self {
            event_receiver,
            current_sweep: None,
            params,
            data_points: Vec::new(),
        }
    }

    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        // Initial draw
        terminal.draw(|frame| self.draw(frame))?;
        
        loop {
            match self.event_receiver.recv() {
                Ok(AppEvent::Key(key)) => {
                    match key {
                        KeyCode::Char('r') => {
                            // Reset data
                            self.data_points.clear();
                            self.current_sweep = None;
                            terminal.draw(|frame| self.draw(frame))?;
                        }
                        _ => {}
                    }
                }
                Ok(AppEvent::NewSweep(sweep)) => {
                    self.current_sweep = Some(sweep);
                    self.update_data_points();
                    terminal.draw(|frame| self.draw(frame))?;
                }
                Ok(AppEvent::Quit) => {
                    return Ok(());
                }
                Err(_) => {
                    // Channel closed
                    return Ok(());
                }
            }
        }
    }

    fn update_data_points(&mut self) {
        if let Some(sweep) = &self.current_sweep {
            // Convert sweep data to data points for the chart
            self.data_points.clear();
            
            let freq_range = (self.params.freq_max - self.params.freq_min) as f64;
            let point_count = sweep.db.len();
            
            for (i, db) in sweep.db.iter().enumerate() {
                let freq = self.params.freq_min as f64 + (i as f64 / point_count as f64) * freq_range;
                self.data_points.push((freq, *db as f64));
            }
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        
        // Create a vertical layout
        let chunks = Layout::vertical([
            Constraint::Length(4),  // Title and info
            Constraint::Fill(1),   // Chart
        ])
        .split(area);
        
        let info_text = if let Some(sweep) = &self.current_sweep {
            format!(
                "{} - {} MHz, {} points, min: {:.1} dB, max: {:.1} dB, Bin width {} Hz",
                self.params.freq_min,
                self.params.freq_max,
                sweep.db.len(),
                sweep.db.iter().cloned().fold(f32::INFINITY, f32::min),
                sweep.db.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
                sweep.hz_bin_width
            )
        } else {
            "Waiting for spectrum data...".to_string()
        };

        let radio_params_text = format!(
            "BB {} dB, IF {} dB, RF AMP {}",
            self.params.gain,
            self.params.lna_gain,
            if self.params.amp_enable == 1 { "ON" } else { "OFF" },
        );
        
        let title_block = Block::bordered()
            .title(Line::from("HackRF Spectrum Analyzer").blue().bold().centered());
        
        frame.render_widget(title_block, chunks[0]);
        
        if !self.data_points.is_empty() {
            let title = Line::from(vec![
                Span::styled(info_text, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(" [q] Quit", Style::default().fg(Color::Gray)),
                Span::styled(" [r] Reset", Style::default().fg(Color::Gray)),
            ]);
            
            // Render info text on first line
            frame.render_widget(
                ratatui::widgets::Paragraph::new(title), 
                Rect::new(chunks[0].x + 2, chunks[0].y + 1, chunks[0].width - 4, 1)
            );
            
            // Render radio parameters on second line
            frame.render_widget(
                ratatui::widgets::Paragraph::new(Line::from(
                    Span::styled(radio_params_text, Style::default().fg(Color::Yellow))
                )), 
                Rect::new(chunks[0].x + 2, chunks[0].y + 2, chunks[0].width - 4, 1)
            );
            
            // Render spectrum chart
            self.render_spectrum_chart(frame, chunks[1]);
        } else {
            // Render info text on first line
            frame.render_widget(
                ratatui::widgets::Paragraph::new(Line::from(Span::styled(info_text, Style::default().fg(Color::Gray)))),
                Rect::new(chunks[0].x + 2, chunks[0].y + 1, chunks[0].width - 4, 1)
            );
            
            // Render radio parameters on second line
            frame.render_widget(
                ratatui::widgets::Paragraph::new(Line::from(
                    Span::styled(radio_params_text, Style::default().fg(Color::Yellow))
                )), 
                Rect::new(chunks[0].x + 2, chunks[0].y + 2, chunks[0].width - 4, 1)
            );
        }
    }

    fn render_spectrum_chart(&self, frame: &mut Frame, area: Rect) {
        if self.data_points.is_empty() {
            return;
        }
        
        // Find the actual min and max frequencies
        let min_freq = self.params.freq_min as f64;
        let max_freq = self.params.freq_max as f64;
        
        // Create x-axis labels
        let mid_freq = (min_freq + max_freq) / 2.0;
        let x_labels = vec![
            Span::styled(
                format!("{:.1}", min_freq),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{:.1}", mid_freq)),
            Span::styled(
                format!("{:.1}", max_freq),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        
        // Create y-axis labels
        let min_db = self.params.min_db as f64;
        let max_db = self.params.max_db as f64;
        let mid_db = (min_db + max_db) / 2.0;
        
        let y_labels = vec![
            Span::styled(
                format!("{:.1}", min_db),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{:.1}", mid_db)),
            Span::styled(
                format!("{:.1}", max_db),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        
        // Create dataset from current sweep data
        let dataset = Dataset::default()
            .name("Spectrum")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&self.data_points);
        
        // Create the chart
        let chart = Chart::new(vec![dataset])
            .block(Block::bordered())
            .x_axis(
                Axis::default()
                    .title("Frequency (MHz)")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([min_freq, max_freq])
                    .labels(x_labels),
            )
            .y_axis(
                Axis::default()
                    .title("Power (dB)")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([min_db, max_db])
                    .labels(y_labels),
            );
        
        frame.render_widget(chart, area);
    }
} 