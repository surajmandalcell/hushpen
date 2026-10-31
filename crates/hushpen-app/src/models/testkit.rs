//! The library under test: a data folder in a temp dir, worker jobs queued to run on the test
//! thread, and an opener that records.

use super::{Catalog, Machine, Models, Opened, Spawner, Transfer};
use crate::storage::{self, Storage};
use gpui_kit::{AppContext as _, Entity, TestAppContext};
use hushpen_store::data_dir::DataDir;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub(super) type Work = Rc<RefCell<VecDeque<Box<dyn FnOnce() + Send>>>>;

pub(super) struct Rig {
    pub models: Entity<Models>,
    pub storage: Rc<Storage>,
    pub work: Work,
    pub opened: Rc<Opened>,
    _tmp: tempfile::TempDir,
}

impl Rig {
    /// A speech model's file, named the way the test catalogs name it.
    pub fn file(&self, id: &str) -> PathBuf {
        self.storage
            .data
            .whisper_models_dir()
            .join(format!("ggml-{id}.bin"))
    }

    /// The names in the partial downloads folder.
    pub fn downloads(&self) -> Vec<String> {
        std::fs::read_dir(self.storage.data.downloads_dir())
            .map(|dir| {
                dir.flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// A catalog that lives as long as the test binary, as the embedded one does.
pub(super) fn leak(catalog: Value) -> &'static Catalog {
    Box::leak(Box::new(Catalog::parse(&catalog.to_string()).unwrap()))
}

/// A queue of worker jobs and the spawner that fills it.
pub(super) fn queue() -> (Work, Spawner) {
    let work: Work = Rc::default();
    let queue = Rc::clone(&work);
    let spawn: Spawner = Rc::new(move |job| {
        queue.borrow_mut().push_back(job);
        Ok(())
    });
    (work, spawn)
}

/// A library on a Linux computer with 8 GB. `prepare` writes files or settings before it opens.
pub(super) fn rig(
    cx: &mut TestAppContext,
    catalog: &'static Catalog,
    transfer: Arc<Transfer>,
    prepare: impl FnOnce(&Storage),
) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    prepare(&storage);
    let (work, spawn) = queue();
    let opened = Rc::new(Opened::default());
    let machine = Machine {
        opener: opened.clone(),
    };
    let models = cx.new(|cx| {
        Models::new(
            Rc::clone(&storage),
            catalog,
            machine,
            transfer,
            spawn,
            None,
            cx,
        )
    });
    Rig {
        models,
        storage,
        work,
        opened,
        _tmp: tmp,
    }
}

/// Runs the queued worker jobs on this thread and applies the events they send, until nothing
/// is left to do.
pub(super) fn settle(cx: &mut TestAppContext, rig: &Rig) {
    loop {
        cx.run_until_parked();
        let Some(job) = rig.work.borrow_mut().pop_front() else {
            return;
        };
        job();
    }
}
