//! Plugin editor/GUI implementation

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::sync::{Arc, Mutex};

use poddyclip::denoiser::VisualizationData;

use crate::params::PoddyclipParams;
use crate::visualizations;

// =============================================================================
// Gain Reduction Meter Helper
// =============================================================================

/// Draw a horizontal gain reduction meter
fn draw_gr_meter(ui: &mut egui::Ui, gain_db: f32, max_reduction: f32, color_scheme: MeterColor) {
    let reduction = -gain_db;

    ui.horizontal(|ui| {
        ui.label(format!("{:.1} dB", gain_db));
        let ratio = (reduction / max_reduction).clamp(0.0, 1.0);
        let available = ui.available_width() - 10.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(available, 20.0), egui::Sense::hover());

        // Background
        ui.painter().rect_filled(rect, 4.0, egui::Color32::from_gray(40));

        // Meter bar
        if ratio > 0.0 {
            let bar_rect = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width() * ratio, rect.height()),
            );
            let color = color_scheme.color_for_ratio(ratio);
            ui.painter().rect_filled(bar_rect, 4.0, color);
        }
    });
}

#[derive(Clone, Copy)]
enum MeterColor {
    Yellow,  // De-Mud
    Green,   // Correction A
    Purple,  // Correction B
    Cyan,    // De-Esser
    Magenta, // Peak Comp
    Red,     // Limiter
}

impl MeterColor {
    fn color_for_ratio(self, ratio: f32) -> egui::Color32 {
        match self {
            MeterColor::Yellow => {
                if ratio > 0.8 { egui::Color32::from_rgb(255, 100, 50) }
                else if ratio > 0.5 { egui::Color32::from_rgb(255, 180, 50) }
                else { egui::Color32::from_rgb(255, 220, 100) }
            }
            MeterColor::Green => {
                if ratio > 0.8 { egui::Color32::from_rgb(50, 200, 100) }
                else if ratio > 0.5 { egui::Color32::from_rgb(80, 220, 120) }
                else { egui::Color32::from_rgb(100, 240, 150) }
            }
            MeterColor::Purple => {
                if ratio > 0.8 { egui::Color32::from_rgb(180, 100, 255) }
                else if ratio > 0.5 { egui::Color32::from_rgb(160, 120, 240) }
                else { egui::Color32::from_rgb(140, 140, 220) }
            }
            MeterColor::Cyan => {
                if ratio > 0.8 { egui::Color32::from_rgb(50, 200, 220) }
                else if ratio > 0.5 { egui::Color32::from_rgb(80, 220, 230) }
                else { egui::Color32::from_rgb(120, 230, 240) }
            }
            MeterColor::Magenta => {
                if ratio > 0.8 { egui::Color32::from_rgb(255, 50, 150) }
                else if ratio > 0.5 { egui::Color32::from_rgb(255, 100, 180) }
                else { egui::Color32::from_rgb(255, 150, 200) }
            }
            MeterColor::Red => {
                if ratio > 0.8 { egui::Color32::from_rgb(255, 50, 50) }
                else if ratio > 0.5 { egui::Color32::from_rgb(255, 100, 50) }
                else { egui::Color32::from_rgb(255, 150, 50) }
            }
        }
    }
}

// =============================================================================
// Editor Creation
// =============================================================================

pub fn create_plugin_editor(
    editor_state: Arc<EguiState>,
    params: Arc<PoddyclipParams>,
    viz_data: Arc<Mutex<VisualizationData>>,
    demud_gain: Arc<Mutex<f32>>,
    correction_a_gain: Arc<Mutex<f32>>,
    correction_b_gain: Arc<Mutex<f32>>,
    deesser_gain: Arc<Mutex<f32>>,
    peakcomp_gain: Arc<Mutex<f32>>,
    limiter_gain: Arc<Mutex<f32>>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        editor_state,
        (),
        |_, _| {},
        move |egui_ctx, setter, _state| {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.heading("Poddyclip - Spectral Subtraction");
                ui.separator();

                ui.columns(2, |columns| {
                    // LEFT COLUMN: Parameters
                    draw_params_column(&mut columns[0], &params, setter);

                    // RIGHT COLUMN: Visualizations
                    draw_viz_column(
                        &mut columns[1],
                        &viz_data,
                        &demud_gain,
                        &correction_a_gain,
                        &correction_b_gain,
                        &deesser_gain,
                        &peakcomp_gain,
                        &limiter_gain,
                    );
                });
            });
        },
    )
}

// =============================================================================
// Parameters Column
// =============================================================================

fn draw_params_column(ui: &mut egui::Ui, params: &PoddyclipParams, setter: &ParamSetter) {
    egui::ScrollArea::vertical().id_salt("params_scroll").show(ui, |ui| {
        // Reset button
        ui.horizontal(|ui| {
            ui.label("Noise Estimation:");
            if ui.button("Reset Noise Floor").clicked() {
                setter.begin_set_parameter(&params.reset_noise);
                setter.set_parameter(&params.reset_noise, !params.reset_noise.value());
                setter.end_set_parameter(&params.reset_noise);
            }
        });

        ui.add_space(15.0);
        ui.separator();

        // Filters
        draw_section(ui, "Filters (HP 80Hz + LP 15.5kHz)", |ui| {
            param_row(ui, "Enable:", &params.filters.enable, setter);
            param_row(ui, "HP Slope:", &params.filters.hp_slope, setter);
        });

        // Subtraction
        draw_section(ui, "Subtraction", |ui| {
            labeled_slider(ui, "Alpha Base:", &params.subtraction.alpha_base, setter);
            labeled_slider(ui, "Alpha Min:", &params.subtraction.alpha_min, setter);
            labeled_slider(ui, "Alpha Max:", &params.subtraction.alpha_max, setter);
            labeled_slider(ui, "Beta (Floor):", &params.subtraction.beta, setter);
        });

        // Noise Estimation
        draw_section(ui, "Noise Estimation", |ui| {
            labeled_slider(ui, "Lambda (Forget Factor):", &params.noise_estimation.lambda, setter);
            labeled_slider(ui, "Spike Threshold:", &params.noise_estimation.spike_threshold, setter);
            labeled_slider(ui, "SFM Speech Threshold:", &params.noise_estimation.sfm_speech, setter);
            labeled_slider(ui, "SFM Noise Threshold:", &params.noise_estimation.sfm_noise, setter);
        });

        // Strength
        draw_section(ui, "Strength (1-5)", |ui| {
            ui.label("Interpolates between CLI presets:");
            ui.label("  1 = Gentle");
            ui.label("  2 = Light");
            ui.label("  3 = Moderate");
            ui.label("  4 = Strong");
            ui.label("  5 = Aggressive");
            ui.add_space(5.0);
            labeled_slider(ui, "Strength:", &params.preset.strength, setter);
        });

        // Dynamic EQ
        draw_section(ui, "Dynamic EQ", |ui| {
            param_row(ui, "De-Mud:", &params.demud.enable, setter);
            labeled_slider(ui, "Frequency:", &params.demud.frequency, setter);
            labeled_slider(ui, "Strength:", &params.demud.macro_val, setter);

            ui.add_space(10.0);

            param_row(ui, "Correction A:", &params.correction_a.enable, setter);
            labeled_slider(ui, "Frequency:", &params.correction_a.frequency, setter);
            labeled_slider(ui, "Strength:", &params.correction_a.macro_val, setter);

            ui.add_space(10.0);

            param_row(ui, "Correction B:", &params.correction_b.enable, setter);
            labeled_slider(ui, "Frequency:", &params.correction_b.frequency, setter);
            labeled_slider(ui, "Strength:", &params.correction_b.macro_val, setter);
        });

        // De-Esser
        draw_section(ui, "De-Esser", |ui| {
            param_row(ui, "Enable:", &params.deesser.enable, setter);
            labeled_slider(ui, "Frequency:", &params.deesser.frequency, setter);
            labeled_slider(ui, "Q:", &params.deesser.q, setter);
            labeled_slider(ui, "Strength:", &params.deesser.strength, setter);
        });

        // Peak Compressor
        draw_section(ui, "Peak Comp (VCA)", |ui| {
            param_row(ui, "Enable:", &params.peakcomp.enable, setter);
            labeled_slider(ui, "Threshold:", &params.peakcomp.threshold, setter);
            labeled_slider(ui, "Ratio:", &params.peakcomp.ratio, setter);
            labeled_slider(ui, "Attack:", &params.peakcomp.attack, setter);
            labeled_slider(ui, "Release:", &params.peakcomp.release, setter);
        });

        // Neve Transformer
        draw_section(ui, "Neve Transformer", |ui| {
            param_row(ui, "Enable:", &params.channel9.enable, setter);
            labeled_slider(ui, "Drive:", &params.channel9.drive, setter);
        });

        // Enhance EQ
        draw_section(ui, "Enhance EQ", |ui| {
            param_row(ui, "Enable:", &params.enhance_eq.enable, setter);
            labeled_slider(ui, "Low-Mid Freq:", &params.enhance_eq.lowmid_freq, setter);
            labeled_slider(ui, "Low-Mid Cut:", &params.enhance_eq.lowmid_gain, setter);
            labeled_slider(ui, "Presence Freq:", &params.enhance_eq.presence_freq, setter);
            labeled_slider(ui, "Presence Gain:", &params.enhance_eq.presence_gain, setter);
            labeled_slider(ui, "Air Gain (dynamic):", &params.enhance_eq.air_gain, setter);
        });

        // ButterComp
        draw_section(ui, "ButterComp", |ui| {
            param_row(ui, "Enable:", &params.buttercomp.enable, setter);
            labeled_slider(ui, "Compress:", &params.buttercomp.compress, setter);
        });

        // TapeGlue
        draw_section(ui, "TapeGlue", |ui| {
            param_row(ui, "Enable:", &params.tape_glue.enable, setter);
            labeled_slider(ui, "Warmth:", &params.tape_glue.warmth, setter);
        });

        // Limiter
        draw_section(ui, "Limiter", |ui| {
            param_row(ui, "Enable:", &params.limiter.enable, setter);
            labeled_slider(ui, "Ceiling:", &params.limiter.ceiling, setter);
        });
    });
}

fn draw_section(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.heading(title);
    ui.add_space(5.0);
    content(ui);
    ui.add_space(15.0);
    ui.separator();
}

fn param_row<P: Param>(ui: &mut egui::Ui, label: &str, param: &P, setter: &ParamSetter) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(widgets::ParamSlider::for_param(param, setter));
    });
}

fn labeled_slider<P: Param>(ui: &mut egui::Ui, label: &str, param: &P, setter: &ParamSetter) {
    ui.label(label);
    ui.add(widgets::ParamSlider::for_param(param, setter));
}

// =============================================================================
// Visualization Column
// =============================================================================

fn draw_viz_column(
    ui: &mut egui::Ui,
    viz_data: &Arc<Mutex<VisualizationData>>,
    demud_gain: &Arc<Mutex<f32>>,
    correction_a_gain: &Arc<Mutex<f32>>,
    correction_b_gain: &Arc<Mutex<f32>>,
    deesser_gain: &Arc<Mutex<f32>>,
    peakcomp_gain: &Arc<Mutex<f32>>,
    limiter_gain: &Arc<Mutex<f32>>,
) {
    ui.vertical(|ui| {
        ui.heading("Analysis");
        ui.add_space(10.0);

        if let Ok(viz) = viz_data.lock() {
            visualizations::draw_spectrum_analyzer(ui, &viz);
            ui.add_space(15.0);
            ui.separator();
            visualizations::draw_gain_reduction_bars(ui, &viz);
        }

        ui.add_space(15.0);
        ui.separator();

        // Gain Reduction Meters
        ui.heading("De-Mud Gain Reduction");
        ui.add_space(5.0);
        let gain = demud_gain.lock().map(|g| *g).unwrap_or(0.0);
        draw_gr_meter(ui, gain, 6.0, MeterColor::Yellow);

        ui.add_space(10.0);

        ui.heading("Correction A Gain Reduction");
        ui.add_space(5.0);
        let gain = correction_a_gain.lock().map(|g| *g).unwrap_or(0.0);
        draw_gr_meter(ui, gain, 6.0, MeterColor::Green);

        ui.add_space(10.0);

        ui.heading("Correction B Gain Reduction");
        ui.add_space(5.0);
        let gain = correction_b_gain.lock().map(|g| *g).unwrap_or(0.0);
        draw_gr_meter(ui, gain, 6.0, MeterColor::Purple);

        ui.add_space(10.0);

        ui.heading("De-Esser Gain Reduction");
        ui.add_space(5.0);
        let gain = deesser_gain.lock().map(|g| *g).unwrap_or(0.0);
        draw_gr_meter(ui, gain, 8.0, MeterColor::Cyan);

        ui.add_space(10.0);

        ui.heading("Peak Comp Gain Reduction");
        ui.add_space(5.0);
        let gain = peakcomp_gain.lock().map(|g| *g).unwrap_or(0.0);
        draw_gr_meter(ui, gain, 12.0, MeterColor::Magenta);

        ui.add_space(10.0);

        ui.heading("Limiter Gain Reduction");
        ui.add_space(5.0);
        let gain = limiter_gain.lock().map(|g| *g).unwrap_or(0.0);
        draw_gr_meter(ui, gain, 12.0, MeterColor::Red);

        if viz_data.lock().is_err() {
            ui.label("Waiting for audio data...");
        }
    });
}
