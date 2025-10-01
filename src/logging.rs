use chrono::Local;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;

pub fn init_logging() {
    let mut builder = Builder::from_default_env();
    builder
        .format(|buf, record| {
            let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S");
            writeln!(
                buf,
                "{timestamp} [{level}] {target}: {message}",
                timestamp = timestamp,
                level = record.level(),
                target = record.target(),
                message = record.args()
            )
        })
        .filter(None, LevelFilter::Info);

    let _ = builder.try_init();
}
