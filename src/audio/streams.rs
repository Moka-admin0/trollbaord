use anyhow::{Result, bail};
use cpal::traits::DeviceTrait;
use cpal::{
    BuildStreamError, Device, FromSample, InputCallbackInfo, OutputCallbackInfo, Sample, SampleFormat, SizedSample,
    Stream, StreamConfig, SupportedStreamConfig,
};
use std::sync::Arc;

pub type ErrorHandler = Arc<dyn Fn(String) + Send + Sync>;

pub fn open_input(
    device: &Device,
    config: &SupportedStreamConfig,
    on_error: ErrorHandler,
    on_audio: impl FnMut(&[f32]) + Send + 'static,
) -> Result<Stream> {
    let channels = usize::from(config.channels());
    let stream_config = config.config();
    let stream = match config.sample_format() {
        SampleFormat::F32 => input_stream::<f32>(device, &stream_config, channels, on_error, on_audio),
        SampleFormat::I16 => input_stream::<i16>(device, &stream_config, channels, on_error, on_audio),
        SampleFormat::U16 => input_stream::<u16>(device, &stream_config, channels, on_error, on_audio),
        SampleFormat::I32 => input_stream::<i32>(device, &stream_config, channels, on_error, on_audio),
        other => bail!("unsupported sample format {other:?}"),
    };
    Ok(stream?)
}

pub fn open_output(
    device: &Device,
    config: &SupportedStreamConfig,
    on_error: ErrorHandler,
    fill: impl FnMut(&mut [f32]) + Send + 'static,
) -> Result<Stream> {
    let channels = usize::from(config.channels());
    let stream_config = config.config();
    let stream = match config.sample_format() {
        SampleFormat::F32 => output_stream::<f32>(device, &stream_config, channels, on_error, fill),
        SampleFormat::I16 => output_stream::<i16>(device, &stream_config, channels, on_error, fill),
        SampleFormat::U16 => output_stream::<u16>(device, &stream_config, channels, on_error, fill),
        SampleFormat::I32 => output_stream::<i32>(device, &stream_config, channels, on_error, fill),
        other => bail!("unsupported sample format {other:?}"),
    };
    Ok(stream?)
}

fn input_stream<T>(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    on_error: ErrorHandler,
    mut on_audio: impl FnMut(&[f32]) + Send + 'static,
) -> Result<Stream, BuildStreamError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let mut mono = Vec::new();
    device.build_input_stream(
        config,
        move |data: &[T], _: &InputCallbackInfo| {
            mono.clear();
            mono.extend(data.chunks(channels).map(downmix));
            on_audio(&mono);
        },
        move |error| on_error(error.to_string()),
        None,
    )
}

fn output_stream<T>(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    on_error: ErrorHandler,
    mut fill: impl FnMut(&mut [f32]) + Send + 'static,
) -> Result<Stream, BuildStreamError>
where
    T: SizedSample + FromSample<f32>,
{
    let mut mono = Vec::new();
    device.build_output_stream(
        config,
        move |data: &mut [T], _: &OutputCallbackInfo| {
            mono.clear();
            mono.resize(data.len() / channels, 0.0);
            fill(&mut mono);
            for (frame, &sample) in data.chunks_mut(channels).zip(&mono) {
                frame.fill(T::from_sample(sample.clamp(-1.0, 1.0)));
            }
        },
        move |error| on_error(error.to_string()),
        None,
    )
}

fn downmix<T>(frame: &[T]) -> f32
where
    T: Copy,
    f32: FromSample<T>,
{
    let sum: f32 = frame.iter().map(|&sample| f32::from_sample(sample)).sum();
    sum / frame.len() as f32
}
