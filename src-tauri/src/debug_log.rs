use chrono::Local;
use std::io::Write;

pub fn log(msg: &str) {
    let log_dir = match dirs::config_dir() {
        Some(d) => d.join("ribbit").join("logs"),
        None => return,
    };

    let _ = crate::private::create_dir(&log_dir);

    let log_file = log_dir.join("debug.log");
    if let Ok(mut file) = crate::private::append(&log_file) {
        let ts = Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] {}", ts, msg);
    }
}
