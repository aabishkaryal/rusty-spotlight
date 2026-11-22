//! World time: `time in dubai`, `sydney time`, `3pm ist to sydney`,
//! `15:30 utc`, `9am dubai`. Pure: the caller supplies each zone's offset
//! from UTC at an instant (the system's time zone database), so summer time
//! is right without a copy of the database here.
//!
//! `3pm dubai` is 3 pm in Dubai, shown here. `3pm in dubai` and
//! `3pm to dubai` are 3 pm here, shown in Dubai. The answer names both sides.

/// A place the query names. `id` is an IANA zone, or `local` for this Mac's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub id: &'static str,
    pub name: &'static str,
}

pub const LOCAL: Zone = Zone { id: "local", name: "here" };

const fn z(id: &'static str, name: &'static str) -> Zone {
    Zone { id, name }
}

const ZONES: &[(&[&str], Zone)] = &[
    (&["here", "local", "me", "my time"], LOCAL),
    (&["utc", "gmt", "zulu"], z("UTC", "UTC")),
    (&["india", "ist", "delhi", "new delhi", "mumbai", "bombay", "bangalore", "bengaluru", "chennai", "kolkata", "calcutta", "hyderabad", "pune", "ahmedabad", "jaipur", "kochi"], z("Asia/Kolkata", "India")),
    (&["dubai", "uae", "gst", "abu dhabi", "sharjah", "ajman"], z("Asia/Dubai", "Dubai")),
    (&["nepal", "npt", "kathmandu", "pokhara"], z("Asia/Kathmandu", "Nepal")),
    (&["sydney", "aest", "aedt", "canberra", "australia"], z("Australia/Sydney", "Sydney")),
    (&["melbourne"], z("Australia/Melbourne", "Melbourne")),
    (&["brisbane"], z("Australia/Brisbane", "Brisbane")),
    (&["adelaide"], z("Australia/Adelaide", "Adelaide")),
    (&["perth", "awst"], z("Australia/Perth", "Perth")),
    (&["auckland", "new zealand", "nz", "nzst", "nzdt", "wellington"], z("Pacific/Auckland", "Auckland")),
    (&["london", "uk", "bst", "england", "britain", "manchester"], z("Europe/London", "London")),
    (&["dublin", "ireland"], z("Europe/Dublin", "Dublin")),
    (&["lisbon", "portugal"], z("Europe/Lisbon", "Lisbon")),
    (&["paris", "france", "cet", "cest"], z("Europe/Paris", "Paris")),
    (&["berlin", "germany", "frankfurt", "munich"], z("Europe/Berlin", "Berlin")),
    (&["amsterdam", "netherlands"], z("Europe/Amsterdam", "Amsterdam")),
    (&["madrid", "spain", "barcelona"], z("Europe/Madrid", "Madrid")),
    (&["rome", "italy", "milan"], z("Europe/Rome", "Rome")),
    (&["zurich", "switzerland", "geneva"], z("Europe/Zurich", "Zurich")),
    (&["stockholm", "sweden"], z("Europe/Stockholm", "Stockholm")),
    (&["istanbul", "turkey"], z("Europe/Istanbul", "Istanbul")),
    (&["moscow", "msk"], z("Europe/Moscow", "Moscow")),
    (&["cairo", "egypt"], z("Africa/Cairo", "Cairo")),
    (&["nairobi", "kenya"], z("Africa/Nairobi", "Nairobi")),
    (&["johannesburg", "south africa", "cape town"], z("Africa/Johannesburg", "Johannesburg")),
    (&["riyadh", "saudi", "saudi arabia", "jeddah"], z("Asia/Riyadh", "Riyadh")),
    (&["doha", "qatar"], z("Asia/Qatar", "Doha")),
    (&["muscat", "oman"], z("Asia/Muscat", "Muscat")),
    (&["karachi", "pakistan", "pkt", "lahore"], z("Asia/Karachi", "Karachi")),
    (&["dhaka", "bangladesh"], z("Asia/Dhaka", "Dhaka")),
    (&["colombo", "sri lanka"], z("Asia/Colombo", "Colombo")),
    (&["bangkok", "thailand"], z("Asia/Bangkok", "Bangkok")),
    (&["jakarta", "indonesia"], z("Asia/Jakarta", "Jakarta")),
    (&["kuala lumpur", "malaysia", "kl"], z("Asia/Kuala_Lumpur", "Kuala Lumpur")),
    (&["singapore", "sgt", "sg"], z("Asia/Singapore", "Singapore")),
    (&["manila", "philippines"], z("Asia/Manila", "Manila")),
    (&["hong kong", "hk", "hkt"], z("Asia/Hong_Kong", "Hong Kong")),
    (&["shanghai", "beijing", "china", "shenzhen"], z("Asia/Shanghai", "Shanghai")),
    (&["seoul", "korea", "kst"], z("Asia/Seoul", "Seoul")),
    (&["tokyo", "japan", "jst", "osaka"], z("Asia/Tokyo", "Tokyo")),
    (&["new york", "nyc", "ny", "est", "edt", "et", "eastern", "boston", "miami", "washington", "dc"], z("America/New_York", "New York")),
    (&["toronto", "canada"], z("America/Toronto", "Toronto")),
    (&["chicago", "cst", "cdt", "ct", "central", "austin", "dallas", "houston"], z("America/Chicago", "Chicago")),
    (&["denver", "mst", "mdt", "mt", "mountain"], z("America/Denver", "Denver")),
    (&["phoenix", "arizona"], z("America/Phoenix", "Phoenix")),
    (&["san francisco", "sf", "los angeles", "la", "seattle", "pst", "pdt", "pt", "pacific", "california", "vancouver"], z("America/Los_Angeles", "San Francisco")),
    (&["mexico city", "mexico"], z("America/Mexico_City", "Mexico City")),
    (&["sao paulo", "são paulo", "brazil", "rio"], z("America/Sao_Paulo", "São Paulo")),
    (&["buenos aires", "argentina"], z("America/Argentina/Buenos_Aires", "Buenos Aires")),
    (&["honolulu", "hawaii"], z("Pacific/Honolulu", "Honolulu")),
];

/// Every zone the table knows, for checking against the system's database.
pub fn zones() -> impl Iterator<Item = Zone> {
    ZONES.iter().map(|(_, zone)| *zone)
}

pub fn zone(text: &str) -> Option<Zone> {
    let text = text.trim().trim_end_matches(" time").trim();
    ZONES.iter().find(|(names, _)| names.contains(&text)).map(|(_, zone)| *zone)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimeQuery {
    /// The time now in `zone`.
    Now(Zone),
    /// `hour:minute` today in `from`, shown in `to`.
    At { hour: u32, minute: u32, from: Zone, to: Zone },
}

/// `3pm`, `3:30 pm`, `15:30`, `noon`, `midnight`. A bare `3` is not a time.
fn clock(s: &str) -> Option<((u32, u32), &str)> {
    for (word, hm) in [("noon", (12, 0)), ("midday", (12, 0)), ("midnight", (0, 0))] {
        if let Some(rest) = s.strip_prefix(word) {
            if rest.is_empty() || rest.starts_with(' ') {
                return Some((hm, rest.trim_start()));
            }
        }
    }
    let digits = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if digits == 0 || digits > 2 {
        return None;
    }
    let hour: u32 = s[..digits].parse().ok()?;
    let mut rest = &s[digits..];
    let mut minute = 0;
    let mut colon = false;
    if let Some(r) = rest.strip_prefix(':').or_else(|| rest.strip_prefix('.').filter(|r| r.len() >= 2 && r[..2].chars().all(|c| c.is_ascii_digit()))) {
        if r.len() < 2 || !r[..2].chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        minute = r[..2].parse().ok()?;
        rest = &r[2..];
        colon = true;
    }
    let trimmed = rest.trim_start();
    let mut meridiem = None;
    for (word, pm) in [("am", false), ("pm", true), ("a.m.", false), ("p.m.", true), ("a", false), ("p", true)] {
        if let Some(r) = trimmed.strip_prefix(word) {
            if r.is_empty() || r.starts_with(' ') {
                meridiem = Some(pm);
                rest = r;
                break;
            }
        }
    }
    let hour = match meridiem {
        Some(pm) if (1..=12).contains(&hour) => hour % 12 + if pm { 12 } else { 0 },
        Some(_) => return None,
        None if colon && hour < 24 => hour,
        None => return None,
    };
    if minute >= 60 || !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    Some(((hour, minute), rest.trim_start()))
}

pub fn parse(input: &str) -> Option<TimeQuery> {
    let q = input.trim().trim_end_matches('?').trim().to_lowercase();
    let q = q.split_whitespace().collect::<Vec<_>>().join(" ");
    for prefix in ["what time is it in ", "what's the time in ", "whats the time in ", "time in ", "time at ", "now in ", "time "] {
        if let Some(rest) = q.strip_prefix(prefix) {
            return zone(rest).map(TimeQuery::Now);
        }
    }
    if let Some(place) = q.strip_suffix(" time") {
        return zone(place).filter(|z| *z != LOCAL).map(TimeQuery::Now);
    }
    let ((hour, minute), rest) = clock(&q)?;
    let (from, to) = match rest.split_once(" to ").or_else(|| rest.split_once(" in ")) {
        Some((from, to)) => (zone(from)?, zone(to)?),
        None => match rest.strip_prefix("to ").or_else(|| rest.strip_prefix("in ")) {
            Some(to) => (LOCAL, zone(to)?),
            None if rest.is_empty() => return None,
            None => (zone(rest)?, LOCAL),
        },
    };
    (from != to).then_some(TimeQuery::At { hour, minute, from, to })
}

/// What the row shows. `title` is also what Return copies.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeAnswer {
    pub title: String,
    pub expression: String,
    pub note: String,
}

const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Days since 1970-01-01 to (year, month, day). Howard Hinnant's algorithm.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

fn clock_text(local_secs: i64) -> String {
    let secs = local_secs.rem_euclid(86_400);
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    format!("{}:{m:02} {}", if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "AM" } else { "PM" })
}

fn date_text(local_secs: i64) -> String {
    let days = local_secs.div_euclid(86_400);
    let (_, month, day) = civil(days);
    format!("{} {day} {}", DAYS[(days + 4).rem_euclid(7) as usize], MONTHS[month as usize - 1])
}

fn offset_text(secs: i32) -> String {
    let sign = if secs < 0 { "−" } else { "+" };
    let (h, m) = (secs.abs() / 3600, secs.abs() % 3600 / 60);
    if m == 0 { format!("UTC{sign}{h}") } else { format!("UTC{sign}{h}:{m:02}") }
}

fn gap_text(secs: i32) -> String {
    let (h, m) = (secs.abs() / 3600, secs.abs() % 3600 / 60);
    match (h, m) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// Work the query out at `now` (seconds since 1970, UTC). `offset(id, t)` is
/// the zone's offset from UTC in seconds at instant `t`, or `None` if unknown.
pub fn answer(query: &TimeQuery, now: i64, offset: &dyn Fn(&str, i64) -> Option<i32>) -> Option<TimeAnswer> {
    match *query {
        TimeQuery::Now(zone) => {
            let there = offset(zone.id, now)?;
            let here = offset(LOCAL.id, now)?;
            let t = now + i64::from(there);
            let gap = there - here;
            let relation = match gap {
                0 => "same time as here".to_string(),
                g if g > 0 => format!("{} ahead of here", gap_text(g)),
                g => format!("{} behind here", gap_text(g)),
            };
            let place = if zone == LOCAL { "Time here".to_string() } else { format!("Time in {}", zone.name) };
            Some(TimeAnswer {
                title: clock_text(t),
                expression: place,
                note: format!("{} · {} · {relation}", date_text(t), offset_text(there)),
            })
        }
        TimeQuery::At { hour, minute, from, to } => {
            // Today in `from`, at the time asked for.
            let from_now = offset(from.id, now)?;
            let day = (now + i64::from(from_now)).div_euclid(86_400);
            let wall = day * 86_400 + i64::from(hour * 3600 + minute * 60);
            let mut instant = wall - i64::from(from_now);
            let from_then = offset(from.id, instant)?;
            if from_then != from_now {
                instant = wall - i64::from(from_then); // summer time starts or ends today
            }
            let to_then = offset(to.id, instant)?;
            let shown = instant + i64::from(to_then);
            let day_gap = shown.div_euclid(86_400) - day;
            let when = match day_gap {
                0 => "same day",
                1 => "next day",
                -1 => "day before",
                _ => "",
            };
            Some(TimeAnswer {
                title: clock_text(shown),
                expression: format!("{} {} → {}", clock_text(wall), from.name, to.name),
                note: format!("{} · {} · {when}", date_text(shown), offset_text(to_then)),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDIA: i32 = 19_800;
    const DUBAI: i32 = 14_400;
    /// Sydney is UTC+11 in summer, UTC+10 otherwise; here summer starts at
    /// `SWITCH`, to check the instant is taken from the asked-for time.
    const SWITCH: i64 = 1_000_000_000;

    fn fixed(id: &str, t: i64) -> Option<i32> {
        Some(match id {
            "local" | "Asia/Kolkata" => INDIA,
            "Asia/Dubai" => DUBAI,
            "UTC" => 0,
            "Asia/Kathmandu" => 20_700,
            "Australia/Sydney" if t >= SWITCH => 39_600,
            "Australia/Sydney" => 36_000,
            _ => return None,
        })
    }

    fn zone_of(name: &str) -> Zone {
        zone(name).unwrap()
    }

    #[test]
    fn reads_the_forms() {
        assert_eq!(parse("time in dubai"), Some(TimeQuery::Now(zone_of("dubai"))));
        assert_eq!(parse("Sydney time"), Some(TimeQuery::Now(zone_of("sydney"))));
        assert_eq!(parse("what time is it in kathmandu?"), Some(TimeQuery::Now(zone_of("nepal"))));
        assert_eq!(
            parse("3pm ist to sydney"),
            Some(TimeQuery::At { hour: 15, minute: 0, from: zone_of("india"), to: zone_of("sydney") })
        );
        assert_eq!(parse("9:30 am dubai"), Some(TimeQuery::At { hour: 9, minute: 30, from: zone_of("dubai"), to: LOCAL }));
        assert_eq!(parse("15:30 in utc"), Some(TimeQuery::At { hour: 15, minute: 30, from: LOCAL, to: zone_of("utc") }));
        assert_eq!(parse("12am new york"), Some(TimeQuery::At { hour: 0, minute: 0, from: zone_of("nyc"), to: LOCAL }));
        assert_eq!(parse("noon london to dubai"), Some(TimeQuery::At { hour: 12, minute: 0, from: zone_of("uk"), to: zone_of("dubai") }));
    }

    #[test]
    fn leaves_other_text_alone() {
        for q in ["3", "3pm", "dubai", "time", "time in narnia", "13pm dubai", "3 dubai", "1password", "10:61 utc", "5 km to mi", "3pm here"] {
            assert_eq!(parse(q), None, "{q}");
        }
    }

    #[test]
    fn works_out_the_time() {
        // 2026-11-22 04:30 UTC is 10:00 in India, 08:30 in Dubai.
        let now = 1_795_321_800;
        let dubai = answer(&parse("time in dubai").unwrap(), now, &fixed).unwrap();
        assert_eq!(dubai.title, "8:30 AM");
        assert_eq!(dubai.note, "Sunday 22 Nov · UTC+4 · 1 h 30 min behind here");
        let nepal = answer(&parse("kathmandu time").unwrap(), now, &fixed).unwrap();
        assert_eq!(nepal.note, "Sunday 22 Nov · UTC+5:45 · 15 min ahead of here");

        let sydney = answer(&parse("11pm india to sydney").unwrap(), now, &fixed).unwrap();
        assert_eq!(sydney.title, "4:30 AM");
        assert_eq!(sydney.expression, "11:00 PM India → Sydney");
        assert_eq!(sydney.note, "Monday 23 Nov · UTC+11 · next day");

        let back = answer(&parse("6am dubai").unwrap(), now, &fixed).unwrap();
        assert_eq!(back.title, "7:30 AM");
        assert_eq!(back.note, "Sunday 22 Nov · UTC+5:30 · same day");
    }
}
