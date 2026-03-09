use rusqlite::Connection;
use std::sync::{Mutex, OnceLock};

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

fn db() -> &'static Mutex<Connection> {
    DB.get_or_init(|| {
        let db_path = dirs::config_dir()
            .unwrap_or_default()
            .join("ribbit")
            .join("usage.db");
        std::fs::create_dir_all(db_path.parent().unwrap()).ok();
        let conn = Connection::open(&db_path).expect("Failed to open usage DB");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS daily_usage (
                date TEXT PRIMARY KEY,
                total_seconds REAL DEFAULT 0,
                count INTEGER DEFAULT 0
            )"
        ).expect("Failed to create table");
        Mutex::new(conn)
    })
}

pub fn record(seconds: f32) {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let conn = db().lock().unwrap();
    conn.execute(
        "INSERT INTO daily_usage (date, total_seconds, count)
         VALUES (?1, ?2, 1)
         ON CONFLICT(date) DO UPDATE SET
            total_seconds = total_seconds + ?2,
            count = count + 1",
        rusqlite::params![today, seconds as f64],
    ).ok();
}

pub fn get_monthly() -> Vec<serde_json::Value> {
    let conn = db().lock().unwrap();
    let start = (chrono::Local::now() - chrono::Duration::days(29))
        .format("%Y-%m-%d").to_string();

    let mut stmt = conn.prepare(
        "SELECT date, total_seconds, count FROM daily_usage
         WHERE date >= ?1 ORDER BY date"
    ).unwrap();

    let rows = stmt.query_map(rusqlite::params![start], |row| {
        Ok(serde_json::json!({
            "date": row.get::<_, String>(0)?,
            "seconds": row.get::<_, f64>(1)?,
            "count": row.get::<_, i64>(2)?,
        }))
    }).unwrap();

    rows.filter_map(|r| r.ok()).collect()
}
