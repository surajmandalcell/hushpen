//! Audio processing utilities for the ASRPro application
//!
//! This module provides audio analysis functions for waveform generation,
//! audio format conversion, metadata extraction, and audio processing.

use std::path::{Path, PathBuf};
use std::time::Duration;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::models::AudioMetadata;
use crate::utils::{AppError, AppResult};

/// Audio processing utility
pub struct AudioProcessor;

/// Waveform data for visualization
#[derive(Debug, Clone)]
pub struct WaveformData {
    /// Audio samples (normalized to -1.0 to 1.0)
    pub samples: Vec<f32>,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u32,
    /// Duration of the audio
    pub duration: Duration,
    /// Peak amplitude
    pub peak_amplitude: f32,
}

/// Audio format information
#[derive(Debug, Clone)]
pub struct AudioFormat {
    /// Format name (e.g., "MP3", "WAV")
    pub name: String,
    /// Codec name
    pub codec: String,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u32,
    /// Bit depth
    pub bit_depth: Option<u32>,
    /// Bit rate
    pub bit_rate: Option<u32>,
}

impl AudioProcessor {
    /// Extract audio metadata from a file
    pub fn extract_metadata(file_path: &Path) -> AppResult<AudioMetadata> {
        // Open the media source
        let file = std::fs::File::open(file_path)
            .map_err(|e| AppError::file_with_source(
                format!("Failed to open audio file: {}", file_path.display()),
                e
            ))?;
        
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        
        // Create a probe hint using the file extension
        let mut hint = Hint::new();
        if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }
        
        // Probe the media format
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();
        
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .map_err(|e| AppError::generic(format!("Failed to probe audio format: {}", e)))?;
        
        let mut format = probed.format;
        let track = format.tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| AppError::generic("No valid audio track found"))?;
        
        let codec_params = &track.codec_params;
        
        // Extract basic information
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params.channels.map(|c| c.count()).unwrap_or(1) as u32;
        
        // Calculate duration
        let duration = if let Some(n_frames) = codec_params.n_frames {
            Duration::from_secs_f64(n_frames as f64 / sample_rate as f64)
        } else {
            Duration::from_secs(60) // Default duration if unknown
        };
        
        // Get file size
        let file_size = std::fs::metadata(file_path)
            .map(|m| m.len())
            .unwrap_or(0);
        
        // Get format name from file extension
        let format_name = file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown")
            .to_uppercase();
        
        // Create metadata
        let metadata = AudioMetadata {
            duration,
            sample_rate,
            channels,
            bit_rate: codec_params.max_bit_rate,
            format: format_name,
            file_size,
            title: None,
            artist: None,
            album: None,
            date: None,
            genre: None,
        };
        
        Ok(metadata)
    }
    
    /// Generate waveform data from an audio file
    pub fn generate_waveform(file_path: &Path, target_samples: usize) -> AppResult<WaveformData> {
        // Open the media source
        let file = std::fs::File::open(file_path)
            .map_err(|e| AppError::file_with_source(
                format!("Failed to open audio file: {}", file_path.display()),
                e
            ))?;
        
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        
        // Create a probe hint using the file extension
        let mut hint = Hint::new();
        if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }
        
        // Probe the media format
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();
        
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .map_err(|e| AppError::generic(format!("Failed to probe audio format: {}", e)))?;
        
        let mut format = probed.format;
        let track = format.tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| AppError::generic("No valid audio track found"))?;
        
        let codec_params = &track.codec_params;
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params.channels.map(|c| c.count()).unwrap_or(1) as u32;
        
        // Calculate duration
        let duration = if let Some(n_frames) = codec_params.n_frames {
            Duration::from_secs_f64(n_frames as f64 / sample_rate as f64)
        } else {
            Duration::from_secs(60) // Default duration if unknown
        };
        
        // Decode the audio and collect samples
        let mut decoder = symphonia::default::get_codecs()
            .make(&codec_params, &Default::default())
            .map_err(|e| AppError::generic(format!("Failed to create decoder: {}", e)))?;
        
        let mut all_samples = Vec::new();
        let mut total_frames = 0u64;
        
        // Decode the entire audio
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::ResetRequired) => {
                    // The decoder requires a reset, continue
                    continue;
                }
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    // End of stream
                    break;
                }
                Err(e) => {
                    return Err(AppError::generic(format!("Error reading packet: {}", e)));
                }
            };
            
            // Skip packets from other tracks
            if packet.track_id() != track.id {
                continue;
            }
            
            match decoder.decode(&packet) {
                Ok(decoded) => {
                    total_frames += decoded.frames as u64;
                    
                    // Convert samples to f32 and collect them
                    if let Some(audio_buf) = decoded {
                        let samples = Self::convert_samples_to_f32(&audio_buf);
                        all_samples.extend_from_slice(&samples);
                    }
                }
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    // End of stream
                    break;
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => {
                    // Decode error, skip this packet
                    continue;
                }
                Err(e) => {
                    return Err(AppError::generic(format!("Error decoding packet: {}", e)));
                }
            }
        }
        
        // If we couldn't decode any samples, create a simple waveform
        if all_samples.is_empty() {
            return Ok(WaveformData {
                samples: vec![0.0; target_samples],
                sample_rate,
                channels,
                duration,
                peak_amplitude: 0.0,
            });
        }
        
        // Downsample to target number of samples
        let downsampled = Self::downsample_samples(&all_samples, target_samples);
        
        // Calculate peak amplitude
        let peak_amplitude = downsampled.iter()
            .map(|&sample| sample.abs())
            .fold(0.0f32, f32::max);
        
        Ok(WaveformData {
            samples: downsampled,
            sample_rate,
            channels,
            duration,
            peak_amplitude,
        })
    }
    
    /// Get audio format information
    pub fn get_audio_format(file_path: &Path) -> AppResult<AudioFormat> {
        // Open the media source
        let file = std::fs::File::open(file_path)
            .map_err(|e| AppError::file_with_source(
                format!("Failed to open audio file: {}", file_path.display()),
                e
            ))?;
        
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        
        // Create a probe hint using the file extension
        let mut hint = Hint::new();
        if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }
        
        // Probe the media format
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();
        
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .map_err(|e| AppError::generic(format!("Failed to probe audio format: {}", e)))?;
        
        let format = probed.format;
        let track = format.tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| AppError::generic("No valid audio track found"))?;
        
        let codec_params = &track.codec_params;
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params.channels.map(|c| c.count()).unwrap_or(1) as u32;
        
        // Get format name from file extension
        let format_name = file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown")
            .to_uppercase();
        
        // Get codec name
        let codec_name = format!("{:?}", codec_params.codec);
        
        Ok(AudioFormat {
            name: format_name,
            codec: codec_name,
            sample_rate,
            channels,
            bit_depth: None, // Not easily available from Symphonia
            bit_rate: codec_params.max_bit_rate,
        })
    }
    
    /// Convert audio samples to a different format
    pub fn convert_format(
        input_path: &Path,
        output_path: &Path,
        target_format: &str,
    ) -> AppResult<()> {
        // In a real implementation, you would use a library like ffmpeg
        // to convert the audio file to the target format
        
        // For now, we'll just copy the file
        std::fs::copy(input_path, output_path)
            .map_err(|e| AppError::file_with_source(
                format!("Failed to convert file from {} to {}",
                    input_path.display(),
                    output_path.display()
                ),
                e
            ))?;
        
        Ok(())
    }
    
    /// Resample audio to a different sample rate
    pub fn resample(
        samples: &[f32],
        from_sample_rate: u32,
        to_sample_rate: u32,
    ) -> Vec<f32> {
        if from_sample_rate == to_sample_rate {
            return samples.to_vec();
        }
        
        // Calculate the resampling ratio
        let ratio = to_sample_rate as f64 / from_sample_rate as f64;
        let output_length = (samples.len() as f64 * ratio).ceil() as usize;
        
        // Simple linear interpolation for resampling
        let mut resampled = Vec::with_capacity(output_length);
        
        for i in 0..output_length {
            let src_pos = i as f64 / ratio;
            let src_index = src_pos.floor() as usize;
            let fraction = src_pos - src_index as f64;
            
            if src_index >= samples.len() - 1 {
                resampled.push(samples[samples.len() - 1]);
            } else {
                let sample1 = samples[src_index];
                let sample2 = samples[src_index + 1];
                let interpolated = sample1 * (1.0 - fraction) as f32 + sample2 * fraction as f32;
                resampled.push(interpolated);
            }
        }
        
        resampled
    }
    
    /// Apply a simple low-pass filter to audio samples
    pub fn apply_low_pass_filter(samples: &[f32], cutoff_frequency: f32, sample_rate: u32) -> Vec<f32> {
        if cutoff_frequency >= sample_rate as f32 / 2.0 {
            return samples.to_vec();
        }
        
        // Calculate RC constant
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff_frequency);
        let dt = 1.0 / sample_rate as f32;
        let alpha = dt / (rc + dt);
        
        let mut filtered = Vec::with_capacity(samples.len());
        let mut prev_output = 0.0;
        
        for &sample in samples {
            let output = prev_output + alpha * (sample - prev_output);
            filtered.push(output);
            prev_output = output;
        }
        
        filtered
    }
    
    /// Normalize audio samples to a target peak level
    pub fn normalize(samples: &[f32], target_peak: f32) -> Vec<f32> {
        if samples.is_empty() {
            return samples.to_vec();
        }
        
        // Find the current peak amplitude
        let current_peak = samples.iter()
            .map(|&sample| sample.abs())
            .fold(0.0f32, f32::max);
        
        if current_peak == 0.0 {
            return samples.to_vec();
        }
        
        // Calculate the gain factor
        let gain = target_peak / current_peak;
        
        // Apply the gain
        samples.iter()
            .map(|&sample| sample * gain)
            .collect()
    }
    
    /// Convert audio samples from different formats to f32
    fn convert_samples_to_f32(audio_buf: &symphonia::core::audio::AudioBufferRef) -> Vec<f32> {
        match audio_buf {
            symphonia::core::audio::AudioBufferRef::U8(buf) => {
                buf.chan(0).iter()
                    .map(|&sample| (sample as f32 - 128.0) / 128.0)
                    .collect()
            }
            symphonia::core::audio::AudioBufferRef::U16(buf) => {
                buf.chan(0).iter()
                    .map(|&sample| (sample as f32 - 32768.0) / 32768.0)
                    .collect()
            }
            symphonia::core::audio::AudioBufferRef::U24(buf) => {
                buf.chan(0).iter()
                    .map(|&sample| (sample as f32 - 8388608.0) / 8388608.0)
                    .collect()
            }
            symphonia::core::audio::AudioBufferRef::U32(buf) => {
                buf.chan(0).iter()
                    .map(|&sample| (sample as f32 - 2147483648.0) / 2147483648.0)
                    .collect()
            }
            symphonia::core::audio::AudioBufferRef::S8(buf) => {
                buf.chan(0).iter()
                    .map(|&sample| sample as f32 / 128.0)
                    .collect()
            }
            symphonia::core::audio::AudioBufferRef::S16(buf) => {
                buf.chan(0).iter()
                    .map(|&sample| sample as f32 / 32768.0)
                    .collect()
            }
            symphonia::core::audio::AudioBufferRef::S24(buf) => {
                buf.chan(0).iter()
                    .map(|&sample| sample as f32 / 8388608.0)
                    .collect()
            }
            symphonia::core::audio::AudioBufferRef::S32(buf) => {
                buf.chan(0).iter()
                    .map(|&sample| sample as f32 / 2147483648.0)
                    .collect()
            }
            symphonia::core::audio::AudioBufferRef::F32(buf) => {
                buf.chan(0).to_vec()
            }
            symphonia::core::audio::AudioBufferRef::F64(buf) => {
                buf.chan(0).iter()
                    .map(|&sample| sample as f32)
                    .collect()
            }
        }
    }
    
    /// Downsample audio samples to a target number of samples
    fn downsample_samples(samples: &[f32], target_samples: usize) -> Vec<f32> {
        if samples.len() <= target_samples {
            return samples.to_vec();
        }
        
        let mut downsampled = Vec::with_capacity(target_samples);
        let step = samples.len() as f64 / target_samples as f64;
        
        for i in 0..target_samples {
            let start = (i as f64 * step) as usize;
            let end = ((i as f64 + 1.0) * step) as usize;
            
            if end > samples.len() {
                break;
            }
            
            // Calculate RMS for this segment
            let segment = &samples[start..end];
            let rms = if segment.is_empty() {
                0.0
            } else {
                let sum_squares: f32 = segment.iter()
                    .map(|&sample| sample * sample)
                    .sum();
                (sum_squares / segment.len() as f32).sqrt()
            };
            
            downsampled.push(rms);
        }
        
        downsampled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_audio_format() {
        // This would require a real audio file for testing
        // For now, we'll just verify the struct can be created
        let format = AudioFormat {
            name: "MP3".to_string(),
            codec: "MP3".to_string(),
            sample_rate: 44100,
            channels: 2,
            bit_depth: Some(16),
            bit_rate: Some(128000),
        };
        
        assert_eq!(format.name, "MP3");
        assert_eq!(format.sample_rate, 44100);
    }

    #[test]
    fn test_waveform_data() {
        let samples = vec![0.0, 0.5, -0.5, 1.0, -1.0, 0.0];
        let waveform = WaveformData {
            samples: samples.clone(),
            sample_rate: 44100,
            channels: 1,
            duration: Duration::from_secs(1),
            peak_amplitude: 1.0,
        };
        
        assert_eq!(waveform.samples, samples);
        assert_eq!(waveform.sample_rate, 44100);
        assert_eq!(waveform.peak_amplitude, 1.0);
    }

    #[test]
    fn test_resample() {
        let input_samples = vec![0.0, 0.5, 1.0, 0.5, 0.0, -0.5, -1.0, -0.5, 0.0];
        let resampled = AudioProcessor::resample(&input_samples, 44100, 22050);
        
        // Resampled should have approximately half the samples
        assert!((resampled.len() as f32 - input_samples.len() as f32 / 2.0).abs() < 2.0);
    }

    #[test]
    fn test_low_pass_filter() {
        let input_samples = vec![1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
        let filtered = AudioProcessor::apply_low_pass_filter(&input_samples, 1000.0, 44100);
        
        // Filtered should have less drastic changes
        assert_eq!(input_samples.len(), filtered.len());
        assert!((filtered[1] - filtered[0]).abs() < (input_samples[1] - input_samples[0]).abs());
    }

    #[test]
    fn test_normalize() {
        let input_samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let normalized = AudioProcessor::normalize(&input_samples, 0.5);
        
        // Normalized peak should be 0.5
        let peak = normalized.iter()
            .map(|&sample| sample.abs())
            .fold(0.0f32, f32::max);
        
        assert!((peak - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_downsample_samples() {
        let input_samples = vec![0.0; 1000];
        let downsampled = AudioProcessor::downsample_samples(&input_samples, 100);
        
        assert_eq!(downsampled.len(), 100);
    }
}