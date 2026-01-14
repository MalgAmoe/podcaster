use axum::{extract::State, Json};

use crate::models::{PresetInfo, PresetsResponse};
use crate::processing::list_chains;
use crate::state::AppState;

/// GET /presets - List available chain presets
pub async fn list_presets(State(state): State<AppState>) -> Json<PresetsResponse> {
    let chains = list_chains(&state.config.chains_dir);

    let chain_presets = chains
        .into_iter()
        .map(|(name, description)| PresetInfo { name, description })
        .collect();

    Json(PresetsResponse { chain_presets })
}
