use std::sync::{mpsc, Arc, Mutex};
use std::thread;

/// Message type to communicate with workers. A JobMsg is either a FnOnce closure or None, which
/// signals the worker to shut down.
type JobMsg = Option<Box<dyn FnOnce() + Send + 'static>>;

/// A ThreadPool should have a sending-end of a mpsc channel (`mpsc::Sender`) and a vector of
/// `JoinHandle`s for the worker threads.
pub struct ThreadPool {
    sender: mpsc::Sender<JobMsg>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl ThreadPool {
    /// Spin up a thread pool with `num_workers` threads. Workers should all share the same
    /// receiving end of an mpsc channel (`mpsc::Receiver`) with appropriate synchronization. Each
    /// thread should loop and (1) listen for new jobs on the channel, (2) execute received jobs,
    /// and (3) quit the loop if it receives None.
    pub fn new(num_workers: usize) -> Self {
        let mut handles = Vec::new();
        let (tx, rx) = mpsc::channel::<JobMsg>();
        let rx_wrapper = Arc::new(Mutex::new(rx));

        for _n in 0..num_workers {
            let rx_wrapper_clone = Arc::clone(&rx_wrapper);
            let handle = thread::spawn(move || loop {
                let rx_i = rx_wrapper_clone.lock().unwrap();
                let job = rx_i.recv().unwrap();
                match job {
                    Some(f) => {
                        std::mem::drop(rx_i);
                        f()
                    }
                    None => break,
                }
            });
            handles.push(handle);
        }

        ThreadPool {
            sender: tx,
            workers: handles,
        }
    }

    /// Push a new job into the thread pool.
    pub fn execute<F>(&mut self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.sender.send(Some(Box::new(job))).unwrap();
    }
}

impl Drop for ThreadPool {
    /// Clean up the thread pool. Send a kill message (None) to each worker, and join each worker.
    /// This function should only return when all workers have finished.
    fn drop(&mut self) {
        for _n in 0..self.workers.len() {
            self.sender.send(None).unwrap();
        }
        for _n in 0..self.workers.len() {
            if let Some(w) = self.workers.pop() {
                w.join().expect("The thread being joined has panicked")
            }
        }
    }
}
