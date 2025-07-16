use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use color_eyre::Result;
use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span},
    widgets::{
        canvas::{Canvas, Rectangle, Shape},
        Axis, Block, Chart, Dataset,
    },
    DefaultTerminal, Frame,
};

use clap::Parser;
use hackrf_spectrum_analyzer::{
    run_sweep,
    sweep::{DownscaleMode, Sweep},
    SweepParams,
};
use log::info;

// Importa la libreria colorous per la palette di colori
use colorous;

// Event type for our application
enum AppEvent {
    Key(KeyCode),
    NewSweep(Sweep),
    Quit,
}

// Visualization mode
enum VisualizationMode {
    Spectrum,
    Waterfall,
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
    waterfall_history: VecDeque<Sweep>,
    visualization_mode: VisualizationMode,
    max_waterfall_lines: usize,
    downscale_mode: DownscaleMode,
}

impl App {
    fn new(event_receiver: Receiver<AppEvent>, params: SweepParams) -> Self {
        Self {
            event_receiver,
            current_sweep: None,
            params,
            data_points: Vec::new(),
            waterfall_history: VecDeque::new(),
            visualization_mode: VisualizationMode::Waterfall,
            max_waterfall_lines: 200,
            downscale_mode: DownscaleMode::Peak,
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
                            self.waterfall_history.clear();
                            self.current_sweep = None;
                            terminal.draw(|frame| self.draw(frame))?;
                        }
                        KeyCode::Char('m') => {
                            // Toggle visualization mode
                            self.visualization_mode = match self.visualization_mode {
                                VisualizationMode::Spectrum => VisualizationMode::Waterfall,
                                VisualizationMode::Waterfall => VisualizationMode::Spectrum,
                            };
                            terminal.draw(|frame| self.draw(frame))?;
                        }
                        KeyCode::Char('d') => {
                            // Toggle downscale mode
                            self.downscale_mode = match self.downscale_mode {
                                DownscaleMode::Peak => DownscaleMode::Average,
                                DownscaleMode::Average => DownscaleMode::Peak,
                            };
                            terminal.draw(|frame| self.draw(frame))?;
                        }
                        _ => {}
                    }
                }
                Ok(AppEvent::NewSweep(sweep)) => {
                    self.current_sweep = Some(sweep);
                    self.update_data();
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

    fn update_data(&mut self) {
        if let Some(sweep) = &self.current_sweep {
            // Update spectrum data points
            self.data_points.clear();

            let freq_range = (self.params.max_freq - self.params.min_freq) as f64;
            let point_count = sweep.db.len();

            for (i, db) in sweep.db.iter().enumerate() {
                let freq =
                    self.params.min_freq as f64 + (i as f64 / point_count as f64) * freq_range;
                self.data_points.push((freq, *db as f64));
            }

            // Update waterfall history
            self.waterfall_history.push_front(sweep.clone());
            if self.waterfall_history.len() > self.max_waterfall_lines {
                self.waterfall_history.pop_back();
            }
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();

        // Create a vertical layout
        let chunks = Layout::vertical([
            Constraint::Length(4), // Title and info
            Constraint::Fill(1),   // Chart
        ])
        .split(area);

        let info_text = if let Some(sweep) = &self.current_sweep {
            format!(
                "{} - {} MHz, {} points, min: {:.1} dB, max: {:.1} dB, Bin width {} Hz",
                self.params.min_freq,
                self.params.max_freq,
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
            if self.params.amp_enable == 1 {
                "ON"
            } else {
                "OFF"
            },
        );

        // Determine mode text
        let mode_text = match self.visualization_mode {
            VisualizationMode::Spectrum => "Mode: Spectrum",
            VisualizationMode::Waterfall => "Mode: Waterfall",
        };

        let title_block = Block::bordered().title(
            Line::from("HackRF Spectrum Analyzer")
                .blue()
                .bold()
                .centered(),
        );

        frame.render_widget(title_block, chunks[0]);

        if !self.data_points.is_empty() {
            let title = Line::from(vec![
                Span::styled(
                    info_text,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [q] Quit", Style::default().fg(Color::Gray)),
                Span::styled(" [r] Reset", Style::default().fg(Color::Gray)),
                Span::styled(" [m] Mode", Style::default().fg(Color::Gray)),
                Span::styled(" [d] Downscale", Style::default().fg(Color::Gray)),
            ]);

            // Render info text on first line
            frame.render_widget(
                ratatui::widgets::Paragraph::new(title),
                Rect::new(chunks[0].x + 2, chunks[0].y + 1, chunks[0].width - 4, 1),
            );

            // Render radio parameters and mode on second line
            frame.render_widget(
                ratatui::widgets::Paragraph::new(Line::from(vec![
                    Span::styled(radio_params_text, Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!(
                            " - {} - Downscale: {}",
                            mode_text,
                            match self.downscale_mode {
                                DownscaleMode::Peak => "Peak",
                                DownscaleMode::Average => "Average",
                            }
                        ),
                        Style::default().fg(Color::Green),
                    ),
                ])),
                Rect::new(chunks[0].x + 2, chunks[0].y + 2, chunks[0].width - 4, 1),
            );

            // Render visualization based on current mode
            match self.visualization_mode {
                VisualizationMode::Spectrum => self.render_spectrum_chart(frame, chunks[1]),
                VisualizationMode::Waterfall => self.render_waterfall(frame, chunks[1]),
            }
        } else {
            // Render info text on first line
            frame.render_widget(
                ratatui::widgets::Paragraph::new(Line::from(Span::styled(
                    info_text,
                    Style::default().fg(Color::Gray),
                ))),
                Rect::new(chunks[0].x + 2, chunks[0].y + 1, chunks[0].width - 4, 1),
            );

            // Render radio parameters on second line
            frame.render_widget(
                ratatui::widgets::Paragraph::new(Line::from(Span::styled(
                    radio_params_text,
                    Style::default().fg(Color::Yellow),
                ))),
                Rect::new(chunks[0].x + 2, chunks[0].y + 2, chunks[0].width - 4, 1),
            );
        }
    }

    fn render_spectrum_chart(&self, frame: &mut Frame, area: Rect) {
        if self.data_points.is_empty() {
            return;
        }

        // Find the actual min and max frequencies and power levels
        let min_freq = self.params.min_freq as f64;
        let max_freq = self.params.max_freq as f64;
        let min_db = self.params.min_db as f64;
        let max_db = self.params.max_db as f64;

        // Create a block with border and title
        let block = Block::bordered()
            .title("Spectrum")
            .title_alignment(ratatui::layout::Alignment::Center);

        let inner_area = block.inner(area);

        frame.render_widget(block, area);

        // Create a canvas to draw the density-colored spectrum
        let spectrum = Canvas::default()
            .marker(symbols::Marker::HalfBlock)
            .paint(|ctx| {
                let width = inner_area.width as usize;
                let height = inner_area.height as usize;

                if width == 0 || height == 0 || self.data_points.is_empty() {
                    return;
                }

                // Create a 2D grid to count how many points fall into each pixel
                let mut density_grid = vec![vec![0u32; height]; width];
                let mut max_density = 0u32;

                // Map each data point to a pixel and count density
                for &(freq, db) in &self.data_points {
                    // Calculate x position (frequency)
                    let x_pos =
                        ((freq - min_freq) / (max_freq - min_freq) * (width as f64 - 1.0)) as usize;

                    // Calculate y position (power)
                    let y_normalized = (db - min_db) / (max_db - min_db);
                    let y_normalized = y_normalized.clamp(0.0, 1.0);
                    // Use y_normalized directly without inverting
                    let y_pos = (height as f64 * y_normalized) as usize;

                    // Ensure we're within bounds
                    if x_pos < width && y_pos < height && x_pos > 0 && y_pos > 0 {
                        // Increment the density counter for this pixel
                        density_grid[x_pos][y_pos] += 1;
                        max_density = max_density.max(density_grid[x_pos][y_pos]);
                    }
                }

                // Now draw each pixel with color based on its density
                for x in 0..width {
                    for y in 0..height {
                        let density = density_grid[x][y];

                        // Only draw pixels that have at least one point
                        if density > 0 {
                            // Normalize density to 0.0-1.0 range for color
                            let normalized_density = density as f32 / max_density as f32;

                            // Get color based on density
                            let color = self.get_color_from_value(normalized_density);

                            // Draw a single pixel
                            ctx.draw(&Rectangle {
                                x: x as f64,
                                y: y as f64,
                                width: 1.0,
                                height: 1.0,
                                color,
                            });
                        }
                    }
                }
            })
            .x_bounds([0.0, inner_area.width as f64])
            .y_bounds([0.0, inner_area.height as f64]);

        frame.render_widget(spectrum, inner_area);

        // Add frequency labels at the bottom
        let mid_freq = (min_freq + max_freq) / 2.0;

        let labels = vec![
            format!("{:.1} MHz", min_freq),
            format!("{:.1} MHz", mid_freq),
            format!("{:.1} MHz", max_freq),
        ];

        let label_width = 12;
        let spacing = (inner_area.width - 3 * label_width) / 2;

        // Render min frequency label
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                labels[0].clone(),
                Style::default().fg(Color::Gray),
            )),
            Rect::new(
                inner_area.x,
                inner_area.y + inner_area.height - 1,
                label_width,
                1,
            ),
        );

        // Render mid frequency label
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                labels[1].clone(),
                Style::default().fg(Color::Gray),
            )),
            Rect::new(
                inner_area.x + label_width + spacing,
                inner_area.y + inner_area.height - 1,
                label_width,
                1,
            ),
        );

        // Render max frequency label
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                labels[2].clone(),
                Style::default().fg(Color::Gray),
            )),
            Rect::new(
                inner_area.x + 2 * (label_width + spacing),
                inner_area.y + inner_area.height - 1,
                label_width,
                1,
            ),
        );

        // Add dB level labels on the side
        let db_label_height = 1;
        let db_spacing = (inner_area.height - 3 * db_label_height) / 2;

        // Render max dB label
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                format!("{:.1} dB", max_db),
                Style::default().fg(Color::Gray),
            )),
            Rect::new(inner_area.x, inner_area.y, label_width, db_label_height),
        );

        // Render mid dB label
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                format!("{:.1} dB", (min_db + max_db) / 2.0),
                Style::default().fg(Color::Gray),
            )),
            Rect::new(
                inner_area.x,
                inner_area.y + db_label_height + db_spacing,
                label_width,
                db_label_height,
            ),
        );

        // Render min dB label
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                format!("{:.1} dB", min_db),
                Style::default().fg(Color::Gray),
            )),
            Rect::new(
                inner_area.x,
                inner_area.y + 2 * (db_label_height + db_spacing),
                label_width,
                db_label_height,
            ),
        );
    }

    fn render_waterfall(&self, frame: &mut Frame, area: Rect) {
        if self.waterfall_history.is_empty() {
            return;
        }

        let block = Block::bordered()
            .title("Waterfall")
            .title_alignment(ratatui::layout::Alignment::Center);

        let inner_area = block.inner(area);

        frame.render_widget(block, area);

        let waterfall = Canvas::default()
            .marker(symbols::Marker::Block)
            .paint(|ctx| {
                let width = inner_area.width as usize;
                let height = inner_area.height as usize;

                // Calculate how many lines we can display
                let lines_to_show = std::cmp::min(height, self.waterfall_history.len());

                // for each history line
                for y in 0..lines_to_show {
                    let sweep = &self.waterfall_history[y];

                    // Downscale the sweep data to fit the width
                    let downscaled_data = sweep.downscale(width, self.downscale_mode);

                    // Render each point in the downscaled data
                    for (x, &db_value) in downscaled_data.iter().enumerate() {
                        // Normalize db value to 0.0-1.0 range
                        let normalized = (db_value - self.params.min_db)
                            / (self.params.max_db - self.params.min_db);
                        let normalized = normalized.clamp(0.0, 1.0);

                        // Get color from inferno palette
                        let color = self.get_color_from_value(normalized);

                        // Draw a single pixel
                        ctx.draw(&Rectangle {
                            x: x as f64,
                            y: y as f64,
                            width: 1.0,
                            height: 1.0,
                            color,
                        });
                    }
                }
            })
            .x_bounds([0.0, inner_area.width as f64])
            .y_bounds([0.0, inner_area.height as f64]);

        frame.render_widget(waterfall, inner_area);

        // Add frequency labels at the bottom
        let min_freq = self.params.min_freq;
        let max_freq = self.params.max_freq;
        let mid_freq = (min_freq + max_freq) / 2;

        let labels = vec![
            format!("{} MHz", min_freq),
            format!("{} MHz", mid_freq),
            format!("{} MHz", max_freq),
        ];

        let label_width = 10;
        let spacing = (inner_area.width - 3 * label_width) / 2;

        // Render min frequency label
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                labels[0].clone(),
                Style::default().fg(Color::Gray),
            )),
            Rect::new(
                inner_area.x,
                inner_area.y + inner_area.height - 1,
                label_width,
                1,
            ),
        );

        // Render mid frequency label
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                labels[1].clone(),
                Style::default().fg(Color::Gray),
            )),
            Rect::new(
                inner_area.x + label_width + spacing,
                inner_area.y + inner_area.height - 1,
                label_width,
                1,
            ),
        );

        // Render max frequency label
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                labels[2].clone(),
                Style::default().fg(Color::Gray),
            )),
            Rect::new(
                inner_area.x + 2 * (label_width + spacing),
                inner_area.y + inner_area.height - 1,
                label_width,
                1,
            ),
        );
    }

    fn get_color_from_value(&self, value: f32) -> Color {
        // Usa la palette Inferno dalla libreria colorous, come in image_logger.rs
        let gradient = colorous::INFERNO;
        let color = gradient.eval_continuous(value as f64);

        Color::Rgb(color.r, color.g, color.b)
    }
}
