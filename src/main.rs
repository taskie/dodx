use std::ffi::OsString;
use std::num::NonZeroUsize;
use std::os::unix::ffi::OsStringExt;
use std::thread::available_parallelism;
use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::Result;
use clap::Parser;
use itertools::process_results;
use log::trace;
use parallel::{parallel_exec_multiple_files_ordered, parallel_exec_multiple_files_unordered};
use similar::TextDiff;

mod parallel;

#[derive(Clone, Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Handle null-separated input items
    #[clap(short = '0', long)]
    null: bool,
    /// The approximate number of threads to use
    #[clap(short = 'j', long, default_value_t = 0)]
    threads: i32,
    /// Produce fast unordered output in multi-threaded execution
    #[clap(short = 'u', long)]
    unordered: bool,
    /// Interpret arguments after last '--' as file names
    #[clap(short = 'X', long)]
    multi_args: bool,
    /// Interpret the last argument as a file name
    #[clap(short = 'x', long)]
    single_arg: bool,
    /// File containing file names
    #[clap(short, long)]
    files_from: Option<PathBuf>,
    /// Show diff between CMD's stdin and stdout
    #[clap(short = 'F', long)]
    filter: bool,
    /// Command to execute
    #[clap(name = "CMD")]
    cmd_name: String,
    /// Command arguments
    #[clap(name = "ARG", trailing_var_arg = true)]
    cmd_args: Vec<String>,
    /// To debug parallelism
    #[doc(hidden)]
    #[clap(long, hide = true)]
    force_parallel: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    let stdout = io::stdout();
    let stdout_lock = stdout.lock();
    let bufw = BufWriter::new(stdout_lock);
    let default = args.files_from.is_none() && !args.single_arg && !args.multi_args && !args.filter;
    if default {
        run_with_files_from_stdin(&args, bufw)?;
    } else if let Some(files_from) = args.files_from.as_ref() {
        if Path::new("-") == files_from {
            run_with_files_from_stdin(&args, bufw)?;
        } else {
            run_with_files_from_file(&args, bufw, files_from)?;
        }
    } else if args.filter {
        run_with_stdin(&args, bufw)?;
    } else if args.single_arg {
        run_with_files_from_single_arg(&args, bufw)?;
    } else {
        run_with_files_from_multi_args(&args, bufw)?;
    }
    Ok(())
}

fn run_with_stdin<W: Write>(args: &Args, bufw: W) -> Result<()> {
    let stdin = io::stdin();
    let stdin_lock = stdin.lock();
    let bufr = BufReader::new(stdin_lock);
    exec_with_buf_read(args, bufr, bufw)?;
    Ok(())
}

fn run_with_files_from_stdin<W: Write>(args: &Args, bufw: W) -> Result<()> {
    let stdin = io::stdin();
    let stdin_lock = stdin.lock();
    let bufr = BufReader::new(stdin_lock);
    run_with_files_from_buf_reader(args, bufw, bufr)?;
    Ok(())
}

fn run_with_files_from_file<W: Write>(args: &Args, bufw: W, path: &Path) -> Result<()> {
    let file = File::open(path)?;
    let bufr = BufReader::new(file);
    run_with_files_from_buf_reader(args, bufw, bufr)?;
    Ok(())
}

fn run_with_files_from_buf_reader<W: Write, R: BufRead>(
    args: &Args,
    mut bufw: W,
    bufr: R,
) -> Result<()> {
    if args.null {
        process_results(bufr.split(0), |lines| {
            exec_multiple_files(
                args,
                &mut bufw,
                &args.cmd_args,
                lines.map(|line| OsString::from_vec(line).into()),
            )
        })??;
    } else {
        process_results(bufr.lines(), |lines| {
            exec_multiple_files(
                args,
                &mut bufw,
                &args.cmd_args,
                lines.map(|line| line.into()),
            )
        })??;
    }
    Ok(())
}

fn run_with_files_from_single_arg<W: Write>(args: &Args, bufw: W) -> Result<()> {
    let cmd_args = args.cmd_args.as_slice();
    let cmd_opts = &cmd_args[..cmd_args.len() - 1];
    let file = Path::new(&cmd_args[cmd_args.len() - 1]);
    exec_one_file(args, bufw, cmd_opts, file)?;
    Ok(())
}

fn run_with_files_from_multi_args<W: Write>(args: &Args, mut bufw: W) -> Result<()> {
    let cmd_args = args.cmd_args.as_slice();
    let last_components = cmd_args.split(|s| s == "--").last();
    if let Some(filestrs) = last_components {
        let cmd_opts = &cmd_args[..cmd_args.len() - filestrs.len()];
        exec_multiple_files(
            args,
            &mut bufw,
            cmd_opts,
            filestrs.iter().map(|line| line.into()),
        )?;
    } else {
        // invalid
    }
    Ok(())
}

fn exec_multiple_files<W: Write, I: Iterator<Item = PathBuf>>(
    args: &Args,
    w: W,
    cmd_args: &[String],
    files: I,
) -> Result<()> {
    let threads = if args.threads > 0 {
        NonZeroUsize::new(args.threads as usize).unwrap()
    } else {
        available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap())
    };
    if threads <= NonZeroUsize::new(1).unwrap() && !args.force_parallel {
        serial_exec_multiple_files(args, w, cmd_args, files)
    } else if args.unordered {
        parallel_exec_multiple_files_unordered(args, w, cmd_args, files, threads)
    } else {
        parallel_exec_multiple_files_ordered(args, w, cmd_args, files, threads)
    }
}

fn serial_exec_multiple_files<W: Write, I: Iterator<Item = PathBuf>>(
    args: &Args,
    mut w: W,
    cmd_args: &[String],
    files: I,
) -> Result<()> {
    let mut count = 0usize;
    for file in files {
        exec_one_file(args, &mut w, cmd_args, &file)?;
        count += 1;
    }
    trace!("processed: {}", count);
    Ok(())
}

fn exec_one_file<W: Write>(args: &Args, w: W, cmd_args: &[String], file: &Path) -> Result<()> {
    let mut command = Command::new(&args.cmd_name);
    let inf = File::open(file)?;
    let mut inbr = BufReader::new(inf);
    let mut inb = Vec::<u8>::new();
    inbr.read_to_end(&mut inb)?;
    let child = command
        .args(cmd_args)
        .arg(file)
        .stdout(Stdio::piped())
        .spawn()?;
    let output = child.wait_with_output()?;
    if output.status.success() {
        let name = file.to_string_lossy();
        diff(args, w, &name, &inb, &name, &output.stdout)?;
    }
    Ok(())
}

fn exec_with_buf_read<R: BufRead, W: Write>(args: &Args, mut r: R, w: W) -> Result<()> {
    let mut command = Command::new(&args.cmd_name);
    let mut inb = Vec::<u8>::new();
    r.read_to_end(&mut inb)?;
    let child = command
        .args(&args.cmd_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    child.stdin.as_ref().unwrap().write_all(&inb)?;
    let output = child.wait_with_output()?;
    if output.status.success() {
        diff(args, w, "<stdin>", &inb, "<stdout>", &output.stdout)?;
    }
    Ok(())
}

fn diff<W: Write>(_args: &Args, w: W, aname: &str, a: &[u8], bname: &str, b: &[u8]) -> Result<()> {
    let diff = TextDiff::from_lines(a, b);
    let mut udiff = diff.unified_diff();
    let udiff = udiff.header(aname, bname);
    udiff.to_writer(w)?;
    Ok(())
}
