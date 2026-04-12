use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use eframe::egui::Context;
use crate::db::DbRepository;
use crate::metadata::Metadata;

#[derive(Debug)]
pub enum Job {
    CacheMetadataForImages(Vec<PathBuf>),
    ClearMovedFiles(Vec<PathBuf>),
}

pub enum WorkerMessage {
    Log(String),
    Done,
}

pub struct Worker {
    job_tx: Sender<Job>,
    msg_rx: Receiver<WorkerMessage>,
    msgs: Vec<String>,
    last_msg: Option<String>
}

impl Worker {
    pub fn new(ctx: Context, db_repo: &DbRepository) -> Self {
        let (job_tx, job_rx) = channel();
        let (msg_tx, msg_rx) = channel();

        let worker_ctx = ctx.clone();
        let db_repo = db_repo.clone();
        thread::spawn(move || {
            worker_loop(worker_ctx, job_rx, msg_tx, db_repo);
        });

        Self { job_tx, msg_rx, msgs: vec![], last_msg: None }
    }

    pub fn send_job(&self, job: Job) {
        self.job_tx.send(job).expect("Failed to send job to worker");
    }

    pub fn get_latest_msg(&mut self) -> &Option<String> {
        self.msg_rx.try_iter().for_each(|x| {
            match x {
                WorkerMessage::Done => self.last_msg = None,
                WorkerMessage::Log(msg) => {self.msgs.push(msg.clone()); self.last_msg = Some(msg) }
            }
        });

        &self.last_msg
    }
}

fn worker_loop(ctx: Context, 
        job_rx: Receiver<Job>, 
        msg_tx: Sender<WorkerMessage>, 
        db_repo: DbRepository
    ) {
    let mut db_repo = db_repo.clone();
    while let Ok(job) = job_rx.recv() {
        match job {
            Job::CacheMetadataForImages(paths) => {
                let log = format!("Caching metadata for {} images", paths.len());
                tracing::info!("{}", log);
                worker_send_msg(&msg_tx, WorkerMessage::Log(log.to_string()));

                Metadata::cache_metadata_for_images(&mut db_repo, &paths);

                worker_send_msg(&msg_tx, WorkerMessage::Done);
                tracing::info!("Finished caching metadata for all images");
            }
            Job::ClearMovedFiles(paths) => {
                worker_send_msg(&msg_tx, WorkerMessage::Log("Clearing moved files from the database".to_string()));
                let _ = Metadata::clear_moved_files(&mut db_repo, &paths);
                worker_send_msg(&msg_tx, WorkerMessage::Done);
            }
        }

        //Repaint so the latest message, which is usually Done, is received and hides the message popup.
        ctx.request_repaint();
    }
}

fn worker_send_msg(msg_tx: &Sender<WorkerMessage>, msg: WorkerMessage) {
    if msg_tx.send(msg).is_err() {
        tracing::error!("Failure sending worker message to channel");
    }
}