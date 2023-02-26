///
/// Forked from https://github.com/RegenJacob/egui_logger
///
use std::{collections::VecDeque, ops::DerefMut, sync::Mutex};

use egui::{Color32, RichText};
use log::SetLoggerError;

use regex::{Regex, RegexBuilder};

const LEVELS: [log::Level; log::Level::Trace as usize] = [
    log::Level::Error,
    log::Level::Warn,
    log::Level::Info,
    log::Level::Debug,
    log::Level::Trace,
];

const DEFAULT_LOG_LENGTH: usize = 10000;
const DEFAULT_DISPLAY_LENGTH: usize = 10000;

// static LOG: Mutex<LogCollector> = Mutex::new(new_collector());
static LOG: once_cell::sync::Lazy<Mutex<LogCollector>> =
    once_cell::sync::Lazy::new(Default::default);

static LOGGER_UI: once_cell::sync::Lazy<Mutex<LoggerUi>> =
    once_cell::sync::Lazy::new(Default::default);

/// Initilizes the global logger.
/// Should be called very early in the program
pub fn init(noisy_modules: &[&'static str]) -> Result<(), SetLoggerError> {
    let mut collector = LOG.lock().unwrap();
    collector.noisy_modules.extend_from_slice(noisy_modules);
    log::set_logger(&EguiLogger).map(|()| log::set_max_level(log::STATIC_MAX_LEVEL))
}

struct EguiLogger;

impl log::Log for EguiLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::STATIC_MAX_LEVEL
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            let mut log = LOG.lock().unwrap();
            log.push_record(record);
        }
    }

    fn flush(&self) {}
}

struct LogRecord {
    level: log::Level,
    args: String,
    module: Option<String>,
}

impl LogRecord {
    fn display_module(&self, leading_space: bool) -> ModuleDisplay<'_> {
        ModuleDisplay {
            leading_space,
            module: self.module.as_deref(),
        }
    }
}

impl std::fmt::Display for LogRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:5}]{}: {}",
            self.level,
            self.display_module(true),
            self.args
        )
    }
}

struct ModuleDisplay<'a> {
    leading_space: bool,
    module: Option<&'a str>,
}

impl<'a> std::fmt::Display for ModuleDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.module {
            Some(m) => {
                if self.leading_space {
                    write!(f, " ")?;
                }
                write!(f, "{}", m)
            }
            None => std::fmt::Result::Ok(()),
        }
    }
}

impl<'a, 'b> From<&'b log::Record<'a>> for LogRecord {
    fn from(record: &'b log::Record<'a>) -> Self {
        LogRecord {
            level: record.level(),
            args: record.args().to_string(),
            module: record.module_path().map(|s| s.to_owned()),
        }
    }
}

struct LogCollector {
    buf: RingCollector<LogRecord>,
    noisy_modules: Vec<&'static str>,
}

impl Default for LogCollector {
    fn default() -> Self {
        LogCollector {
            buf: RingCollector::new(DEFAULT_LOG_LENGTH),
            noisy_modules: Vec::new(),
        }
    }
}

impl LogCollector {
    fn push_record(&mut self, record: &log::Record<'_>) {
        if record.level() > log::Level::Warn {
            if let Some(module) = record.module_path() {
                if self.noisy_modules.iter().any(|nm| module.starts_with(nm)) {
                    return;
                }
            }
        }
        self.buf.push(LogRecord::from(record));
    }
    fn clear(&mut self) {
        self.buf.clear()
    }
    fn set_buf_size(&mut self, len: usize) {
        self.buf.ring_size = std::cmp::max(len, DEFAULT_LOG_LENGTH);
    }
    fn len(&self) -> usize {
        self.buf.len()
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LoggerUi {
    loglevels: [bool; log::Level::Trace as usize],
    search_term: String,
    #[serde(skip)]
    regex: Option<Regex>,
    search_case_sensitive: bool,
    search_use_regex: bool,
    max_log_length: usize,
}

impl Default for LoggerUi {
    fn default() -> Self {
        Self {
            loglevels: [true, true, true, false, false],
            search_term: String::new(),
            search_case_sensitive: false,
            regex: None,
            search_use_regex: false,
            max_log_length: DEFAULT_DISPLAY_LENGTH,
        }
    }
}

impl LoggerUi {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut collector = LOG.lock().unwrap();

        ui.horizontal(|ui| {
            if ui.button("Clear").clicked() {
                collector.clear();
            }
            ui.menu_button("Log Levels", |ui| {
                for level in LEVELS {
                    if ui
                        .selectable_label(self.loglevels[level as usize - 1], level.as_str())
                        .clicked()
                    {
                        self.loglevels[level as usize - 1] = !self.loglevels[level as usize - 1];
                    }
                }
            });
        });

        ui.horizontal(|ui| {
            ui.label("Search: ");
            let response = ui.text_edit_singleline(&mut self.search_term);

            let mut config_changed = false;

            if ui
                .selectable_label(self.search_case_sensitive, "Aa")
                .on_hover_text("Case sensitive")
                .clicked()
            {
                self.search_case_sensitive = !self.search_case_sensitive;
                config_changed = true;
            };
            if ui
                .selectable_label(self.search_use_regex, ".*")
                .on_hover_text("Use regex")
                .clicked()
            {
                self.search_use_regex = !self.search_use_regex;
                config_changed = true;
            }
            if self.search_use_regex
                && (response.changed() || config_changed || self.regex.is_none())
            {
                self.regex = RegexBuilder::new(&self.search_term)
                    .case_insensitive(!self.search_case_sensitive)
                    .build()
                    .ok()
            };
        });

        ui.horizontal(|ui| {
            ui.label("Max Log output");
            ui.add(egui::widgets::DragValue::new(&mut self.max_log_length).speed(1));
        });

        ui.horizontal(|ui| {
            if ui.button("Sort").clicked() {
                // logs.sort()
            }
        });
        ui.separator();

        let mut logs_displayed: usize = 0;

        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .max_height(ui.available_height() - 30.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let records = self.record_buf(collector.deref_mut());
                for record in records.iter() {
                    let string_format = format!("{}", record);
                    let rich =
                        RichText::new(string_format).text_style(egui::style::TextStyle::Monospace);

                    let rich = match record.level {
                        log::Level::Trace => rich.weak(),
                        log::Level::Info => {
                            // Green
                            rich.color(Color32::from_rgb(140, 148, 64))
                        }
                        log::Level::Warn => {
                            // Orange
                            rich.color(Color32::from_rgb(222, 148, 95))
                        }
                        log::Level::Error => {
                            // Red
                            rich.color(Color32::from_rgb(165, 66, 82))
                        }
                        _ => rich,
                    };
                    ui.label(rich);

                    // self.copy_text.push_str(&record.args);
                    // self.copy_text.push('\n');
                    logs_displayed += 1;
                }
            });

        ui.horizontal(|ui| {
            ui.label(format!("Log size: {}", collector.len()));
            ui.label(format!("Displayed: {}", logs_displayed));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Copy").clicked() {
                    ui.output_mut(|o| {
                        let records = self.record_buf(collector.deref_mut());
                        let text = records
                            .iter()
                            .map(|r| format!("{}", r))
                            .collect::<Vec<_>>()
                            .join("\n");
                        o.copied_text = text;
                    });
                }
            });
        });

        collector.set_buf_size(self.max_log_length);
    }

    fn iter_logs<'c, 'a: 'c>(
        &'a self,
        collector: &'c mut LogCollector,
    ) -> impl Iterator<Item = &'c LogRecord> + 'c {
        collector.buf.iter().filter(move |r| {
            self.loglevels[r.level as usize - 1]
                && (self.search_term.is_empty() || self.match_string(&r.args))
        })
    }

    fn record_buf<'c, 'a: 'c>(
        &'a self,
        collector: &'c mut LogCollector,
    ) -> RingCollector<&'c LogRecord> {
        let mut ring = RingCollector::new(self.max_log_length);
        ring.collect(self.iter_logs(collector));
        ring
    }

    fn match_string(&self, string: &str) -> bool {
        if self.search_use_regex {
            if let Some(matcher) = &self.regex {
                matcher.is_match(string)
            } else {
                // Failed to compile
                false
            }
        } else {
            if self.search_case_sensitive {
                string.contains(&self.search_term)
            } else {
                string
                    .to_lowercase()
                    .contains(&self.search_term.to_lowercase())
            }
        }
    }
}

struct RingCollector<T> {
    inner: VecDeque<T>,
    ring_size: usize,
}

impl<T> RingCollector<T> {
    fn new(capacity: usize) -> RingCollector<T> {
        RingCollector {
            inner: VecDeque::with_capacity(capacity + 1),
            ring_size: capacity,
        }
    }
    fn push(&mut self, value: T) {
        self.inner.push_back(value);
        let extra = self.inner.len().saturating_sub(self.ring_size);
        let _ = self.inner.drain(..extra);
    }
    fn collect<I: Iterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.push(item);
        }
    }
    fn len(&self) -> usize {
        self.inner.len()
    }
    fn clear(&mut self) {
        self.inner.clear()
    }
    fn iter(&self) -> std::collections::vec_deque::Iter<'_, T> {
        self.inner.iter()
    }
}

/// Draws the logger ui
/// has to be called after [`init()`](init());
pub fn logger_ui(ui: &mut egui::Ui) {
    let mut logger_ui = LOGGER_UI.lock().unwrap();

    logger_ui.ui(ui);
}
