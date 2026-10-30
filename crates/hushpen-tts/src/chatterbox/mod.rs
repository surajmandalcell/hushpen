//! Chatterbox Turbo and Nano on ONNX Runtime: the worker clones the voice of a clip and reads
//! English text with it.
//!
//! A model folder holds four graphs. The speech encoder turns the clip into the conditioning
//! (once for each voice, kept beside the clip; the encoder is opened only for that), the
//! language model writes speech tokens one
//! by one after the conditioning and the text, with a cache of its keys and values, and the
//! decoder turns the tokens into 24 kHz audio. Each piece of the text (see `text`) is spoken
//! on its own, so the first audio comes after the first short piece and a long text never
//! needs more memory than one piece.

mod audio;
mod text;
pub mod tokenizer;
mod voice;

use crate::backend::{Backend, CancelFlag, JobError, LoadError, LoadSpec, Session};
use crate::onnx;
use hushpen_core::chatterbox::{
    DECODER_FILE, DEFAULT_VOICE_FILE, EMBED_TOKENS_FILE, FILES, LANGUAGE_MODEL_FILE, SAMPLE_RATE,
    SPEECH_ENCODER_FILE, TOKENIZER_FILE, conditioning_path, reads_as_english,
};
use hushpen_core::error::{SPEECH_FAILED, SPEECH_NOT_ENGLISH, SPEECH_VOICE_CLIP};
use ort::environment::Environment;
use ort::session::{Session as Graph, SessionInputValue, SessionOutputs};
use ort::value::{DynValue, PrimitiveTensorElementType, Tensor as OrtTensor};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokenizer::Tokenizer;
use voice::{Conditioning, Tensor};

const START_SPEECH: i64 = 6561;
const STOP_SPEECH: i64 = 6562;
/// A short silence. Three of them end the tokens the decoder reads, so the last sound is not
/// cut off.
const SILENCE: i64 = 4299;
const MAX_TOKENS: usize = 1024;
/// The guard on the tokens of one piece: the model writes 25 tokens for each second of
/// speech, and even slow speech stays under one word a second.
const BASE_TOKENS: usize = 50;
const TOKENS_PER_WORD: usize = 25;
/// The same token this many times in a row (two seconds) is a model that is stuck.
const MAX_REPEATS: usize = 50;
/// Logits of tokens already written are divided by this (multiplied when negative), so the
/// model does not loop on one sound.
const REPETITION_PENALTY: f32 = 1.2;
/// The voices whose conditioning stays in memory.
const VOICE_CACHE: usize = 8;

/// Opens ONNX Runtime on the first load, so a worker without it still starts and answers.
pub struct ChatterboxBackend {
    library: PathBuf,
}

impl ChatterboxBackend {
    pub fn new(library: PathBuf) -> Self {
        Self { library }
    }

    /// The version of the ONNX Runtime library, such as `1.28.2`, once it opens.
    pub fn runtime_version(&self) -> Result<String, String> {
        onnx::environment(&self.library)?;
        onnx::version(&self.library)
    }
}

impl Backend for ChatterboxBackend {
    fn load(&self, spec: &LoadSpec) -> Result<Box<dyn Session>, LoadError> {
        let failed = |detail: String| LoadError { detail };
        if let Some(missing) = FILES.iter().find(|file| !spec.dir.join(file).is_file()) {
            return Err(failed(format!("{} has no {missing}", spec.dir.display())));
        }
        let environment = onnx::environment(&self.library).map_err(failed)?;
        let open = |file: &str| onnx::session(environment, &spec.dir.join(file), spec.threads);
        let tokenizer = Tokenizer::read(&spec.dir.join(TOKENIZER_FILE)).map_err(failed)?;
        let graphs = Graphs {
            embed: open(EMBED_TOKENS_FILE).map_err(failed)?,
            language: Language::new(open(LANGUAGE_MODEL_FILE).map_err(failed)?).map_err(failed)?,
            decoder: open(DECODER_FILE).map_err(failed)?,
        };
        let mut session = ChatterboxSession {
            graphs,
            tokenizer,
            environment,
            encoder: spec.dir.join(SPEECH_ENCODER_FILE),
            threads: spec.threads,
            model: spec.model_name(),
            default_voice: spec.dir.join(DEFAULT_VOICE_FILE),
            voices: Vec::new(),
        };
        session.warm_up().map_err(failed)?;
        Ok(Box::new(session))
    }
}

struct ChatterboxSession {
    graphs: Graphs,
    tokenizer: Tokenizer,
    environment: &'static Environment,
    /// The speech encoder graph, opened for a voice without a conditioning file and closed
    /// after it, so its memory is not held for the rare new voice.
    encoder: PathBuf,
    threads: usize,
    /// The folder name, part of the name of each conditioning file.
    model: String,
    default_voice: PathBuf,
    /// The voices used last, the newest at the end.
    voices: Vec<(PathBuf, Arc<Conditioning>)>,
}

impl ChatterboxSession {
    /// Readies the default voice and speaks one word, so the first job does not pay for the
    /// first run of each graph.
    fn warm_up(&mut self) -> Result<(), String> {
        let voice = self.voice(None).map_err(|failure| format!("{failure:?}"))?;
        let ids = self.tokenizer.encode("Hi.");
        let tokens = self
            .graphs
            .tokens(&ids, &voice, 8, &CancelFlag::new())
            .map_err(|failure| format!("{failure:?}"))?;
        self.graphs.decode(&tokens, &voice)?;
        Ok(())
    }

    /// The conditioning of the clip at `path` (the model's own clip for `None`): from memory,
    /// from its file, or from the speech encoder, which then saves it.
    fn voice(&mut self, path: Option<&str>) -> Result<Arc<Conditioning>, JobError> {
        let clip = path.map_or_else(|| self.default_voice.clone(), PathBuf::from);
        if let Some(at) = self.voices.iter().position(|(known, _)| *known == clip) {
            let entry = self.voices.remove(at);
            let conditioning = Arc::clone(&entry.1);
            self.voices.push(entry);
            return Ok(conditioning);
        }
        let bad_clip = |detail: String| JobError::Failed {
            code: SPEECH_VOICE_CLIP,
            detail: format!("{}: {detail}", clip.display()),
        };
        let read = voice::read_clip(&clip).map_err(bad_clip)?;
        let cache = conditioning_path(&clip, &self.model);
        let conditioning = match voice::load(&cache, read.hash) {
            Some(conditioning) => conditioning,
            None => {
                let mut encoder =
                    onnx::session(self.environment, &self.encoder, self.threads).map_err(failed)?;
                let made = condition(&mut encoder, &read.samples).map_err(failed)?;
                // A voice in a folder the worker cannot write to still speaks; it is made
                // again next time.
                let _ = voice::save(&cache, read.hash, &made);
                made
            }
        };
        let conditioning = Arc::new(conditioning);
        if self.voices.len() >= VOICE_CACHE {
            self.voices.remove(0);
        }
        self.voices.push((clip, Arc::clone(&conditioning)));
        Ok(conditioning)
    }
}

fn failed(detail: String) -> JobError {
    JobError::Failed {
        code: SPEECH_FAILED,
        detail,
    }
}

impl Session for ChatterboxSession {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn speak(
        &mut self,
        text: &str,
        voice: Option<&str>,
        speed: f32,
        cancel: &CancelFlag,
        sink: &mut dyn FnMut(&[f32]),
    ) -> Result<(), JobError> {
        if !reads_as_english(text) {
            return Err(JobError::Failed {
                code: SPEECH_NOT_ENGLISH,
                detail: "Chatterbox reads English only".into(),
            });
        }
        let pieces = text::pieces(text);
        if pieces.is_empty() {
            return Ok(());
        }
        let conditioning = self.voice(voice)?;
        for piece in pieces {
            if cancel.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            let ids = self.tokenizer.encode(&piece.text);
            let words = piece.text.split(' ').count();
            let limit = (BASE_TOKENS + TOKENS_PER_WORD * words).min(MAX_TOKENS);
            let tokens = self.graphs.tokens(&ids, &conditioning, limit, cancel)?;
            if cancel.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            if tokens.is_empty() {
                continue;
            }
            let wave = self.graphs.decode(&tokens, &conditioning).map_err(failed)?;
            let out = audio::finish(&wave, SAMPLE_RATE, piece.pause_ms, speed);
            if cancel.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            if !out.is_empty() {
                sink(&out);
            }
        }
        Ok(())
    }
}

struct Graphs {
    embed: Graph,
    language: Language,
    decoder: Graph,
}

/// The language model and the names of its cache: `past_key_values.N.key` in, `present.N.key`
/// out, and the same for `value`, for each layer.
struct Language {
    graph: Graph,
    past: Vec<String>,
    present: Vec<String>,
    /// The shape of an empty cache entry: `[1, heads, 0, head size]`.
    empty: [usize; 4],
}

impl Language {
    fn new(graph: Graph) -> Result<Self, String> {
        let past: Vec<String> = graph
            .inputs()
            .iter()
            .map(|input| input.name().to_owned())
            .filter(|name| name.starts_with("past_key_values."))
            .collect();
        let first = graph
            .inputs()
            .iter()
            .find(|input| input.name().starts_with("past_key_values."))
            .ok_or("the language model has no cache inputs")?;
        let shape = first
            .dtype()
            .tensor_shape()
            .ok_or("the language model cache is not a tensor")?;
        let dim = |index: usize| {
            shape
                .get(index)
                .and_then(|dim| usize::try_from(*dim).ok())
                .filter(|dim| *dim > 0)
                .ok_or_else(|| format!("the language model cache has the shape {shape:?}"))
        };
        let empty = [1, dim(1)?, 0, dim(3)?];
        let present: Vec<String> = past
            .iter()
            .map(|name| name.replacen("past_key_values.", "present.", 1))
            .collect();
        let outputs: Vec<&str> = graph.outputs().iter().map(|output| output.name()).collect();
        if let Some(missing) = present
            .iter()
            .find(|name| !outputs.contains(&name.as_str()))
        {
            return Err(format!("the language model has no output {missing}"));
        }
        Ok(Self {
            graph,
            past,
            present,
            empty,
        })
    }
}

fn condition(encoder: &mut Graph, clip: &[f32]) -> Result<Conditioning, String> {
    let audio = tensor([1, clip.len()], clip.to_vec())?;
    let outputs = encoder
        .run(ort::inputs!["audio_values" => audio])
        .map_err(|error| format!("the speech encoder failed: {error}"))?;
    Ok(Conditioning {
        features: extract(&outputs, "audio_features")?,
        prompt: extract(&outputs, "audio_tokens")?,
        speaker: extract(&outputs, "speaker_embeddings")?,
        speaker_features: extract(&outputs, "speaker_features")?,
    })
}

impl Graphs {
    fn embed(&mut self, ids: &[i64]) -> Result<Tensor<f32>, String> {
        let input = tensor([1, ids.len()], ids.to_vec())?;
        let outputs = self
            .embed
            .run(ort::inputs!["input_ids" => input])
            .map_err(|error| format!("the token embedding failed: {error}"))?;
        extract(&outputs, "inputs_embeds")
    }

    /// The speech tokens for the text `ids`, after the voice's conditioning. Stops at the stop
    /// token, after `limit` tokens, or when one token repeats too long.
    fn tokens(
        &mut self,
        ids: &[u32],
        voice: &Conditioning,
        limit: usize,
        cancel: &CancelFlag,
    ) -> Result<Vec<i64>, JobError> {
        let ids: Vec<i64> = ids.iter().map(|id| i64::from(*id)).collect();
        let text = self.embed(&ids).map_err(failed)?;
        let hidden = voice.features.shape.last().copied().unwrap_or(0);
        if text.shape.last() != Some(&hidden) {
            return Err(failed(format!(
                "the voice has {hidden} values a frame, the model {:?}",
                text.shape.last()
            )));
        }
        let hidden = usize::try_from(hidden).map_err(|_| failed("a bad shape".into()))?;
        let mut embeds = voice.features.data.clone();
        embeds.extend(text.data);
        let mut cache: Vec<DynValue> = Vec::with_capacity(self.language.past.len());
        for _ in &self.language.past {
            cache.push(
                tensor(self.language.empty, Vec::<f32>::new())
                    .map_err(failed)?
                    .into_dyn(),
            );
        }
        let mut written = vec![START_SPEECH];
        let mut seen = Vec::new();
        let mut done = 0_usize;
        let mut repeats = 0_usize;
        while written.len() <= limit {
            if cancel.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            let fresh = embeds.len() / hidden;
            let total = done + fresh;
            let mut inputs: Vec<(Cow<'_, str>, SessionInputValue<'_>)> = vec![
                (
                    "inputs_embeds".into(),
                    tensor([1, fresh, hidden], std::mem::take(&mut embeds))
                        .map_err(failed)?
                        .into(),
                ),
                (
                    "attention_mask".into(),
                    tensor([1, total], vec![1_i64; total])
                        .map_err(failed)?
                        .into(),
                ),
                (
                    "position_ids".into(),
                    tensor([1, fresh], (done..total).map(|at| at as i64).collect())
                        .map_err(failed)?
                        .into(),
                ),
            ];
            for (name, value) in self.language.past.iter().zip(cache.drain(..)) {
                inputs.push((name.as_str().into(), value.into()));
            }
            let mut outputs = self
                .language
                .graph
                .run(inputs)
                .map_err(|error| failed(format!("the language model failed: {error}")))?;
            let next = {
                let (_, logits) = outputs["logits"]
                    .try_extract_tensor::<f32>()
                    .map_err(|error| failed(error.to_string()))?;
                pick(logits, &written, &mut seen)
            };
            for name in &self.language.present {
                let value = outputs
                    .remove(name.as_str())
                    .ok_or_else(|| failed(format!("no {name}")))?;
                cache.push(value);
            }
            drop(outputs);
            done = total;
            if next == STOP_SPEECH {
                break;
            }
            repeats = if written.last() == Some(&next) {
                repeats + 1
            } else {
                0
            };
            written.push(next);
            if repeats >= MAX_REPEATS {
                break;
            }
            embeds = self.embed(&[next]).map_err(failed)?.data;
        }
        Ok(written
            .into_iter()
            .skip(1)
            .filter(|token| *token < START_SPEECH)
            .collect())
    }

    fn decode(&mut self, tokens: &[i64], voice: &Conditioning) -> Result<Vec<f32>, String> {
        let mut speech = voice.prompt.data.clone();
        speech.extend_from_slice(tokens);
        speech.extend([SILENCE; 3]);
        let inputs = ort::inputs![
            "speech_tokens" => tensor([1, speech.len()], speech)?,
            "speaker_embeddings" => tensor_of(&voice.speaker)?,
            "speaker_features" => tensor_of(&voice.speaker_features)?,
        ];
        let outputs = self
            .decoder
            .run(inputs)
            .map_err(|error| format!("the decoder failed: {error}"))?;
        Ok(extract::<f32>(&outputs, "waveform")?.data)
    }
}

/// The next token: the highest logit of the last position, after the repetition penalty on
/// every token in `written`. `seen` marks those tokens and grows with them.
fn pick(logits: &[f32], written: &[i64], seen: &mut Vec<bool>) -> i64 {
    let vocab = usize::try_from(STOP_SPEECH + 1).unwrap_or(0);
    let last = &logits[logits.len().saturating_sub(vocab)..];
    seen.resize(last.len(), false);
    for token in written {
        if let Some(mark) = usize::try_from(*token).ok().and_then(|at| seen.get_mut(at)) {
            *mark = true;
        }
    }
    let mut best = (STOP_SPEECH, f32::NEG_INFINITY);
    for (token, (&logit, &was)) in last.iter().zip(seen.iter()).enumerate() {
        let score = match (was, logit < 0.0) {
            (false, _) => logit,
            (true, true) => logit * REPETITION_PENALTY,
            (true, false) => logit / REPETITION_PENALTY,
        };
        if score > best.1 {
            best = (token as i64, score);
        }
    }
    best.0
}

fn tensor<T, const N: usize>(shape: [usize; N], data: Vec<T>) -> Result<OrtTensor<T>, String>
where
    T: PrimitiveTensorElementType + Clone + std::fmt::Debug + 'static,
{
    OrtTensor::from_array((shape, data)).map_err(|error| error.to_string())
}

fn tensor_of<T>(value: &Tensor<T>) -> Result<OrtTensor<T>, String>
where
    T: PrimitiveTensorElementType + Clone + std::fmt::Debug + 'static,
{
    OrtTensor::from_array((value.shape.clone(), value.data.clone()))
        .map_err(|error| error.to_string())
}

fn extract<T>(outputs: &SessionOutputs<'_>, name: &str) -> Result<Tensor<T>, String>
where
    T: PrimitiveTensorElementType + Clone,
{
    let value = outputs
        .get(name)
        .ok_or_else(|| format!("no output {name}"))?;
    let (shape, data) = value
        .try_extract_tensor::<T>()
        .map_err(|error| format!("{name}: {error}"))?;
    Ok(Tensor {
        shape: shape.to_vec(),
        data: data.to_vec(),
    })
}

/// Whether `dir` holds every file of a Chatterbox model.
pub fn is_model_dir(dir: &Path) -> bool {
    FILES.iter().all(|file| dir.join(file).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::TtsEngine;

    #[test]
    fn the_penalty_turns_the_model_from_a_token_it_wrote() {
        let mut logits = vec![0.0_f32; 6563];
        logits[10] = 3.0;
        logits[11] = 2.6;
        let mut seen = Vec::new();
        assert_eq!(pick(&logits, &[START_SPEECH], &mut seen), 10);
        // 3.0 / 1.2 = 2.5 is below 2.6.
        assert_eq!(pick(&logits, &[START_SPEECH, 10], &mut seen), 11);
        logits.fill(-2.0);
        logits[12] = -0.9;
        logits[13] = -1.05;
        // -0.9 * 1.2 = -1.08 is below -1.05.
        assert_eq!(pick(&logits, &[12], &mut Vec::new()), 13);
    }

    #[test]
    fn the_last_position_of_the_logits_counts() {
        let mut logits = vec![0.0_f32; 2 * 6563];
        logits[5] = 9.0;
        logits[6563 + 7] = 1.0;
        assert_eq!(pick(&logits, &[], &mut Vec::new()), 7);
    }

    #[test]
    fn a_folder_without_the_model_files_fails_before_the_runtime_opens() {
        let backend = ChatterboxBackend::new(PathBuf::from("/nonexistent/lib"));
        let spec = LoadSpec {
            dir: std::env::temp_dir().join("hushpen-tts-no-chatterbox"),
            engine: TtsEngine::Chatterbox,
            threads: 2,
            idle: None,
        };
        let Err(problem) = backend.load(&spec) else {
            panic!("no model, no load");
        };
        assert!(
            problem.detail.contains(LANGUAGE_MODEL_FILE),
            "{}",
            problem.detail
        );
        assert!(!is_model_dir(&spec.dir));
    }
}
