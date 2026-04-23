use std::borrow::Cow;

use anyhow::Result;
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

pub fn resample_mono_if_needed<'a>(
    audio: &'a [f32],
    from_sr: u32,
    to_sr: u32,
) -> Result<Cow<'a, [f32]>> {
    if from_sr == to_sr {
        return Ok(Cow::Borrowed(audio));
    }

    Ok(Cow::Owned(resample_mono(audio, from_sr, to_sr)?))
}

pub fn resample_mono(audio: &[f32], from_sr: u32, to_sr: u32) -> Result<Vec<f32>> {
    if from_sr == to_sr {
        return Ok(audio.to_vec());
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        to_sr as f64 / from_sr as f64,
        2.0,
        params,
        audio.len().max(1024),
        1,
    )?;

    let input = vec![audio.to_vec()];
    let output = resampler.process(&input, None)?;
    Ok(output.into_iter().next().unwrap_or_default())
}
