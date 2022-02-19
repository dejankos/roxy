use log::LevelFilter;
use simplelog::{ConfigBuilder, TermLogger, TerminalMode, ThreadLogMode, WriteLogger};
use std::fs::File;

pub fn init_logger(log_path: Option<String>, dev_mode: bool) {
    let cfg = ConfigBuilder::new()
        .set_thread_mode(ThreadLogMode::Both)
        .build();

    if log_path.is_none() || dev_mode {
        TermLogger::init(LevelFilter::Info, cfg, TerminalMode::Mixed)
            .expect("Failed to init term logger");
    } else {
        let log_file =
            File::create(format!("{}/roxy.log", log_path.unwrap())).expect("Can't create log file");
        WriteLogger::init(LevelFilter::Info, cfg, log_file).expect("Failed to init file logger")
    }
}
