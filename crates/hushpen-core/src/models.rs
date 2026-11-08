//! Model download, verification, and default-choice rules. File work stays with the caller.

use crate::catalog::ModelKind;
use crate::error::{DOWNLOAD_FAILED, MODEL_HASH_MISMATCH, MODEL_HOST_BLOCKED};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelState {
    NotDownloaded,
    Downloading { done: u64, total: u64 },
    Verifying,
    Ready,
    Failed,
}

impl ModelState {
    pub fn key(self) -> &'static str {
        match self {
            Self::NotDownloaded => "not_downloaded",
            Self::Downloading { .. } => "downloading",
            Self::Verifying => "verifying",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    pub fn percent(self) -> Option<u64> {
        match self {
            Self::Downloading { done, total } if total > 0 => Some(done * 100 / total),
            _ => None,
        }
    }

    pub fn download(self, id: &str) -> Result<(), String> {
        match self {
            Self::NotDownloaded | Self::Failed => Ok(()),
            Self::Downloading { .. } | Self::Verifying => {
                Err(format!("{id} is already being downloaded or checked"))
            }
            Self::Ready => Err(format!("{id} is already downloaded")),
        }
    }

    pub fn delete(self, id: &str) -> Result<(), String> {
        if matches!(self, Self::Downloading { .. } | Self::Verifying) {
            Err(format!("{id} is busy; cancel the download first"))
        } else {
            Ok(())
        }
    }

    pub fn select(self, id: &str, kind: ModelKind) -> Result<(), String> {
        if kind != ModelKind::Speech {
            return Err(format!("{id} is not a speech model"));
        }
        match self {
            Self::Ready => Ok(()),
            Self::Failed => Err(format!("{MODEL_HASH_MISMATCH}: {id} failed verification")),
            _ => Err(format!("{id} is not downloaded yet")),
        }
    }

    pub fn selection_notice(self, kind: ModelKind, name: &str) -> Option<Notice> {
        (kind == ModelKind::Speech && self == Self::Failed).then(|| Notice {
            code: MODEL_HASH_MISMATCH,
            message: format!("{name} failed verification and cannot be used. Download it again."),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    Done,
    Cancelled,
    Failed(&'static str),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Finished {
    pub state: Option<ModelState>,
    pub remove_files: bool,
    pub adopt: bool,
    pub load: bool,
    pub clear_notice: bool,
    pub notice: Option<Notice>,
    pub restart: bool,
}

struct Job {
    number: u64,
    cancelled: bool,
}

#[derive(Default)]
pub struct Downloads {
    jobs: HashMap<String, Job>,
    restart: HashSet<String>,
    stopped_updates: HashSet<String>,
    next_job: u64,
}

impl Downloads {
    pub fn has_job(&self, id: &str) -> bool {
        self.jobs.contains_key(id)
    }

    pub fn progress(&self, id: &str, number: u64) -> bool {
        self.jobs
            .get(id)
            .is_some_and(|job| job.number == number && !job.cancelled)
    }

    /// Wait for a cancelled worker before starting another download of the same files.
    pub fn queue_restart(&mut self, id: &str, total: u64) -> Option<ModelState> {
        if self.jobs.get(id).is_some_and(|job| job.cancelled) {
            self.restart.insert(id.to_owned());
            Some(ModelState::Downloading { done: 0, total })
        } else {
            None
        }
    }

    pub fn next_job(&mut self) -> u64 {
        self.next_job += 1;
        self.next_job
    }

    pub fn started(&mut self, id: &str, number: u64, total: u64) -> ModelState {
        self.jobs.insert(
            id.to_owned(),
            Job {
                number,
                cancelled: false,
            },
        );
        ModelState::Downloading { done: 0, total }
    }

    pub fn cancel(&mut self, id: &str) -> Result<ModelState, String> {
        self.restart.remove(id);
        let Some(job) = self.jobs.get_mut(id) else {
            return Err(format!("{id} is not being downloaded"));
        };
        job.cancelled = true;
        Ok(ModelState::NotDownloaded)
    }

    pub fn retry_updates(&mut self) -> HashSet<String> {
        std::mem::take(&mut self.stopped_updates)
    }

    pub fn can_retry(&self, id: &str, state: ModelState, old_files: bool) -> bool {
        !self.has_job(id) && state == ModelState::NotDownloaded && old_files
    }

    pub fn verified(&self, id: &str, state: ModelState, ready: bool) -> Option<ModelState> {
        (state == ModelState::Verifying && !self.has_job(id)).then_some(if ready {
            ModelState::Ready
        } else {
            ModelState::Failed
        })
    }

    pub fn finished(
        &mut self,
        id: &str,
        number: u64,
        result: Completion,
        updating: bool,
        name: &str,
    ) -> Option<Finished> {
        if self.jobs.get(id).is_none_or(|job| job.number != number) {
            return None;
        }
        let cancelled = self.jobs.remove(id).is_some_and(|job| job.cancelled);
        let mut finished = Finished {
            state: None,
            remove_files: cancelled && result == Completion::Done,
            adopt: false,
            load: false,
            clear_notice: false,
            notice: None,
            restart: self.restart.remove(id),
        };
        if cancelled {
            return Some(finished);
        }
        match result {
            Completion::Done => {
                finished.state = Some(ModelState::Ready);
                finished.adopt = !updating;
                finished.load = true;
                finished.clear_notice = true;
            }
            Completion::Cancelled => finished.state = Some(ModelState::NotDownloaded),
            Completion::Failed(code) => {
                finished.state = Some(ModelState::NotDownloaded);
                if updating && !matches!(code, MODEL_HASH_MISMATCH | MODEL_HOST_BLOCKED) {
                    self.stopped_updates.insert(id.to_owned());
                } else {
                    finished.notice = Some(notice_for_failure(code, name));
                }
            }
        }
        Some(finished)
    }
}

pub fn effective<'a>(chosen: &'a str, default: Option<&'a str>) -> &'a str {
    if chosen.trim().is_empty() {
        default.unwrap_or_default()
    } else {
        chosen
    }
}

pub fn should_adopt<'a>(
    kind: ModelKind,
    id: &str,
    chosen: &str,
    default: &str,
    mut others: impl Iterator<Item = (&'a str, ModelState, bool)>,
) -> bool {
    kind == ModelKind::Speech
        && chosen != id
        && !others.any(|(other, state, updating)| {
            other == default && other != id && (state == ModelState::Ready || updating)
        })
}

pub fn replacement<'a>(
    deleted: &str,
    mut rows: impl Iterator<Item = (&'a str, ModelState)>,
) -> Option<&'a str> {
    rows.find(|(id, state)| *id != deleted && *state == ModelState::Ready)
        .map(|(id, _)| id)
}

pub fn deletes_default(kind: ModelKind, deleted: &str, effective: &str) -> bool {
    kind == ModelKind::Speech && effective == deleted
}

pub fn scan_state(
    old_files: bool,
    wrong_size: bool,
    missing: bool,
    needs_hash: bool,
) -> ModelState {
    if old_files {
        ModelState::NotDownloaded
    } else if wrong_size {
        ModelState::Failed
    } else if missing {
        ModelState::NotDownloaded
    } else if needs_hash {
        ModelState::Verifying
    } else {
        ModelState::Ready
    }
}

fn notice_for_failure(code: &'static str, name: &str) -> Notice {
    let message = match code {
        MODEL_HASH_MISMATCH => {
            format!("The download of {name} did not match its checksum. It was removed.")
        }
        MODEL_HOST_BLOCKED => {
            "The download was refused because it did not come from an approved host.".to_owned()
        }
        _ => format!("The download of {name} stopped. Try again to continue it."),
    };
    Notice {
        code: match code {
            MODEL_HASH_MISMATCH => MODEL_HASH_MISMATCH,
            MODEL_HOST_BLOCKED => MODEL_HOST_BLOCKED,
            _ => DOWNLOAD_FAILED,
        },
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_wins_over_late_success_and_a_retry_waits_for_the_worker() {
        let mut downloads = Downloads::default();
        let job = downloads.next_job();
        downloads.started("speech", job, 100);
        downloads.cancel("speech").unwrap();
        assert!(!downloads.progress("speech", job));
        assert_eq!(
            downloads.queue_restart("speech", 100),
            Some(ModelState::Downloading {
                done: 0,
                total: 100
            })
        );
        assert!(
            downloads
                .finished("speech", job + 1, Completion::Done, false, "Speech")
                .is_none()
        );
        let done = downloads
            .finished("speech", job, Completion::Done, false, "Speech")
            .unwrap();
        assert!(done.remove_files && done.restart);
        assert!(!done.adopt && !done.load);
        assert_eq!(done.state, None);
    }

    #[test]
    fn network_stopped_updates_retry_quietly_but_bad_files_report_failure() {
        let mut downloads = Downloads::default();
        downloads.started("voice", 1, 100);
        let done = downloads
            .finished(
                "voice",
                1,
                Completion::Failed(DOWNLOAD_FAILED),
                true,
                "Voice",
            )
            .unwrap();
        assert_eq!(done.state, Some(ModelState::NotDownloaded));
        assert_eq!(done.notice, None);
        assert!(downloads.retry_updates().contains("voice"));
        assert!(downloads.can_retry("voice", ModelState::NotDownloaded, true));
        downloads.started("voice", 2, 100);
        let done = downloads
            .finished(
                "voice",
                2,
                Completion::Failed(MODEL_HASH_MISMATCH),
                true,
                "Voice",
            )
            .unwrap();
        assert_eq!(done.notice.unwrap().code, MODEL_HASH_MISMATCH);
        assert!(downloads.retry_updates().is_empty());
    }

    #[test]
    fn verification_ignores_live_downloads_and_preserves_failed_state() {
        let mut downloads = Downloads::default();
        assert_eq!(
            downloads.verified("speech", ModelState::Verifying, false),
            Some(ModelState::Failed)
        );
        downloads.started("speech", 1, 100);
        assert_eq!(
            downloads.verified("speech", ModelState::Verifying, true),
            None
        );
        assert_eq!(downloads.verified("other", ModelState::Ready, false), None);
    }

    #[test]
    fn default_changes_only_when_the_previous_default_is_unusable() {
        let others = || [("first", ModelState::Ready, false)].into_iter();
        assert!(!should_adopt(
            ModelKind::Speech,
            "next",
            "first",
            "first",
            others()
        ));
        assert!(should_adopt(
            ModelKind::Speech,
            "next",
            "missing",
            "missing",
            others()
        ));
        assert!(!should_adopt(
            ModelKind::Voice,
            "next",
            "missing",
            "missing",
            others()
        ));
        assert_eq!(
            replacement(
                "first",
                [("first", ModelState::Ready), ("next", ModelState::Ready)].into_iter()
            ),
            Some("next")
        );
        assert_eq!(
            replacement("first", [("first", ModelState::Ready)].into_iter()),
            None
        );
        assert_eq!(effective("", Some("first")), "first");
        assert_eq!(effective("next", Some("first")), "next");
    }

    #[test]
    fn startup_check_and_action_rules_keep_damaged_models_out() {
        assert_eq!(scan_state(false, true, true, true), ModelState::Failed);
        assert_eq!(
            scan_state(false, false, true, true),
            ModelState::NotDownloaded
        );
        assert_eq!(scan_state(false, false, false, true), ModelState::Verifying);
        assert_eq!(scan_state(false, false, false, false), ModelState::Ready);
        assert_eq!(
            scan_state(true, false, false, false),
            ModelState::NotDownloaded
        );
        assert!(ModelState::Failed.download("speech").is_ok());
        assert!(ModelState::Ready.download("speech").is_err());
        assert!(ModelState::Verifying.delete("speech").is_err());
        assert!(
            ModelState::Failed
                .select("speech", ModelKind::Speech)
                .is_err()
        );
        assert!(ModelState::Ready.select("voice", ModelKind::Voice).is_err());
    }

    #[test]
    fn selection_notices_and_deletion_apply_only_to_speech_defaults() {
        assert_eq!(
            ModelState::Failed
                .selection_notice(ModelKind::Speech, "Speech")
                .unwrap()
                .code,
            MODEL_HASH_MISMATCH
        );
        assert_eq!(
            ModelState::Failed.selection_notice(ModelKind::Voice, "Voice"),
            None
        );
        assert_eq!(
            ModelState::NotDownloaded.selection_notice(ModelKind::Speech, "Speech"),
            None
        );
        assert!(deletes_default(ModelKind::Speech, "speech", "speech"));
        assert!(!deletes_default(ModelKind::Speech, "other", "speech"));
        assert!(!deletes_default(ModelKind::Voice, "voice", "voice"));
    }
}
