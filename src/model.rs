use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct Config {
    pub names: Vec<String>,
    pub path_contains: String,
    pub blink_ms: u32,
    pub timeout_seconds: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            names: vec!["client.exe".into()],
            path_contains: "mabinogi".into(),
            blink_ms: 650,
            timeout_seconds: 300,
        }
    }
}
impl Config {
    pub fn parse(text: &str) -> Self {
        let mut c = Self::default();
        for line in text.lines() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.starts_with('#') {
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let val = val.trim();
            match key.trim() {
                "process_names" => {
                    let names: Vec<_> = val
                        .split(',')
                        .map(|s| s.trim().to_lowercase())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if !names.is_empty() {
                        c.names = names;
                    }
                }
                "path_contains" => c.path_contains = val.to_lowercase(),
                "blink_ms" => {
                    if let Ok(n) = val.parse::<u32>() {
                        c.blink_ms = if n == 0 { 0 } else { n.clamp(500, 10000) };
                    }
                }
                "timeout_seconds" => {
                    if let Ok(n) = val.parse::<u64>() {
                        c.timeout_seconds = n.clamp(1, 3600);
                    }
                }
                _ => {}
            }
        }
        c
    }
}

#[derive(Default)]
pub struct Reminder {
    pub known: HashSet<u32>,
    pub started_ms: Option<u64>,
    pub game_bound: bool,
}
impl Reminder {
    // Only new process IDs trigger reminders; dismissal survives ongoing polling.
    pub fn observe(&mut self, pids: HashSet<u32>, now: u64) -> bool {
        let new = pids.difference(&self.known).next().is_some();
        if pids.is_empty() && self.game_bound {
            self.dismiss();
        }
        self.known = pids;
        if new {
            self.show(now);
            self.game_bound = true;
        }
        new
    }
    pub fn show(&mut self, now: u64) {
        self.started_ms = Some(now);
        self.game_bound = false;
    }
    pub fn dismiss(&mut self) {
        self.started_ms = None;
        self.game_bound = false;
    }
    pub fn expire(&mut self, now: u64, seconds: u64) -> bool {
        if self
            .started_ms
            .is_some_and(|t| now.saturating_sub(t) >= seconds * 1000)
        {
            self.dismiss();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ids(xs: &[u32]) -> HashSet<u32> {
        xs.iter().copied().collect()
    }
    #[test]
    fn dismiss_does_not_reopen_until_new_launch() {
        let mut r = Reminder::default();
        assert!(r.observe(ids(&[10]), 0));
        r.dismiss();
        assert!(!r.observe(ids(&[10]), 2000));
        assert_eq!(r.started_ms, None);
        assert!(!r.observe(ids(&[]), 4000));
        assert!(r.observe(ids(&[11]), 6000));
    }
    #[test]
    fn expires_at_five_minutes_and_not_before() {
        let mut r = Reminder::default();
        r.show(100);
        assert!(!r.expire(300099, 300));
        assert!(r.expire(300100, 300));
        assert!(!r.expire(400000, 300));
    }
    #[test]
    fn multiple_clients_do_not_repeat_every_poll() {
        let mut r = Reminder::default();
        assert!(r.observe(ids(&[1, 2]), 0));
        r.dismiss();
        assert!(!r.observe(ids(&[1, 2]), 1));
        assert!(r.observe(ids(&[1, 2, 3]), 2));
    }
    #[test]
    fn game_exit_closes_reminder_but_one_remaining_client_keeps_it() {
        let mut r = Reminder::default();
        r.observe(ids(&[1, 2]), 0);
        r.observe(ids(&[2]), 2000);
        assert_eq!(r.started_ms, Some(0));
        r.observe(ids(&[]), 4000);
        assert_eq!(r.started_ms, None);
        assert!(r.observe(ids(&[3]), 6000));
    }
    #[test]
    fn manual_preview_does_not_require_a_running_game() {
        let mut r = Reminder::default();
        r.show(0);
        assert!(!r.observe(ids(&[]), 2000));
        assert_eq!(r.started_ms, Some(0));
    }
    #[test]
    fn configuration_is_bounded_and_case_insensitive() {
        let c = Config::parse("process_names=Client.EXE, Other.exe\npath_contains=MaBi\nblink_ms=10\ntimeout_seconds=0");
        assert_eq!(c.names, vec!["client.exe", "other.exe"]);
        assert_eq!(c.path_contains, "mabi");
        assert_eq!(c.blink_ms, 500);
        assert_eq!(c.timeout_seconds, 1);
        assert_eq!(Config::parse("blink_ms=0").blink_ms, 0);
        assert_eq!(Config::parse("timeout_seconds=bad").timeout_seconds, 300);
    }
}
