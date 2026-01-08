//! Real-time visualization widgets for the denoiser

use crate::denoiser::{VisualizationData, NUM_BANDS, WINDOW_SIZE};
use egui_plot::{Corner, Legend, Line, Plot, PlotPoints};
use nih_plug_egui::egui;

/// Draw spectrum analyzer showing current signal vs noise floor
pub fn draw_spectrum_analyzer(ui: &mut egui::Ui, viz_data: &VisualizationData) {
    let bin_to_hz =
        |bin: usize| -> f64 { (bin as f64 * viz_data.sample_rate as f64) / WINDOW_SIZE as f64 };

    let linear_to_db = |power: f32| -> f64 { 10.0 * (power + 1e-10).log10() as f64 };

    // Build current spectrum line (blue)
    let current_points: PlotPoints = viz_data
        .current_spectrum
        .iter()
        .enumerate()
        .map(|(bin, &power)| [bin_to_hz(bin), linear_to_db(power)])
        .collect();

    // Build noise floor line (red)
    let noise_points: PlotPoints = viz_data
        .noise_spectrum
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

/// Draw gain reduction bars for frequency bands
pub fn draw_gain_reduction_bars(ui: &mut egui::Ui, viz_data: &VisualizationData) {
    ui.label("Gain Reduction (dB)");
    ui.add_space(5.0);

    let bar_width = 25.0;
    let spacing = 3.0;
    let bar_height = 150.0;
    let label_height = 15.0;
    let total_height = bar_height + label_height + 5.0;

    // Calculate total width needed
    let total_width = (bar_width + spacing) * NUM_BANDS as f32;

    egui::ScrollArea::horizontal().show(ui, |ui| {
        let (response, painter) =
            ui.allocate_painter(egui::vec2(total_width, total_height), egui::Sense::hover());

        let top_left = response.rect.left_top();

        for i in 0..NUM_BANDS {
            let x = top_left.x + (bar_width + spacing) * i as f32;
            let gain_db = viz_data.band_gain_db[i];

            // Bar background rect
            let bar_rect = egui::Rect::from_min_size(
                egui::pos2(x, top_left.y),
                egui::vec2(bar_width, bar_height),
            );
            painter.rect_filled(bar_rect, 2.0, egui::Color32::from_gray(30));

            // Filled bar (gain reduction)
            let normalized = (gain_db / -60.0).clamp(0.0, 1.0);
            let filled_height = normalized * bar_height;

            if filled_height > 0.0 {
                let filled_rect = egui::Rect::from_min_max(
                    egui::pos2(x, top_left.y + bar_height - filled_height),
                    egui::pos2(x + bar_width, top_left.y + bar_height),
                );

                // Color: green -> yellow -> red
                let color = if gain_db > -10.0 {
                    egui::Color32::from_rgb(100, 200, 100)
                } else if gain_db > -30.0 {
                    egui::Color32::from_rgb(255, 200, 50)
                } else {
                    egui::Color32::from_rgb(255, 100, 100)
                };

                painter.rect_filled(filled_rect, 2.0, color);
            }

            // Label below bar
            let label_pos = egui::pos2(x + bar_width / 2.0, top_left.y + bar_height + 5.0);
            painter.text(
                label_pos,
                egui::Align2::CENTER_TOP,
                format!("B{}", i),
                egui::FontId::default(),
                egui::Color32::from_gray(200),
            );
        }
    });
}
