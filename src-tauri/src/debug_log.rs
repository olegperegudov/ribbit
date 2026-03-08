use chrono::Local;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;

pub fn log(msg: &str) {
    let log_dir = match dirs::config_dir() {
        Some(d) => d.join("ribbit").join("logs"),
        None => return,
    };

    let _ = create_dir_all(&log_dir);

    let log_file = log_dir.join("debug.log");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        let ts = Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] {}", ts, msg);
    }
}
