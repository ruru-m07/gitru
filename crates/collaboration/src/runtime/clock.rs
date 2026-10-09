use super::*;

/// An accepted provider wait is permission, not a bounded scheduler wake.
/// Overflow means no representable Instant in this process can reach the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum AccountCooldown {
    At(Instant),
    BeyondInstantRange,
}
impl AccountCooldown {
    pub(super) fn after(now: Instant, delay: Duration) -> Self {
        now.checked_add(delay)
            .map(Self::At)
            .unwrap_or(Self::BeyondInstantRange)
    }
    pub(super) fn remaining(self, now: Instant) -> Option<Duration> {
        match self {
            Self::At(deadline) => deadline
                .checked_duration_since(now)
                .filter(|d| !d.is_zero()),
            Self::BeyondInstantRange => Some(Duration::from_secs(u64::MAX)),
        }
    }
}
impl From<Instant> for AccountCooldown {
    fn from(deadline: Instant) -> Self {
        Self::At(deadline)
    }
}

pub(super) trait Clock: Send + Sync {
    fn now(&self) -> Instant;
    fn utc(&self) -> DateTime<Utc>;
    fn jitter(&self) -> u64 {
        u64::from(uuid::Uuid::new_v4().as_bytes()[0])
    }
}
pub(super) struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
impl CollaborationRuntime {
    pub(super) fn now(&self) -> Instant {
        self.clock.now()
    }
    pub(super) fn now_string(&self) -> String {
        self.clock
            .utc()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }
    pub(super) fn future_string(&self, seconds: u64) -> String {
        // Durable deadlines are parsed as RFC3339 on restart. Chrono's own
        // MAX_UTC has a six-digit year that its RFC3339 parser cannot reread.
        let ceiling = DateTime::<Utc>::from_timestamp(253_402_300_799, 999_000_000)
            .expect("last representable RFC3339 millisecond");
        self.clock
            .utc()
            .checked_add_signed(chrono::Duration::seconds(
                seconds.min(i64::MAX as u64 / 1000) as i64,
            ))
            .map(|value| value.min(ceiling))
            .unwrap_or(ceiling)
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }
    pub(super) fn delay_until(&self, time: &str) -> Option<Duration> {
        let delay = DateTime::parse_from_rfc3339(time)
            .ok()?
            .signed_duration_since(self.clock.utc())
            .num_milliseconds();
        (delay > 0).then(|| Duration::from_millis((delay as u64).min(86_400_000)))
    }
    pub(super) fn provider_delay_until(&self, time: &str) -> Option<Duration> {
        self.wall_delay_until(time)
    }
    pub(super) fn wall_delay_until(&self, time: &str) -> Option<Duration> {
        let delay = DateTime::parse_from_rfc3339(time)
            .ok()?
            .signed_duration_since(self.clock.utc());
        delay.to_std().ok().filter(|delay| !delay.is_zero())
    }
    pub(super) fn deadline_after(&self, seconds: u64) -> Instant {
        self.now() + Duration::from_secs(seconds.min(86400))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_deadlines_keep_full_duration_and_overflow_fails_closed() {
        let now = Instant::now();
        let full = AccountCooldown::after(now, Duration::from_secs(48 * 3600));
        assert_eq!(
            full.remaining(now + Duration::from_secs(25 * 3600)),
            Some(Duration::from_secs(23 * 3600))
        );
        assert_eq!(full.remaining(now + Duration::from_secs(48 * 3600)), None);
        let extreme = AccountCooldown::after(now, Duration::from_secs(u64::MAX));
        assert_eq!(extreme, AccountCooldown::BeyondInstantRange);
        assert_eq!(extreme.max(full), extreme);
        assert!(
            extreme
                .remaining(now + Duration::from_secs(48 * 3600))
                .is_some()
        );
    }
}
