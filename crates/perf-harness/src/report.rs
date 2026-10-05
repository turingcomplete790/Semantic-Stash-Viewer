//! The performance report (003 data-model "Performance report", research R8, FR-015/FR-016).

use serde::{Deserialize, Serialize};

/// More than this much slower than the previous run counts as a regression (FR-015).
const REGRESSION_PERCENT: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Ms,
    Fps,
    Count,
    Percent,
    /// Megabytes of memory.
    Mb,
}

impl Unit {
    fn higher_is_better(self) -> bool {
        self == Unit::Fps
    }

    fn label(self) -> &'static str {
        match self {
            Unit::Ms => "ms",
            Unit::Fps => "fps",
            Unit::Count => "",
            Unit::Percent => "%",
            Unit::Mb => "MB",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Pass,
    Fail,
    Invalid,
    /// Reported without a budget.
    Info,
}

/// Which statistic the budget applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Median,
    P95,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    pub median: f64,
    pub p95: f64,
    pub max: f64,
    pub samples: usize,
}

/// Median, p95, and max (nearest-rank, like the UI bench). `None` without samples.
pub fn summarise(samples: &[f64]) -> Option<Summary> {
    let mut sorted: Vec<f64> = samples.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(f64::total_cmp);
    let at = |q: f64| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let i = (q * sorted.len() as f64).floor() as usize;
        sorted[i.min(sorted.len() - 1)]
    };
    Some(Summary {
        median: at(0.5),
        p95: at(0.95),
        max: sorted[sorted.len() - 1],
        samples: sorted.len(),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    pub name: String,
    pub unit: Unit,
    pub budget: Option<f64>,
    pub median: f64,
    pub p95: f64,
    pub max: f64,
    pub samples: usize,
    pub result: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub previous_median: Option<f64>,
    pub change: Option<f64>,
    pub regressed: bool,
}

impl Measurement {
    /// A measurement from its summary. An `invalid` reason (or no samples) wins over the numbers.
    pub fn new(
        name: &str,
        unit: Unit,
        budget: Option<f64>,
        check: Check,
        summary: Option<Summary>,
        invalid: Option<String>,
    ) -> Self {
        let s = summary.unwrap_or(Summary {
            median: 0.0,
            p95: 0.0,
            max: 0.0,
            samples: 0,
        });
        let reason = invalid.or_else(|| summary.is_none().then(|| "no samples".to_owned()));
        let result = if reason.is_some() {
            Outcome::Invalid
        } else if let Some(budget) = budget {
            let value = match check {
                Check::Median => s.median,
                Check::P95 => s.p95,
            };
            let ok = if unit.higher_is_better() {
                value >= budget
            } else {
                value <= budget
            };
            if ok {
                Outcome::Pass
            } else {
                Outcome::Fail
            }
        } else {
            Outcome::Info
        };
        Self {
            name: name.to_owned(),
            unit,
            budget,
            median: s.median,
            p95: s.p95,
            max: s.max,
            samples: s.samples,
            result,
            reason,
            previous_median: None,
            change: None,
            regressed: false,
        }
    }

    /// Straight from raw samples.
    pub fn from_samples(
        name: &str,
        unit: Unit,
        budget: Option<f64>,
        samples: &[f64],
        invalid: Option<String>,
    ) -> Self {
        Self::new(
            name,
            unit,
            budget,
            Check::Median,
            summarise(samples),
            invalid,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub created_at: String,
    pub app_version: String,
    pub machine: String,
    /// The profile's display name only: never an address or key.
    pub profile: String,
    /// `web` or `native` (006); reports written before 006 are the web build's.
    #[serde(default = "web_app")]
    pub app: String,
    pub measurements: Vec<Measurement>,
}

fn web_app() -> String {
    "web".into()
}

impl Report {
    pub fn failed(&self) -> bool {
        self.measurements.iter().any(|m| m.result == Outcome::Fail)
    }

    /// Fill in the change since `previous` (valid measurements with the same name only).
    pub fn compare(&mut self, previous: Option<&Report>) {
        let Some(previous) = previous else {
            return;
        };
        for m in &mut self.measurements {
            let before = previous
                .measurements
                .iter()
                .find(|p| p.name == m.name && p.result != Outcome::Invalid && p.samples > 0);
            let Some(before) = before else {
                continue;
            };
            m.previous_median = Some(before.median);
            if m.result == Outcome::Invalid || before.median == 0.0 {
                continue;
            }
            let change = (m.median - before.median) / before.median * 100.0;
            m.change = Some((change * 10.0).round() / 10.0);
            m.regressed = if m.unit.higher_is_better() {
                change < -REGRESSION_PERCENT
            } else {
                change > REGRESSION_PERCENT
            };
        }
    }

    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "# Performance report\n\n- **App**: {}\n- **Profile**: {}\n- **When**: {}\n- **Version**: {}\n- **Machine**: {}\n\n",
            self.app, self.profile, self.created_at, self.app_version, self.machine
        );
        out.push_str("| Measurement | Result | Median | p95 | Max | Budget | Samples | Change |\n");
        out.push_str("|---|---|---|---|---|---|---|---|\n");
        for m in &self.measurements {
            let unit = m.unit.label();
            let num = |v: f64| format!("{}{unit}", trim(v));
            let result = match (m.result, &m.reason) {
                (Outcome::Invalid, Some(r)) => format!("invalid ({r})"),
                (Outcome::Pass, _) => "pass".to_owned(),
                (Outcome::Fail, _) => "**FAIL**".to_owned(),
                (Outcome::Invalid, None) => "invalid".to_owned(),
                (Outcome::Info, _) => "info".to_owned(),
            };
            let change = match m.change {
                Some(c) if m.regressed => format!("**{c:+}% (regressed)**"),
                Some(c) => format!("{c:+}%"),
                None => "–".to_owned(),
            };
            let (median, p95, max) = if m.samples == 0 {
                ("–".to_owned(), "–".to_owned(), "–".to_owned())
            } else {
                (num(m.median), num(m.p95), num(m.max))
            };
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                m.name,
                result,
                median,
                p95,
                max,
                m.budget.map_or_else(|| "–".to_owned(), num),
                m.samples,
                change
            ));
        }
        out
    }
}

/// Whole numbers without decimals, others to one place.
fn trim(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(measurements: Vec<Measurement>) -> Report {
        Report {
            created_at: "2026-09-29T12:00:00Z".into(),
            app_version: "0.1.0".into(),
            machine: "Linux, test CPU".into(),
            profile: "Home server".into(),
            app: "web".into(),
            measurements,
        }
    }

    fn ms(name: &str, budget: Option<f64>, samples: &[f64]) -> Measurement {
        Measurement::from_samples(name, Unit::Ms, budget, samples, None)
    }

    #[test]
    fn summarise_gives_median_p95_and_max() {
        let samples: Vec<f64> = (1..=20).map(f64::from).collect();
        let s = summarise(&samples).expect("summary");
        assert_eq!(s.median, 11.0);
        assert_eq!(s.p95, 20.0);
        assert_eq!(s.max, 20.0);
        assert_eq!(s.samples, 20);
        assert!(summarise(&[]).is_none());
        assert_eq!(summarise(&[f64::NAN, 5.0]).expect("finite only").samples, 1);
    }

    #[test]
    fn budgets_pass_and_fail_on_the_median() {
        assert_eq!(
            ms("nav", Some(150.0), &[110.0, 120.0, 130.0]).result,
            Outcome::Pass
        );
        assert_eq!(
            ms("nav", Some(150.0), &[170.0, 180.0, 190.0]).result,
            Outcome::Fail
        );
        assert_eq!(ms("cold-cleared", None, &[900.0]).result, Outcome::Info);
        let fps = Measurement::from_samples("fps", Unit::Fps, Some(60.0), &[58.0], None);
        assert_eq!(fps.result, Outcome::Fail, "fps: higher is better");
    }

    #[test]
    fn a_p95_budget_checks_the_p95() {
        let samples: Vec<f64> = (0..100).map(|i| if i < 90 { 8.0 } else { 30.0 }).collect();
        let m = Measurement::new(
            "scroll-frame-time",
            Unit::Ms,
            Some(16.7),
            Check::P95,
            summarise(&samples),
            None,
        );
        assert_eq!(m.median, 8.0);
        assert_eq!(m.result, Outcome::Fail);
    }

    #[test]
    fn invalid_wins_over_the_numbers() {
        let m = Measurement::from_samples(
            "nav",
            Unit::Ms,
            Some(150.0),
            &[10.0],
            Some("window hidden".into()),
        );
        assert_eq!(m.result, Outcome::Invalid);
        assert_eq!(m.reason.as_deref(), Some("window hidden"));
        let empty = ms("nav", Some(150.0), &[]);
        assert_eq!(empty.result, Outcome::Invalid);
        assert_eq!(empty.reason.as_deref(), Some("no samples"));
    }

    #[test]
    fn comparison_gives_the_change_and_flags_regressions() {
        let previous = report(vec![
            ms("nav", Some(150.0), &[100.0]),
            ms("press", Some(50.0), &[20.0]),
            Measurement::from_samples("fps", Unit::Fps, None, &[60.0], None),
            Measurement::from_samples("drops", Unit::Count, Some(1.0), &[0.5], None),
        ]);
        let mut current = report(vec![
            ms("nav", Some(150.0), &[125.0]),
            ms("press", Some(50.0), &[23.0]),
            Measurement::from_samples("fps", Unit::Fps, None, &[45.0], None),
            Measurement::from_samples("drops", Unit::Count, Some(1.0), &[0.4], None),
            ms("new", None, &[5.0]),
        ]);
        current.compare(Some(&previous));
        let by = |n: &str| current.measurements.iter().find(|m| m.name == n).expect(n);
        assert_eq!(by("nav").change, Some(25.0));
        assert!(by("nav").regressed, "25% slower");
        assert_eq!(by("press").change, Some(15.0));
        assert!(!by("press").regressed, "15% is within the margin");
        assert_eq!(by("fps").change, Some(-25.0));
        assert!(by("fps").regressed, "fewer fps is worse");
        assert!(!by("drops").regressed, "fewer drops is better");
        assert_eq!(by("drops").previous_median, Some(0.5));
        assert_eq!(by("new").previous_median, None);
        assert_eq!(by("new").change, None);
    }

    #[test]
    fn an_invalid_previous_measurement_is_not_compared() {
        let previous = report(vec![Measurement::from_samples(
            "nav",
            Unit::Ms,
            Some(150.0),
            &[10.0],
            Some("window hidden".into()),
        )]);
        let mut current = report(vec![ms("nav", Some(150.0), &[100.0])]);
        current.compare(Some(&previous));
        assert_eq!(current.measurements[0].change, None);
        current.compare(None);
        assert_eq!(current.measurements[0].change, None);
    }

    #[test]
    fn markdown_has_one_row_per_measurement_and_no_address() {
        let mut r = report(vec![
            ms("nav", Some(150.0), &[120.0]),
            ms("press", Some(50.0), &[80.0]),
            Measurement::from_samples("drops", Unit::Count, Some(1.0), &[], Some("x".into())),
        ]);
        r.compare(None);
        let md = r.to_markdown();
        let rows = md
            .lines()
            .filter(|l| l.starts_with("| ") && !l.starts_with("| Measurement"));
        assert_eq!(rows.count(), 3);
        assert!(md.contains("Home server"));
        assert!(md.contains("**FAIL**"));
        assert!(md.contains("invalid (x)"));
        assert!(!md.contains("http"), "never an address");
        let json = serde_json::to_string(&r).expect("json");
        let back: Report = serde_json::from_str(&json).expect("round trip");
        assert_eq!(back, r);
    }

    #[test]
    fn reports_from_before_the_native_build_are_the_web_builds() {
        let old = r#"{"createdAt":"2026-09-29T12:00:00Z","appVersion":"0.1.0","machine":"m","profile":"p","measurements":[]}"#;
        let r: Report = serde_json::from_str(old).expect("old report");
        assert_eq!(r.app, "web");
    }
}
