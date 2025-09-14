use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use eframe::egui::{Context, Id};

use crate::metadata::Metadata;
use crate::WORKER_MESSAGE_MEMORY_KEY;

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
}

impl Worker {
    pub fn new(ctx: Context) -> Self {
        let (job_tx, job_rx) = channel();

        let worker_ctx = ctx.clone();
        thread::spawn(move || {
            worker_loop(worker_ctx, job_rx);
        });

        Self { job_tx }
    }

    pub fn send_job(&self, job: Job) {
        self.job_tx.send(job).expect("Failed to send job to worker");
    }
}

fn worker_loop(ctx: Context, job_rx: Receiver<Job>) {
    while let Ok(job) = job_rx.recv() {
        match job {
            Job::CacheMetadataForImages(paths) => {
                worker_set_msg(
                    &ctx,
                    &format!("Caching metadata for {} images", paths.len()),
                );
                Metadata::cache_metadata_for_images(&paths);
                worker_set_msg(
                    &ctx,
                    &format!("Finished caching metadata for {} images", paths.len()),
                );
            }
            Job::ClearMovedFiles(paths) => {
                worker_set_msg(&ctx, "Clearing moved files from the database");
                let cleared_files = Metadata::clear_moved_files(&paths);
                worker_set_msg(
                    &ctx,
                    &format!("Cleared {cleared_files} moved files from the database"),
                );
            }
        }
    }
}

fn worker_set_msg(ctx: &Context, msg: &str) {
    ctx.memory_mut(|mem| {
        let worker_msgs = mem
            .data
            .get_temp_mut_or_default::<Vec<Arc<String>>>(Id::new(WORKER_MESSAGE_MEMORY_KEY));

        worker_msgs.push(Arc::new(msg.to_string()));
    });
}
