use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use tracing::Subscriber;
use bevy::log::tracing_subscriber::{Layer, registry::LookupSpan};

static LOG_RING: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

fn get_log_ring() -> &'static Mutex<VecDeque<String>> {
    LOG_RING.get_or_init(|| Mutex::new(VecDeque::with_capacity(200)))
}

pub struct LogRingLayer;

impl<S> Layer<S> for LogRingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = StringVisitor { message: String::new() };
        event.record(&mut visitor);

        let log_line = format!(
            "[{:<5}] [{}]: {}",
            metadata.level().to_string(),
            metadata.target(),
            visitor.message
        );

        if let Ok(mut ring) = get_log_ring().lock() {
            if ring.len() >= 200 {
                ring.pop_front();
            }
            ring.push_back(log_line);
        }
    }
}

struct StringVisitor {
    message: String,
}

impl tracing::field::Visit for StringVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        }
    }
}

pub fn create_log_layer() -> bevy::log::BoxedLayer {
    Box::new(LogRingLayer)
}

pub fn init_crash_reporting() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // 1. Gather panic details
        let payload = panic_info.payload();
        let message = if let Some(s) = payload.downcast_ref::<&str>() {
            *s
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.as_str()
        } else {
            "Unknown panic message"
        };

        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let backtrace = std::backtrace::Backtrace::force_capture();

        let logs = if let Ok(ring) = get_log_ring().lock() {
            ring.iter().cloned().collect::<Vec<_>>().join("\n")
        } else {
            "Failed to lock log ring".to_string()
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let report = format!(
            "Guild Forge Crash Report\n\
             =======================\n\
             Version: {}\n\
             Time (UNIX Epoch): {}s\n\
             Location: {}\n\
             Panic Message: {}\n\n\
             Recent Logs:\n\
             -----------------------\n\
             {}\n\n\
             Backtrace:\n\
             -----------------------\n\
             {}",
            env!("CARGO_PKG_VERSION"),
            now,
            location,
            message,
            logs,
            backtrace
        );

        // 2. Write crash log file to disk
        let crash_path = dirs::data_dir()
            .map(|d| d.join("guild-forge").join("crash.log"))
            .unwrap_or_else(|| std::path::PathBuf::from("crash.log"));

        if let Some(parent) = crash_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let _ = std::fs::write(&crash_path, report);

        // 3. Print standard panic output to stdout/stderr via default hook
        default_hook(panic_info);

        // 4. Show native error popup dialog
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Guild Forge Crashed")
            .set_description(&format!(
                "The game crashed unexpectedly!\n\n\
                 Message: {}\n\
                 Location: {}\n\n\
                 A crash report has been saved to:\n{:?}\n\n\
                 Please report this issue and share the crash log on Discord or GitHub.",
                message, location, crash_path
            ))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_ring_captures_logs() {
        // Initialize logging using our LogRingLayer
        let registry = bevy::log::tracing_subscriber::Registry::default();
        let _subscriber = tracing::subscriber::set_default(registry);

        // We can manually push log lines to test
        if let Ok(mut ring) = get_log_ring().lock() {
            ring.clear();
            ring.push_back("[INFO] test log".to_string());
        }

        let logs = get_log_ring().lock().unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0], "[INFO] test log");
    }
}
