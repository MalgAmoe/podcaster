//! Real-time visualization widgets for the denoiser

use nih_plug_egui::egui;
use egui_plot::{Line, Plot, PlotPoints, Legend, Corner};
use crate::denoiser_rt::{VisualizationData, BANDS, NUM_BANDS};

/// Draw spectrum analyzer showing current signal vs noise floor
pub fn draw_spectrum_analyzer(ui: &mut egui::Ui, viz_data: &VisualizationData) {
    let bin_to_hz = |bin: usize| -> f64 {
        (bin as f64 * viz_data.sample_rate as f64) / 2048.0
    };

    let linear_to_db = |power: f32| -> f64 {
        10.0 * (power + 1e-10).log10() as f64
    };

    // Build current spectrum line (blue)
    let current_points: PlotPoints = viz_data.current_spectrum
        .iter()
        .enumerate()
        .map(|(bin, &power)| [bin_to_hz(bin), linear_to_db(power)])
        .collect();

    // Build noise floor line (red)
    let noise_points: PlotPoints = viz_data.noise_spectrum
        .iter()
        .enumerate()
        .map(|(bin, &power)| [bin_to_hz(bin), linear_to_db(power)])
        .collect();

    let current_line = Line::new(current_points)
        .color(egui::Color32::from_rgb(100, 150, 255))
        .name("Signal");

    let noise_line = Line::new(noise_points)
        .color(egui::Color32::from_rgb(255, 100, 100))
        .name("Noise Floor");

    Plot::new("spectrum_analyzer")
        .legend(Legend::default().position(Corner::RightTop))
        .height(200.0)
        .x_axis_label("Frequency (Hz)")
        .y_axis_label("Power (dB)")
        .show(ui, |plot_ui| {
            plot_ui.line(current_line);
            plot_ui.line(noise_line);
        });
}

/// Draw gain reduction bars for 9 frequency bands
pub fn draw_gain_reduction_bars(ui: &mut egui::Ui, viz_data: &VisualizationData) {
    ui.label("Gain Reduction (dB)");
    ui.add_space(5.0);

    let available_width = ui.available_width();
    let bar_width = available_width / NUM_BANDS as f32 - 4.0;
    let bar_height = 150.0;

    ui.horizontal(|ui| {
        for (i, &gain_db) in viz_data.band_gain_db.iter().enumerate() {
            ui.vertical(|ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(bar_width, bar_height),
                    egui::Sense::hover()
                );

                // Background
                ui.painter().rect_filled(rect, 2.0, egui::Color32::from_gray(30));

                // Calculate bar height (0dB = no bar, -60dB = full bar)
                let normalized = (gain_db / -60.0).clamp(0.0, 1.0);
                let filled_height = normalized * bar_height;

                if filled_height > 0.0 {
                    let bar_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.min.x, rect.max.y - filled_height),
                        rect.max
                    );

                    // Color: green -> yellow -> red
                    let color = if gain_db > -10.0 {
                        egui::Color32::from_rgb(100, 200, 100)
                    } else if gain_db > -30.0 {
                        egui::Color32::from_rgb(255, 200, 50)
                    } else {
                        egui::Color32::from_rgb(255, 100, 100)
                    };

                    ui.painter().rect_filled(bar_rect, 2.0, color);
                }

                ui.add_space(2.0);
                ui.label(format!("B{}", i));
            });
        }
    });
}

/// Draw SNR table for 9 frequency bands
pub fn draw_snr_table(ui: &mut egui::Ui, viz_data: &VisualizationData) {
    ui.label("Signal-to-Noise Ratio (dB)");
    ui.add_space(5.0);

    egui::Grid::new("snr_grid")
        .spacing([10.0, 5.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Band");
            ui.label("Frequency");
            ui.label("SNR (dB)");
            ui.end_row();

            for (i, &snr_db) in viz_data.band_snr_db.iter().enumerate() {
                let (start_hz, end_hz) = BANDS[i];

                ui.label(format!("{}", i));

                // Format frequency range
                let freq_label = if end_hz >= 1000.0 {
                    format!("{:.0}-{:.0}kHz", start_hz / 1000.0, end_hz / 1000.0)
                } else {
                    format!("{:.0}-{:.0}Hz", start_hz, end_hz)
                };
                ui.label(freq_label);

                // Color-code SNR
                let color = if snr_db > 20.0 {
                    egui::Color32::from_rgb(100, 255, 100)
                } else if snr_db > 10.0 {
                    egui::Color32::from_rgb(255, 255, 100)
                } else if snr_db > 0.0 {
                    egui::Color32::from_rgb(255, 200, 100)
                } else {
                    egui::Color32::from_rgb(255, 100, 100)
                };

                ui.colored_label(color, format!("{:+.1}", snr_db));
                ui.end_row();
            }
        });
}
