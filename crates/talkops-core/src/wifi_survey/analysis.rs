//! Rating of roams, pattern detection and recommendations (ADR 0041).
//!
//! Pure functions over the stored samples. The media worker runs them after
//! the call; the live view uses the same rules for a preliminary rating.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Event, MediaSample, RadioSample, Thresholds};

/// Seconds around a roam that belong to its window.
pub const WINDOW_MS: i32 = 3000;
/// Markers this close to a roam count as "near".
pub const MARKER_NEAR_MS: i32 = 10_000;

/// Rating of a roam or of a time period.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    Clean,
    Audible,
    Bad,
}

impl Rating {
    pub fn as_str(self) -> &'static str {
        match self {
            Rating::Clean => "clean",
            Rating::Audible => "audible",
            Rating::Bad => "bad",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "clean" => Some(Rating::Clean),
            "audible" => Some(Rating::Audible),
            "bad" => Some(Rating::Bad),
            _ => None,
        }
    }

    /// Rating of a window by its longest gap and its loss share. Loss above
    /// `loss_block_pct` in the window is a loss block.
    pub fn of(gap_ms: i32, window_loss_pct: f32, t: &Thresholds) -> Self {
        if gap_ms > t.bad_ms || window_loss_pct > t.loss_block_pct as f32 {
            Rating::Bad
        } else if gap_ms >= t.clean_ms {
            Rating::Audible
        } else {
            Rating::Clean
        }
    }
}

/// One roam (AP change) or band change (same AP, other radio or channel).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Roam {
    pub offset_ms: i32,
    pub from_ap_mac: Option<String>,
    pub from_ap_name: Option<String>,
    pub to_ap_mac: Option<String>,
    pub to_ap_name: Option<String>,
    pub from_radio: Option<String>,
    pub to_radio: Option<String>,
    pub from_channel: Option<i32>,
    pub to_channel: Option<i32>,
    pub rssi_before: Option<i32>,
    pub rssi_after: Option<i32>,
    /// Longest run of missing uplink packets in the window.
    pub gap_ms: i32,
    /// Uplink loss in the window, in percent.
    pub loss_pct: f32,
    /// Highest downlink jitter (RTCP) in the window.
    pub jitter_ms: Option<f32>,
    pub rating: Rating,
    /// `band_change`, `pingpong`, `sticky_before`, `no_drop`.
    pub flags: Vec<String>,
    /// Markers within ±10 s.
    pub markers: i32,
}

/// A detected pattern with its time span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Pattern {
    /// `sticky_client`, `pingpong`, `drop_without_roam`, `roam_without_drop`,
    /// `weak_coverage`, `vad_suspected`.
    pub kind: String,
    pub start_ms: i32,
    pub end_ms: i32,
    /// AP name or MAC, where it applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ap: Option<String>,
    /// Second AP (ping-pong).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other_ap: Option<String>,
    /// Gap (drop without roam) or lowest RSSI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<i32>,
}

/// A rule-based recommendation. The UI builds the text from `id` and
/// `values`, so both languages use the same result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Recommendation {
    pub id: String,
    /// `info`, `warning` or `critical`.
    pub severity: String,
    #[schema(value_type = Object)]
    pub values: serde_json::Value,
}

/// Time on one AP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApShare {
    pub ap_mac: String,
    pub ap_name: Option<String>,
    pub seconds: i32,
    pub avg_rssi: Option<i32>,
    pub min_rssi: Option<i32>,
}

/// Summary of a survey.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Summary {
    pub duration_ms: i32,
    pub roams: i32,
    pub clean: i32,
    pub audible: i32,
    pub bad: i32,
    pub band_changes: i32,
    pub worst_gap_ms: i32,
    pub loss_pct: f32,
    pub mos: Option<f32>,
    pub min_rssi: Option<i32>,
    pub avg_rssi: Option<i32>,
    pub markers: i32,
    pub drops_without_roam: i32,
    pub vad_suspected: bool,
    pub has_radio: bool,
    pub aps: Vec<ApShare>,
    pub patterns: Vec<Pattern>,
    pub recommendations: Vec<Recommendation>,
}

/// Result of an analysis.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Analysis {
    pub roams: Vec<Roam>,
    pub summary: Summary,
}

/// Missing uplink packets of one sample (never negative: a burst after a
/// gap does not fill the gap).
fn missing(s: &MediaSample) -> i32 {
    (s.expected - s.received).max(0)
}

fn loss_pct(samples: &[&MediaSample]) -> f32 {
    let expected: i64 = samples.iter().map(|s| s.expected as i64).sum();
    let missing: i64 = samples.iter().map(|s| missing(s) as i64).sum();
    if expected == 0 {
        0.0
    } else {
        (missing as f32 * 100.0 / expected as f32).min(100.0)
    }
}

/// A sample is "silent" if no packet arrived without a sequence gap: the
/// handset suppressed silence (VAD), the packets were not lost.
fn silent(s: &MediaSample) -> bool {
    s.expected > 0 && s.received == 0 && s.flaws == 0
}

/// A run of seconds with missing packets: start, end and gap length.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Gap {
    start_ms: i32,
    end_ms: i32,
    gap_ms: i32,
}

/// Runs of consecutive samples with missing packets. A run's length is the
/// number of missing packets multiplied by the packet time.
fn gaps(media: &[MediaSample], ptime_ms: i32) -> Vec<Gap> {
    let mut out: Vec<Gap> = Vec::new();
    let mut open: Option<Gap> = None;
    for s in media {
        let m = if silent(s) { 0 } else { missing(s) };
        if m > 0 {
            let start = s.offset_ms - s.interval_ms;
            let g = open.get_or_insert(Gap {
                start_ms: start,
                end_ms: s.offset_ms,
                gap_ms: 0,
            });
            g.end_ms = s.offset_ms;
            g.gap_ms += m * ptime_ms;
        } else if let Some(g) = open.take() {
            out.push(g);
        }
    }
    out.extend(open);
    out
}

fn media_in(media: &[MediaSample], from: i32, to: i32) -> Vec<&MediaSample> {
    // A sample covers (offset - interval, offset].
    media
        .iter()
        .filter(|s| s.offset_ms > from && s.offset_ms - s.interval_ms < to)
        .collect()
}

fn ap_label(mac: Option<&str>, name: Option<&str>) -> Option<String> {
    name.filter(|n| !n.is_empty()).or(mac).map(str::to_owned)
}

/// Radio samples with an AP (poll errors and disconnected periods removed).
fn connected(radio: &[RadioSample]) -> Vec<&RadioSample> {
    radio.iter().filter(|r| r.ap_mac.is_some()).collect()
}

/// Rates the window around `[from, to]`.
fn rate_window(
    media: &[MediaSample],
    gaps: &[Gap],
    from: i32,
    to: i32,
    t: &Thresholds,
) -> (i32, f32, Option<f32>, Rating) {
    let (lo, hi) = (from - WINDOW_MS, to + WINDOW_MS);
    let window = media_in(media, lo, hi);
    let gap = gaps
        .iter()
        .filter(|g| g.end_ms > lo && g.start_ms < hi)
        .map(|g| g.gap_ms)
        .max()
        .unwrap_or(0);
    let jitter = window
        .iter()
        .filter_map(|s| s.rtcp_jitter_ms)
        .fold(None, |acc: Option<f32>, j| {
            Some(acc.map_or(j, |a| a.max(j)))
        });
    let loss = loss_pct(&window);
    (gap, loss, jitter, Rating::of(gap, loss, t))
}

/// Analyses a survey. `media`, `radio` and `events` must be sorted by
/// `offset_ms`.
pub fn analyse(
    media: &[MediaSample],
    radio: &[RadioSample],
    events: &[Event],
    ptime_ms: i32,
    t: &Thresholds,
) -> Analysis {
    let ptime_ms = ptime_ms.max(1);
    let gaps = gaps(media, ptime_ms);
    let markers: Vec<i32> = events
        .iter()
        .filter(|e| e.kind == "marker")
        .map(|e| e.offset_ms)
        .collect();
    let near = |at: i32| {
        markers
            .iter()
            .filter(|m| (**m - at).abs() <= MARKER_NEAR_MS)
            .count() as i32
    };

    // Roams and band changes between consecutive connected samples.
    let conn = connected(radio);
    let mut roams = Vec::new();
    for pair in conn.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let ap_change = a.ap_mac != b.ap_mac;
        let band_change = !ap_change && (a.radio != b.radio || a.channel != b.channel);
        if !ap_change && !band_change {
            continue;
        }
        let at = (a.offset_ms + b.offset_ms) / 2;
        let (gap_ms, loss, jitter, rating) = rate_window(media, &gaps, a.offset_ms, b.offset_ms, t);
        let mut flags = Vec::new();
        if band_change {
            flags.push("band_change".to_owned());
        }
        if rating == Rating::Clean {
            flags.push("no_drop".to_owned());
        }
        roams.push(Roam {
            offset_ms: at,
            from_ap_mac: a.ap_mac.clone(),
            from_ap_name: a.ap_name.clone(),
            to_ap_mac: b.ap_mac.clone(),
            to_ap_name: b.ap_name.clone(),
            from_radio: a.radio.clone(),
            to_radio: b.radio.clone(),
            from_channel: a.channel,
            to_channel: b.channel,
            rssi_before: a.rssi,
            rssi_after: b.rssi,
            gap_ms,
            loss_pct: loss,
            jitter_ms: jitter,
            rating,
            flags,
            markers: near(at),
        });
    }

    let mut patterns = Vec::new();

    // Ping-pong: `pingpong_count` AP changes between the same two APs
    // within `pingpong_secs`.
    let ap_roams: Vec<usize> = (0..roams.len())
        .filter(|i| !roams[*i].flags.iter().any(|f| f == "band_change"))
        .collect();
    let count = t.pingpong_count.max(2) as usize;
    let mut marked_until = i32::MIN;
    for start in 0..ap_roams.len() {
        let first = roams[ap_roams[start]].clone();
        if first.offset_ms <= marked_until {
            continue;
        }
        let pair = |r: &Roam| {
            let mut p = [r.from_ap_mac.clone(), r.to_ap_mac.clone()];
            p.sort();
            p
        };
        let key = pair(&first);
        let run: Vec<usize> = ap_roams[start..]
            .iter()
            .copied()
            .take_while(|i| {
                pair(&roams[*i]) == key
                    && roams[*i].offset_ms - first.offset_ms <= t.pingpong_secs * 1000
            })
            .collect();
        if run.len() >= count {
            let last = roams[*run.last().unwrap_or(&ap_roams[start])].offset_ms;
            for i in &run {
                roams[*i].flags.push("pingpong".to_owned());
            }
            patterns.push(Pattern {
                kind: "pingpong".into(),
                start_ms: first.offset_ms,
                end_ms: last,
                ap: ap_label(first.from_ap_mac.as_deref(), first.from_ap_name.as_deref()),
                other_ap: ap_label(first.to_ap_mac.as_deref(), first.to_ap_name.as_deref()),
                value: Some(run.len() as i32),
            });
            marked_until = last;
        }
    }

    // Runs of radio samples on one AP below an RSSI limit.
    let low_runs = |limit: i32, same_ap: bool| -> Vec<(i32, i32, usize, i32)> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < conn.len() {
            if conn[i].rssi.is_some_and(|r| r < limit) {
                let mut j = i;
                let mut min = conn[i].rssi.unwrap_or(limit);
                while j + 1 < conn.len()
                    && conn[j + 1].rssi.is_some_and(|r| r < limit)
                    && (!same_ap || conn[j + 1].ap_mac == conn[i].ap_mac)
                {
                    j += 1;
                    min = min.min(conn[j].rssi.unwrap_or(min));
                }
                out.push((conn[i].offset_ms, conn[j].offset_ms, i, min));
                i = j + 1;
            } else {
                i += 1;
            }
        }
        out
    };
    for (start, end, i, min) in low_runs(t.sticky_rssi, true) {
        if end - start < t.sticky_secs * 1000 {
            continue;
        }
        // Sticky: the handset stayed, then roamed or lost packets.
        let roamed_after = roams.iter().any(|r| {
            r.offset_ms >= end
                && r.offset_ms <= end + WINDOW_MS * 2
                && r.from_ap_mac == conn[i].ap_mac
        });
        let lost = loss_pct(&media_in(media, start, end)) > 0.0;
        if roamed_after || lost {
            for r in roams.iter_mut().filter(|r| {
                r.offset_ms >= end
                    && r.offset_ms <= end + WINDOW_MS * 2
                    && r.from_ap_mac == conn[i].ap_mac
            }) {
                r.flags.push("sticky_before".to_owned());
            }
            patterns.push(Pattern {
                kind: "sticky_client".into(),
                start_ms: start,
                end_ms: end,
                ap: ap_label(conn[i].ap_mac.as_deref(), conn[i].ap_name.as_deref()),
                other_ap: None,
                value: Some(min),
            });
        }
    }
    for (start, end, i, min) in low_runs(t.weak_rssi, false) {
        if end - start >= t.weak_secs * 1000 {
            patterns.push(Pattern {
                kind: "weak_coverage".into(),
                start_ms: start,
                end_ms: end,
                ap: ap_label(conn[i].ap_mac.as_deref(), conn[i].ap_name.as_deref()),
                other_ap: None,
                value: Some(min),
            });
        }
    }

    // Drops without roam: audible gaps with no AP change near them.
    let mut drops = 0;
    for g in &gaps {
        if g.gap_ms < t.clean_ms {
            continue;
        }
        let roam_near = roams
            .iter()
            .any(|r| r.offset_ms >= g.start_ms - WINDOW_MS && r.offset_ms <= g.end_ms + WINDOW_MS);
        if !roam_near {
            drops += 1;
            patterns.push(Pattern {
                kind: "drop_without_roam".into(),
                start_ms: g.start_ms,
                end_ms: g.end_ms,
                ap: radio_at(&conn, g.start_ms)
                    .and_then(|r| ap_label(r.ap_mac.as_deref(), r.ap_name.as_deref())),
                other_ap: None,
                value: Some(g.gap_ms),
            });
        }
    }
    for r in roams.iter().filter(|r| r.rating == Rating::Clean) {
        patterns.push(Pattern {
            kind: "roam_without_drop".into(),
            start_ms: r.offset_ms,
            end_ms: r.offset_ms,
            ap: ap_label(r.to_ap_mac.as_deref(), r.to_ap_name.as_deref()),
            other_ap: ap_label(r.from_ap_mac.as_deref(), r.from_ap_name.as_deref()),
            value: Some(r.gap_ms),
        });
    }

    // Silence suppression: more than 10 % of the seconds without packets
    // and without a sequence gap.
    let silent_secs = media.iter().filter(|s| silent(s)).count();
    let vad = !media.is_empty() && silent_secs * 10 > media.len();
    if vad {
        patterns.push(Pattern {
            kind: "vad_suspected".into(),
            start_ms: media.first().map_or(0, |s| s.offset_ms),
            end_ms: media.last().map_or(0, |s| s.offset_ms),
            ap: None,
            other_ap: None,
            value: Some(silent_secs as i32),
        });
    }
    patterns.sort_by_key(|p| p.start_ms);

    let all: Vec<&MediaSample> = media.iter().collect();
    let rssi: Vec<i32> = conn.iter().filter_map(|r| r.rssi).collect();
    let mut aps: BTreeMap<String, (Option<String>, i32, Vec<i32>)> = BTreeMap::new();
    for pair in conn.windows(2) {
        let entry = aps
            .entry(pair[0].ap_mac.clone().unwrap_or_default())
            .or_insert((pair[0].ap_name.clone(), 0, Vec::new()));
        entry.1 += pair[1].offset_ms - pair[0].offset_ms;
        entry.2.extend(pair[0].rssi);
    }
    if let Some(last) = conn.last() {
        let entry = aps
            .entry(last.ap_mac.clone().unwrap_or_default())
            .or_insert((last.ap_name.clone(), 0, Vec::new()));
        entry.2.extend(last.rssi);
    }
    let avg = |v: &[i32]| (!v.is_empty()).then(|| v.iter().sum::<i32>() / v.len() as i32);
    let mut summary = Summary {
        duration_ms: media
            .last()
            .map(|s| s.offset_ms)
            .max(conn.last().map(|r| r.offset_ms))
            .unwrap_or(0),
        roams: roams
            .iter()
            .filter(|r| !r.flags.iter().any(|f| f == "band_change"))
            .count() as i32,
        clean: roams.iter().filter(|r| r.rating == Rating::Clean).count() as i32,
        audible: roams.iter().filter(|r| r.rating == Rating::Audible).count() as i32,
        bad: roams.iter().filter(|r| r.rating == Rating::Bad).count() as i32,
        band_changes: roams
            .iter()
            .filter(|r| r.flags.iter().any(|f| f == "band_change"))
            .count() as i32,
        worst_gap_ms: gaps.iter().map(|g| g.gap_ms).max().unwrap_or(0),
        loss_pct: loss_pct(&all),
        mos: media.iter().rev().find_map(|s| s.mos),
        min_rssi: rssi.iter().copied().min(),
        avg_rssi: avg(&rssi),
        markers: markers.len() as i32,
        drops_without_roam: drops,
        vad_suspected: vad,
        has_radio: !conn.is_empty(),
        aps: aps
            .into_iter()
            .map(|(mac, (name, ms, rssi))| ApShare {
                ap_mac: mac,
                ap_name: name,
                seconds: ms / 1000,
                avg_rssi: avg(&rssi),
                min_rssi: rssi.iter().copied().min(),
            })
            .collect(),
        patterns,
        recommendations: Vec::new(),
    };
    summary.recommendations = recommend(&summary, &roams, media);
    Analysis { roams, summary }
}

fn radio_at<'a>(conn: &[&'a RadioSample], at: i32) -> Option<&'a RadioSample> {
    conn.iter()
        .rev()
        .find(|r| r.offset_ms <= at)
        .copied()
        .or(conn.first().copied())
}

fn rec(id: &str, severity: &str, values: serde_json::Value) -> Recommendation {
    Recommendation {
        id: id.into(),
        severity: severity.into(),
        values,
    }
}

/// Rule-based recommendations. They say what to check; TalkOps does not
/// read the Wi-Fi configuration.
fn recommend(s: &Summary, roams: &[Roam], media: &[MediaSample]) -> Vec<Recommendation> {
    use serde_json::json;
    let mut out = Vec::new();
    let kinds = |k: &str| s.patterns.iter().filter(|p| p.kind == k).count();
    if s.vad_suspected {
        out.push(rec("disable_vad", "critical", json!({})));
    }
    if s.audible + s.bad > 0 {
        let slow: Vec<&Roam> = roams.iter().filter(|r| r.rating != Rating::Clean).collect();
        let avg = slow.iter().map(|r| r.gap_ms).sum::<i32>() / slow.len().max(1) as i32;
        out.push(rec(
            "fast_roaming",
            if s.bad > 0 { "critical" } else { "warning" },
            json!({ "count": slow.len(), "avg_gap_ms": avg }),
        ));
    }
    if kinds("sticky_client") > 0 {
        let rssi = s
            .patterns
            .iter()
            .filter(|p| p.kind == "sticky_client")
            .filter_map(|p| p.value)
            .min();
        out.push(rec(
            "min_rssi",
            "warning",
            json!({ "count": kinds("sticky_client"), "rssi": rssi }),
        ));
    }
    if kinds("pingpong") > 0 {
        let p = s.patterns.iter().find(|p| p.kind == "pingpong");
        out.push(rec(
            "pingpong",
            "warning",
            json!({ "ap": p.and_then(|p| p.ap.clone()), "other_ap": p.and_then(|p| p.other_ap.clone()) }),
        ));
    }
    if kinds("weak_coverage") > 0 {
        let aps: Vec<String> = s
            .patterns
            .iter()
            .filter(|p| p.kind == "weak_coverage")
            .filter_map(|p| p.ap.clone())
            .collect();
        out.push(rec(
            "coverage",
            "warning",
            json!({ "count": kinds("weak_coverage"), "aps": aps, "markers": s.markers }),
        ));
    }
    if s.drops_without_roam > 0 {
        out.push(rec(
            "interference",
            "warning",
            json!({ "count": s.drops_without_roam }),
        ));
    }
    // Downlink loss (RTCP) without uplink loss.
    let downlink = media
        .iter()
        .filter_map(|m| m.rtcp_fraction_lost)
        .fold(0.0_f32, f32::max);
    if downlink >= 1.0 && s.loss_pct < 0.5 {
        out.push(rec(
            "downlink_loss",
            "warning",
            json!({ "loss_pct": (downlink * 10.0).round() / 10.0 }),
        ));
    }
    let on_24 = roams
        .iter()
        .any(|r| r.to_radio.as_deref() == Some("ng") || r.from_radio.as_deref() == Some("ng"));
    if on_24 && s.loss_pct >= 1.0 {
        out.push(rec("band_5ghz", "info", json!({})));
    }
    if !s.has_radio {
        out.push(rec("connect_unifi", "info", json!({})));
    }
    // General checks for a voice SSID are always useful.
    out.push(rec("voice_ssid", "info", json!({})));
    out
}

/// Comparison of the same AP pair in two surveys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PairComparison {
    pub from_ap: String,
    pub to_ap: String,
    pub a_count: i32,
    pub b_count: i32,
    pub a_avg_gap_ms: Option<i32>,
    pub b_avg_gap_ms: Option<i32>,
    pub a_worst: Option<Rating>,
    pub b_worst: Option<Rating>,
}

/// Groups the roams of two surveys by AP pair (direction-independent).
pub fn compare_pairs(a: &[Roam], b: &[Roam]) -> Vec<PairComparison> {
    type Acc = (i32, i32, Option<Rating>);
    let mut pairs: BTreeMap<(String, String), (Acc, Acc)> = BTreeMap::new();
    let key = |r: &Roam| {
        let x = ap_label(r.from_ap_mac.as_deref(), r.from_ap_name.as_deref()).unwrap_or_default();
        let y = ap_label(r.to_ap_mac.as_deref(), r.to_ap_name.as_deref()).unwrap_or_default();
        if x <= y { (x, y) } else { (y, x) }
    };
    let add = |acc: &mut Acc, r: &Roam| {
        acc.0 += 1;
        acc.1 += r.gap_ms;
        acc.2 = Some(acc.2.map_or(r.rating, |w| w.max(r.rating)));
    };
    for r in a
        .iter()
        .filter(|r| !r.flags.iter().any(|f| f == "band_change"))
    {
        add(&mut pairs.entry(key(r)).or_default().0, r);
    }
    for r in b
        .iter()
        .filter(|r| !r.flags.iter().any(|f| f == "band_change"))
    {
        add(&mut pairs.entry(key(r)).or_default().1, r);
    }
    pairs
        .into_iter()
        .map(|((from_ap, to_ap), (x, y))| PairComparison {
            from_ap,
            to_ap,
            a_count: x.0,
            b_count: y.0,
            a_avg_gap_ms: (x.0 > 0).then(|| x.1 / x.0),
            b_avg_gap_ms: (y.0 > 0).then(|| y.1 / y.0),
            a_worst: x.2,
            b_worst: y.2,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> Thresholds {
        Thresholds::default()
    }

    /// One sample each second; `lost` lists (second, missing packets).
    fn media(secs: i32, lost: &[(i32, i32)]) -> Vec<MediaSample> {
        (1..=secs)
            .map(|s| {
                let m = lost.iter().find(|(x, _)| *x == s).map_or(0, |(_, m)| *m);
                MediaSample {
                    offset_ms: s * 1000,
                    interval_ms: 1000,
                    received: 50 - m,
                    expected: 50,
                    flaws: m,
                    mos: Some(4.4),
                    jitter_max_ms: None,
                    rtcp_fraction_lost: None,
                    rtcp_jitter_ms: None,
                }
            })
            .collect()
    }

    /// Radio samples each second: (from second, AP, RSSI).
    fn radio(secs: i32, plan: &[(i32, &str, i32)]) -> Vec<RadioSample> {
        (1..=secs)
            .map(|s| {
                let (_, ap, rssi) = plan.iter().rev().find(|(from, _, _)| *from <= s).unwrap();
                RadioSample {
                    offset_ms: s * 1000,
                    ap_mac: Some(format!("aa0000000{ap}")),
                    ap_name: Some(format!("AP {ap}")),
                    bssid: None,
                    radio: Some("na".into()),
                    channel: Some(36),
                    rssi: Some(*rssi),
                    tx_rate_kbps: None,
                    rx_rate_kbps: None,
                    error: None,
                }
            })
            .collect()
    }

    fn marker(at: i32) -> Event {
        Event {
            id: uuid::Uuid::nil(),
            offset_ms: at,
            kind: "marker".into(),
            marker: Some("disturbance".into()),
            source: Some("dtmf".into()),
            note: String::new(),
            data: serde_json::json!({}),
        }
    }

    #[test]
    fn rates_clean_audible_and_bad_roams() {
        for (missing, rating) in [(1, Rating::Clean), (4, Rating::Audible), (10, Rating::Bad)] {
            let m = media(20, &[(10, missing)]);
            let r = radio(20, &[(1, "1", -60), (10, "2", -55)]);
            let a = analyse(&m, &r, &[marker(12000)], 20, &t());
            assert_eq!(a.roams.len(), 1);
            let roam = &a.roams[0];
            assert_eq!(roam.rating, rating, "{missing} missing packets");
            assert_eq!(roam.gap_ms, missing * 20);
            assert_eq!(roam.from_ap_name.as_deref(), Some("AP 1"));
            assert_eq!(roam.rssi_before, Some(-60));
            assert_eq!(roam.rssi_after, Some(-55));
            assert_eq!(roam.markers, 1);
        }
    }

    #[test]
    fn a_loss_block_is_bad_even_with_short_gaps() {
        // Four gaps of 100 ms in the window of 7 s: 20 of 350 packets (5.7 %).
        let m = media(20, &[(7, 5), (9, 5), (11, 5), (13, 5)]);
        let a = analyse(
            &m,
            &radio(20, &[(1, "1", -60), (10, "2", -60)]),
            &[],
            20,
            &t(),
        );
        assert_eq!(a.roams[0].gap_ms, 100);
        assert!(a.roams[0].loss_pct > 5.0);
        assert_eq!(a.roams[0].rating, Rating::Bad);
    }

    #[test]
    fn gaps_across_seconds_are_one_gap() {
        let m = media(20, &[(9, 3), (10, 3)]);
        let a = analyse(&m, &radio(20, &[(1, "1", -60)]), &[], 20, &t());
        assert_eq!(a.summary.worst_gap_ms, 120);
        assert_eq!(a.summary.drops_without_roam, 1);
        assert!(
            a.summary
                .patterns
                .iter()
                .any(|p| p.kind == "drop_without_roam")
        );
        assert!(
            a.summary
                .recommendations
                .iter()
                .any(|r| r.id == "interference")
        );
    }

    #[test]
    fn detects_pingpong() {
        let m = media(30, &[]);
        let r = radio(
            30,
            &[(1, "1", -70), (5, "2", -70), (10, "1", -70), (15, "2", -70)],
        );
        let a = analyse(&m, &r, &[], 20, &t());
        assert_eq!(a.summary.roams, 3);
        assert!(a.summary.patterns.iter().any(|p| p.kind == "pingpong"));
        assert!(
            a.roams
                .iter()
                .all(|r| r.flags.contains(&"pingpong".to_owned()))
        );
        assert_eq!(a.summary.clean, 3);
        assert!(
            a.summary
                .patterns
                .iter()
                .any(|p| p.kind == "roam_without_drop")
        );
    }

    #[test]
    fn detects_sticky_client_and_weak_coverage() {
        let m = media(40, &[(20, 2)]);
        let r = radio(40, &[(1, "1", -60), (5, "1", -82), (22, "2", -58)]);
        let a = analyse(&m, &r, &[], 20, &t());
        let kinds: Vec<&str> = a.summary.patterns.iter().map(|p| p.kind.as_str()).collect();
        assert!(kinds.contains(&"sticky_client"), "{kinds:?}");
        assert!(kinds.contains(&"weak_coverage"), "{kinds:?}");
        assert!(a.roams[0].flags.contains(&"sticky_before".to_owned()));
        let ids: Vec<&str> = a
            .summary
            .recommendations
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        assert!(
            ids.contains(&"min_rssi") && ids.contains(&"coverage"),
            "{ids:?}"
        );
        assert_eq!(a.summary.min_rssi, Some(-82));
    }

    #[test]
    fn band_change_is_not_a_roam() {
        let m = media(10, &[]);
        let mut r = radio(10, &[(1, "1", -60)]);
        for s in r.iter_mut().skip(5) {
            s.radio = Some("ng".into());
            s.channel = Some(6);
        }
        let a = analyse(&m, &r, &[], 20, &t());
        assert_eq!(a.summary.roams, 0);
        assert_eq!(a.summary.band_changes, 1);
        assert!(a.roams[0].flags.contains(&"band_change".to_owned()));
    }

    #[test]
    fn silence_suppression_is_not_loss() {
        let mut m = media(20, &[]);
        for s in m.iter_mut().skip(5).take(5) {
            s.received = 0;
            s.flaws = 0;
        }
        let a = analyse(&m, &[], &[], 20, &t());
        assert!(a.summary.vad_suspected);
        assert_eq!(a.summary.worst_gap_ms, 0);
        assert_eq!(a.summary.recommendations[0].id, "disable_vad");
        assert!(!a.summary.has_radio);
    }

    #[test]
    fn compares_pairs_in_both_directions() {
        let m = media(20, &[(10, 4)]);
        let a = analyse(
            &m,
            &radio(20, &[(1, "1", -60), (10, "2", -60)]),
            &[],
            20,
            &t(),
        );
        let b = analyse(
            &media(20, &[]),
            &radio(20, &[(1, "2", -60), (10, "1", -60)]),
            &[],
            20,
            &t(),
        );
        let pairs = compare_pairs(&a.roams, &b.roams);
        assert_eq!(pairs.len(), 1);
        assert_eq!((pairs[0].a_count, pairs[0].b_count), (1, 1));
        assert_eq!(pairs[0].a_worst, Some(Rating::Audible));
        assert_eq!(pairs[0].b_worst, Some(Rating::Clean));
    }
}
