use std::env;
use std::io::{self, IsTerminal};
use std::time::Duration;

pub struct ProgressView {
    completed: usize,
    total: usize,
    color: bool,
}

impl ProgressView {
    pub fn new(collection: &str, completed: usize, total: usize) -> Self {
        let view = Self {
            completed,
            total,
            color: io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none(),
        };
        println!("\nMistborn {collection} setup");
        println!("{}", view.summary("ready"));
        view
    }

    pub fn skipped(&self, module: &str) {
        println!("  {} {module} (already completed)", self.green("✓"));
    }

    pub fn started(&self, module: &str, attempt: u32) {
        let retry = if attempt > 1 {
            format!(" · attempt {attempt}")
        } else {
            String::new()
        };
        println!(
            "\n{}  {} {module}{retry}",
            self.summary("running"),
            self.cyan("▶")
        );
    }

    pub fn completed(&mut self, module: &str, elapsed: Duration) {
        self.completed += 1;
        println!(
            "{}  {} {module} ({})",
            self.summary("complete"),
            self.green("✓"),
            duration(elapsed)
        );
    }

    pub fn failed(&self, module: &str, elapsed: Duration) {
        println!(
            "{}  {} {module} failed ({})",
            self.summary("failed"),
            self.red("✗"),
            duration(elapsed)
        );
    }

    pub fn finish(&self) {
        println!("\n{}", self.summary("complete"));
    }

    fn summary(&self, label: &str) -> String {
        format!(
            "[{}] {}/{} {label}",
            bar(self.completed, self.total, 16),
            self.completed,
            self.total
        )
    }

    fn cyan<'a>(&self, value: &'a str) -> Colored<'a> {
        Colored::new(value, "36", self.color)
    }

    fn green<'a>(&self, value: &'a str) -> Colored<'a> {
        Colored::new(value, "32", self.color)
    }

    fn red<'a>(&self, value: &'a str) -> Colored<'a> {
        Colored::new(value, "31", self.color)
    }
}

struct Colored<'a> {
    value: &'a str,
    code: &'static str,
    enabled: bool,
}

impl<'a> Colored<'a> {
    fn new(value: &'a str, code: &'static str, enabled: bool) -> Self {
        Self {
            value,
            code,
            enabled,
        }
    }
}

impl std::fmt::Display for Colored<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.enabled {
            write!(formatter, "\u{1b}[{}m{}\u{1b}[0m", self.code, self.value)
        } else {
            formatter.write_str(self.value)
        }
    }
}

fn bar(completed: usize, total: usize, width: usize) -> String {
    let filled = completed
        .saturating_mul(width)
        .checked_div(total)
        .unwrap_or(0)
        .min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn duration(elapsed: Duration) -> String {
    if elapsed.as_secs() >= 60 {
        format!("{}m {:02}s", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
    } else if elapsed.as_secs() > 0 {
        format!(
            "{}.{:01}s",
            elapsed.as_secs(),
            elapsed.subsec_millis() / 100
        )
    } else {
        format!("{}ms", elapsed.as_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_bar_represents_partial_completion() {
        assert_eq!(bar(2, 4, 8), "████░░░░");
        assert_eq!(bar(0, 0, 4), "░░░░");
    }

    #[test]
    fn elapsed_time_is_compact() {
        assert_eq!(duration(Duration::from_millis(420)), "420ms");
        assert_eq!(duration(Duration::from_millis(1_250)), "1.2s");
        assert_eq!(duration(Duration::from_secs(65)), "1m 05s");
    }
}
