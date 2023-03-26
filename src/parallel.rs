use std::{collections::VecDeque, io::Write, num::NonZeroUsize, path::PathBuf, sync::Arc, thread};

use anyhow::{Context as _, Result};
use crossbeam_channel::{bounded, select, Sender};
use log::trace;

use crate::{exec_one_file, Args};
#[derive(Debug)]
enum Request {
    Input(usize, PathBuf),
}

#[derive(Debug)]
enum Response {
    Diff(usize, PathBuf, usize, Vec<u8>),
    Error(usize, PathBuf, usize, anyhow::Error),
}

pub(crate) fn parallel_exec_multiple_files_unordered<W: Write, I: Iterator<Item = PathBuf>>(
    args: &Args,
    mut w: W,
    cmd_args: &[String],
    files: I,
    threads: NonZeroUsize,
) -> Result<()> {
    trace!("the number of threads: {}", threads.get());
    thread::scope(|s| {
        let args = Arc::new(args.clone());
        let cmd_args = Arc::new(cmd_args.to_owned());
        // parent to children
        let p2c_capacity = threads.get() * 2;
        let (p2c_tx, p2c_rx) = bounded::<Request>(p2c_capacity);
        // children to parent
        let c2p_capacity = threads.get() * 2;
        let (c2p_tx, c2p_rx) = bounded::<Response>(c2p_capacity);
        // generate workers
        for tid in 0..threads.get() {
            let args = args.clone();
            let cmd_args = cmd_args.clone();
            let p2c_rx = p2c_rx.clone();
            let c2p_tx = c2p_tx.clone();
            s.spawn(move || -> Result<()> {
                loop {
                    let Ok(req) = p2c_rx.recv() else {
                        // disconnected
                        break;
                    };
                    match req {
                        Request::Input(i, file) => {
                            let mut buf = Vec::new();
                            let resp =
                                match exec_one_file(args.as_ref(), &mut buf, &cmd_args, &file) {
                                    Ok(_) => Response::Diff(i, file, tid, buf),
                                    Err(e) => Response::Error(i, file, tid, e),
                                };
                            c2p_tx.send(resp)?;
                        }
                    }
                }
                Ok(())
            });
        }
        // define response handler
        let mut handle_resp = |resp: Response| -> Result<()> {
            match resp {
                Response::Diff(i, file, tid, buf) => {
                    trace!("handle_resp: {:?} {:?} {:?}", i, file, tid);
                    w.write_all(&buf)
                        .with_context(|| format!("{}", file.to_string_lossy()))?;
                }
                Response::Error(_, file, _, e) => {
                    return Err(e).with_context(|| format!("{}", file.to_string_lossy()))
                }
            };
            Ok(())
        };
        // processing files
        trace!("processing...");
        let mut count = 0usize;
        for (i, file) in files.enumerate() {
            let req = Request::Input(i, file.to_owned());
            trace!("send: {} {:?}", i, req);
            loop {
                // pipelining
                select! {
                    send(p2c_tx, req) -> unit => {
                        unit?;
                        trace!("sent: {}", i);
                        count += 1;
                        break;
                    }
                    recv(c2p_rx) -> resp => {
                        handle_resp(resp?)?;
                        count -= 1;
                    }
                }
            }
        }
        trace!("remains: {}", count);
        while count > 0 {
            let resp = c2p_rx.recv()?;
            handle_resp(resp)?;
            count -= 1;
        }
        Ok(())
    })
}

pub(crate) fn parallel_exec_multiple_files_ordered<W: Write, I: Iterator<Item = PathBuf>>(
    args: &Args,
    mut w: W,
    cmd_args: &[String],
    files: I,
    threads: NonZeroUsize,
) -> Result<()> {
    trace!("the number of threads: {}", threads.get());
    thread::scope(|s| {
        let args = Arc::new(args.clone());
        let cmd_args = Arc::new(cmd_args.to_owned());
        // parent to children
        let p2c_capacity = threads.get() * 2;
        let (p2c_tx, p2c_rx) = bounded::<(Request, Sender<Response>)>(p2c_capacity);
        // generate workers
        for tid in 0..threads.get() {
            let args = args.clone();
            let cmd_args = cmd_args.clone();
            let p2c_rx = p2c_rx.clone();
            s.spawn(move || -> Result<()> {
                loop {
                    let Ok((req, c2p_tx)) = p2c_rx.recv() else {
                        // disconnected
                        break;
                    };
                    match req {
                        Request::Input(i, file) => {
                            let mut buf = Vec::new();
                            let resp =
                                match exec_one_file(args.as_ref(), &mut buf, &cmd_args, &file) {
                                    Ok(_) => Response::Diff(i, file, tid, buf),
                                    Err(e) => Response::Error(i, file, tid, e),
                                };
                            c2p_tx.send(resp)?;
                        }
                    }
                }
                Ok(())
            });
        }
        // define response handler
        let mut handle_resp = |resp: Response| -> Result<()> {
            match resp {
                Response::Diff(i, file, tid, buf) => {
                    trace!("handle_resp: {:?} {:?} {:?}", i, file, tid);
                    w.write_all(&buf)
                        .with_context(|| format!("{}", file.to_string_lossy()))?;
                }
                Response::Error(_, file, _, e) => {
                    return Err(e).with_context(|| format!("{}", file.to_string_lossy()))
                }
            };
            Ok(())
        };
        // processing files
        trace!("processing...");
        let mut c2p_rxs = VecDeque::new();
        let c2p_rxs_capacity = threads.get() * 8;
        for (i, file) in files.enumerate() {
            let req = Request::Input(i, file.to_owned());
            // child to parent
            let (c2p_tx, c2p_rx) = bounded::<Response>(1);
            trace!("send: {} {:?}", i, req);
            c2p_rxs.push_back(c2p_rx);
            loop {
                // pipelining
                if c2p_rxs.len() > c2p_rxs_capacity {
                    let resp = c2p_rxs[0].recv()?;
                    handle_resp(resp)?;
                    c2p_rxs.pop_front();
                } else if !c2p_rxs.is_empty() {
                    select! {
                        send(p2c_tx, (req, c2p_tx)) -> unit => {
                            unit?;
                            trace!("sent: {}", i);
                            break;
                        }
                        recv(c2p_rxs[0]) -> resp => {
                            handle_resp(resp?)?;
                            c2p_rxs.pop_front();
                        }
                    }
                } else {
                    p2c_tx.send((req, c2p_tx))?;
                    trace!("sent: {}", i);
                    break;
                }
            }
        }
        trace!("remains: {}", c2p_rxs.len());
        loop {
            let Some(c2p_rx) = c2p_rxs.pop_front() else { break; };
            let resp = c2p_rx.recv()?;
            handle_resp(resp)?;
        }
        Ok(())
    })
}
