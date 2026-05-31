use crate::db::DbRepository;
use crate::metadata::Metadata;
use eframe::egui::Context;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Instant;

pub const LAST_MSG_LINGER_MS: u128 = 2000;

#[derive(Debug)]
pub enum Job {
    CacheMetadataForImages(Vec<PathBuf>),
    ClearMovedFiles(Vec<PathBuf>),
    SetXMPRating(Vec<PathBuf>, i32),
}

pub enum WorkerMessage {
    Log(String),
    Done,
}

pub enum PostProcessingMessage {
    RefreshMetadata,
    RefreshRating(Vec<PathBuf>, i32),
}

pub struct Worker {
    job_tx: Sender<Job>,
    msg_rx: Receiver<WorkerMessage>,
    post_processing_rx: Receiver<PostProcessingMessage>,
    msgs: Vec<String>,
    last_msg: Option<String>,
    lingering_msg: Option<(Instant, String)>,
}

impl Worker {
    pub fn new(ctx: Context, db_repo: &DbRepository) -> Self {
        let (job_tx, job_rx) = channel();
        let (msg_tx, msg_rx) = channel();
        let (pp_tx, pp_rx) = channel();

        let worker_ctx = ctx.clone();
        let db_repo = db_repo.clone();
        thread::spawn(move || {
            worker_loop(worker_ctx, job_rx, msg_tx, pp_tx, db_repo);
        });

        Self {
            job_tx,
            msg_rx,
            post_processing_rx: pp_rx,
            msgs: vec![],
            last_msg: None,
            lingering_msg: None,
        }
    }

    pub fn send_job(&self, job: Job) {
        self.job_tx.send(job).expect("Failed to send job to worker");
    }

    pub fn get_latest_msg(&mut self) -> Option<String> {
        self.msg_rx.try_iter().for_each(|x| match x {
            WorkerMessage::Done => {
                if let Some(last_msg) = &self.last_msg {
                    self.lingering_msg = Some((Instant::now(), last_msg.clone()));
                }
                self.last_msg = None
            }
            WorkerMessage::Log(msg) => {
                self.msgs.push(msg.clone());
                self.last_msg = Some(msg);
                self.lingering_msg = None;
            }
        });

        if self.last_msg.is_some() {
            return self.last_msg.clone();
        }

        if let Some((msg_start, _msg)) = &self.lingering_msg
            && msg_start.elapsed().as_millis() > LAST_MSG_LINGER_MS
        {
            self.lingering_msg = None;
        }

        self.lingering_msg.as_ref().map(|(_, msg)| msg.clone())
    }

    pub fn get_all_post_processing_messages(&mut self) -> Vec<PostProcessingMessage> {
        self.post_processing_rx.try_iter().collect()
    }
}

fn worker_loop(
    ctx: Context,
    job_rx: Receiver<Job>,
    msg_tx: Sender<WorkerMessage>,
    pp_tx: Sender<PostProcessingMessage>,
    db_repo: DbRepository,
) {
    let mut db_repo = db_repo.clone();
    while let Ok(job) = job_rx.recv() {
        match job {
            Job::CacheMetadataForImages(paths) => {
                cache_metadata_for_images(&msg_tx, &pp_tx, &mut db_repo, paths);
            }
            Job::SetXMPRating(paths, rating) => {
                set_xmp_rating_for_images(&msg_tx, &pp_tx, &mut db_repo, paths, rating);
            }
            Job::ClearMovedFiles(paths) => {
                clear_moved_files(&msg_tx, &mut db_repo, paths);
            }
        }

        //Repaint so the latest message, which is usually Done, is received and hides the message popup.
        ctx.request_repaint();
    }
}

fn clear_moved_files(
    msg_tx: &Sender<WorkerMessage>,
    db_repo: &mut DbRepository,
    paths: Vec<PathBuf>,
) {
    worker_send_msg(
        msg_tx,
        WorkerMessage::Log("Clearing moved files from the database".to_string()),
    );
    let _ = Metadata::clear_moved_files(db_repo, &paths);
    worker_send_msg(
        msg_tx,
        WorkerMessage::Log("Finished cleaning the database".to_string()),
    );
    worker_send_msg(msg_tx, WorkerMessage::Done);
}

fn cache_metadata_for_images(
    msg_tx: &Sender<WorkerMessage>,
    pp_tx: &Sender<PostProcessingMessage>,
    db_repo: &mut DbRepository,
    paths: Vec<PathBuf>,
) {
    Metadata::cache_metadata_for_images(db_repo, &paths, |progress_msg| {
        worker_send_msg(msg_tx, WorkerMessage::Log(progress_msg));
    });

    Metadata::update_xmp_metadata_for_images(db_repo, &paths, |progress_msg| {
        worker_send_msg(msg_tx, WorkerMessage::Log(progress_msg));
    });

    worker_send_msg(
        msg_tx,
        WorkerMessage::Log("Finished caching exif and xmp metadata for all images".to_string()),
    );
    worker_send_msg(msg_tx, WorkerMessage::Done);
    let _ = pp_tx.send(PostProcessingMessage::RefreshMetadata);
}

fn set_xmp_rating_for_images(
    msg_tx: &Sender<WorkerMessage>,
    pp_tx: &Sender<PostProcessingMessage>,
    db_repo: &mut DbRepository,
    paths: Vec<PathBuf>,
    rating: i32,
) {
    worker_send_msg(
        msg_tx,
        WorkerMessage::Log(format!(
            "Applying rating {} to {} images",
            rating,
            paths.len()
        )),
    );

    if Metadata::exiftool_rate_images_xmp(&paths, rating).is_err() {
        worker_send_msg(
            msg_tx,
            WorkerMessage::Log("Failure applying xmp rating to selected images".to_string()),
        );
        worker_send_msg(msg_tx, WorkerMessage::Done);
        return;
    }

    match db_repo.update_files_xmp_rating(&paths, rating) {
        Ok(_) => {
            worker_send_msg(
                msg_tx,
                WorkerMessage::Log(format!(
                    "Applied {} rating to {} images",
                    rating,
                    paths.len()
                )),
            );
        }
        Err(e) => {
            tracing::error!(e);
            worker_send_msg(
                msg_tx,
                WorkerMessage::Log(
                    "Partial failure applying xmp rating to selected images".to_string(),
                ),
            );
        }
    }

    worker_send_msg(msg_tx, WorkerMessage::Done);
    let _ = pp_tx.send(PostProcessingMessage::RefreshRating(paths, rating));
}

fn worker_send_msg(msg_tx: &Sender<WorkerMessage>, msg: WorkerMessage) {
    match msg {
        WorkerMessage::Log(ref msg) => tracing::info!(msg),
        WorkerMessage::Done => {}
    };

    if msg_tx.send(msg).is_err() {
        tracing::error!("Failure sending worker message to channel");
    }
}
