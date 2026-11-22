//! Unit conversion: `5 km to mi`, `70kg in lb`, `100 f to c`, `6 ft`.
//! Pure: no I/O. A bare amount (`6 ft`) converts to its usual counterpart.
//!
//! `in` is both the inch and the word "in": a unit written straight after the
//! number is always the unit (`5 in to cm`, `5 in cm`), and "in" anywhere
//! else separates the two units (`5 cm in in`).

#[derive(Clone, Copy, Debug, PartialEq)]
enum Dim {
    Length,
    Mass,
    Volume,
    Area,
    Speed,
    Data,
    Temperature,
}

#[derive(Debug)]
struct Unit {
    /// What the answer shows.
    symbol: &'static str,
    dim: Dim,
    /// Size in the dimension's base unit (m, kg, l, m², km/h, byte).
    /// Temperatures use `to_kelvin` instead.
    factor: f64,
    /// The usual counterpart for a bare amount.
    pair: Option<&'static str>,
}

const fn u(symbol: &'static str, dim: Dim, factor: f64, pair: Option<&'static str>) -> Unit {
    Unit { symbol, dim, factor, pair }
}

const UNITS: &[(&[&str], Unit)] = &[
    (&["mm", "millimeter", "millimeters", "millimetre", "millimetres"], u("mm", Dim::Length, 0.001, Some("in"))),
    (&["cm", "centimeter", "centimeters", "centimetre", "centimetres"], u("cm", Dim::Length, 0.01, Some("in"))),
    (&["m", "meter", "meters", "metre", "metres"], u("m", Dim::Length, 1.0, Some("ft"))),
    (&["km", "kilometer", "kilometers", "kilometre", "kilometres", "kms"], u("km", Dim::Length, 1000.0, Some("mi"))),
    (&["in", "inch", "inches", "\""], u("in", Dim::Length, 0.0254, Some("cm"))),
    (&["ft", "foot", "feet", "'"], u("ft", Dim::Length, 0.3048, Some("m"))),
    (&["yd", "yard", "yards"], u("yd", Dim::Length, 0.9144, Some("m"))),
    (&["mi", "mile", "miles"], u("mi", Dim::Length, 1609.344, Some("km"))),
    (&["nmi", "nautical mile", "nautical miles"], u("nmi", Dim::Length, 1852.0, Some("km"))),
    (&["mg", "milligram", "milligrams"], u("mg", Dim::Mass, 1e-6, None)),
    (&["g", "gram", "grams", "gm", "gms"], u("g", Dim::Mass, 0.001, Some("oz"))),
    (&["kg", "kilo", "kilos", "kilogram", "kilograms", "kgs"], u("kg", Dim::Mass, 1.0, Some("lb"))),
    (&["t", "tonne", "tonnes", "metric ton", "metric tons"], u("t", Dim::Mass, 1000.0, None)),
    (&["oz", "ounce", "ounces"], u("oz", Dim::Mass, 0.028_349_523_125, Some("g"))),
    (&["lb", "lbs"], u("lb", Dim::Mass, 0.453_592_37, Some("kg"))),
    (&["st", "stone", "stones"], u("st", Dim::Mass, 6.350_293_18, Some("kg"))),
    (&["ml", "milliliter", "milliliters", "millilitre", "millilitres"], u("ml", Dim::Volume, 0.001, Some("fl oz"))),
    (&["l", "liter", "liters", "litre", "litres", "ltr"], u("l", Dim::Volume, 1.0, Some("gal"))),
    (&["gal", "gallon", "gallons"], u("gal", Dim::Volume, 3.785_411_784, Some("l"))),
    (&["qt", "quart", "quarts"], u("qt", Dim::Volume, 0.946_352_946, Some("l"))),
    (&["pt", "pint", "pints"], u("pt", Dim::Volume, 0.473_176_473, Some("ml"))),
    (&["cup", "cups"], u("cup", Dim::Volume, 0.236_588_236_5, Some("ml"))),
    (&["fl oz", "floz", "fluid ounce", "fluid ounces"], u("fl oz", Dim::Volume, 0.029_573_529_562_5, Some("ml"))),
    (&["tbsp", "tablespoon", "tablespoons"], u("tbsp", Dim::Volume, 0.014_786_764_781_25, Some("ml"))),
    (&["tsp", "teaspoon", "teaspoons"], u("tsp", Dim::Volume, 0.004_928_921_593_75, Some("ml"))),
    (&["sqm", "sq m", "m2", "m²", "square meter", "square meters", "square metre", "square metres"], u("m²", Dim::Area, 1.0, Some("sq ft"))),
    (&["sqft", "sq ft", "ft2", "ft²", "square foot", "square feet"], u("sq ft", Dim::Area, 0.092_903_04, Some("m²"))),
    (&["sqkm", "sq km", "km2", "km²", "square kilometer", "square kilometers"], u("km²", Dim::Area, 1e6, Some("sq mi"))),
    (&["sqmi", "sq mi", "mi2", "square mile", "square miles"], u("sq mi", Dim::Area, 2_589_988.110_336, Some("km²"))),
    (&["acre", "acres", "ac"], u("acre", Dim::Area, 4_046.856_422_4, Some("ha"))),
    (&["ha", "hectare", "hectares"], u("ha", Dim::Area, 10_000.0, Some("acre"))),
    (&["kmh", "km/h", "kph", "kmph"], u("km/h", Dim::Speed, 1.0, Some("mph"))),
    (&["mph"], u("mph", Dim::Speed, 1.609_344, Some("km/h"))),
    (&["m/s", "mps"], u("m/s", Dim::Speed, 3.6, Some("km/h"))),
    (&["kn", "knot", "knots", "kt"], u("kn", Dim::Speed, 1.852, Some("km/h"))),
    (&["b", "byte", "bytes"], u("B", Dim::Data, 1.0, None)),
    (&["kb", "kilobyte", "kilobytes"], u("KB", Dim::Data, 1e3, None)),
    (&["mb", "megabyte", "megabytes"], u("MB", Dim::Data, 1e6, None)),
    (&["gb", "gigabyte", "gigabytes"], u("GB", Dim::Data, 1e9, None)),
    (&["tb", "terabyte", "terabytes"], u("TB", Dim::Data, 1e12, None)),
    (&["kib"], u("KiB", Dim::Data, 1024.0, None)),
    (&["mib"], u("MiB", Dim::Data, 1_048_576.0, None)),
    (&["gib"], u("GiB", Dim::Data, 1_073_741_824.0, None)),
    (&["tib"], u("TiB", Dim::Data, 1_099_511_627_776.0, None)),
    (&["c", "°c", "℃", "celsius", "centigrade"], u("°C", Dim::Temperature, 0.0, Some("°F"))),
    (&["f", "°f", "℉", "fahrenheit"], u("°F", Dim::Temperature, 0.0, Some("°C"))),
    (&["kelvin"], u("K", Dim::Temperature, 0.0, Some("°C"))),
];

fn unit(word: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|(names, _)| names.contains(&word)).map(|(_, unit)| unit)
}

fn by_symbol(symbol: &str) -> Option<&'static Unit> {
    UNITS.iter().map(|(_, unit)| unit).find(|unit| unit.symbol == symbol)
}

fn to_kelvin(symbol: &str, v: f64) -> f64 {
    match symbol {
        "°C" => v + 273.15,
        "°F" => (v - 32.0) * 5.0 / 9.0 + 273.15,
        _ => v,
    }
}

fn from_kelvin(symbol: &str, k: f64) -> f64 {
    match symbol {
        "°C" => k - 273.15,
        "°F" => (k - 273.15) * 9.0 / 5.0 + 32.0,
        _ => k,
    }
}

/// A conversion the query asks for, already worked out.
#[derive(Clone, Debug, PartialEq)]
pub struct Conversion {
    pub amount: f64,
    pub from: &'static str,
    pub value: f64,
    pub to: &'static str,
}

/// Split the number off the front: `-40`, `5.5`, `1,200`.
fn number(s: &str) -> Option<(f64, &str)> {
    let end = s
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || c == ',' || (i == 0 && (c == '-' || c == '−'))))
        .map_or(s.len(), |(i, _)| i);
    let text: String = s[..end].chars().filter(|&c| c != ',').map(|c| if c == '−' { '-' } else { c }).collect();
    if !text.chars().any(|c| c.is_ascii_digit()) || text.matches('.').count() > 1 {
        return None;
    }
    Some((text.parse().ok()?, s[end..].trim_start()))
}

/// The longest unit name at the start of `s` that ends at a word boundary.
fn leading_unit(s: &str) -> Option<(&'static Unit, &str)> {
    let mut best: Option<(&'static Unit, usize)> = None;
    for (names, unit) in UNITS {
        for name in *names {
            if s.starts_with(name) && best.is_none_or(|(_, len)| name.len() > len) {
                let rest = &s[name.len()..];
                let boundary = rest.is_empty() || rest.starts_with(' ') || !name.chars().last().is_some_and(char::is_alphanumeric);
                if boundary {
                    best = Some((unit, name.len()));
                }
            }
        }
    }
    best.map(|(unit, len)| (unit, s[len..].trim_start()))
}

pub fn parse(input: &str) -> Option<Conversion> {
    let lower = input.trim().to_lowercase();
    let (amount, rest) = number(&lower)?;
    let (from, rest) = leading_unit(rest)?;
    let rest = rest
        .strip_prefix("to ")
        .or_else(|| rest.strip_prefix("in "))
        .or_else(|| rest.strip_prefix("as "))
        .or_else(|| rest.strip_prefix("into "))
        .unwrap_or(rest)
        .trim();
    let to = if rest.is_empty() {
        by_symbol(from.pair?)?
    } else {
        unit(rest)?
    };
    if to.dim != from.dim || std::ptr::eq(to, from) {
        return None;
    }
    let value = if from.dim == Dim::Temperature {
        from_kelvin(to.symbol, to_kelvin(from.symbol, amount))
    } else {
        amount * from.factor / to.factor
    };
    // `-5 km` is not a distance; a temperature can be negative.
    if amount < 0.0 && from.dim != Dim::Temperature {
        return None;
    }
    Some(Conversion { amount, from: from.symbol, value, to: to.symbol })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(q: &str) -> Option<(f64, &'static str)> {
        parse(q).map(|c| ((c.value * 1000.0).round() / 1000.0, c.to))
    }

    #[test]
    fn converts_between_units() {
        assert_eq!(conv("5 km to mi"), Some((3.107, "mi")));
        assert_eq!(conv("5km in miles"), Some((3.107, "mi")));
        assert_eq!(conv("70 kg to lb"), Some((154.324, "lb")));
        assert_eq!(conv("100 f to c"), Some((37.778, "°C")));
        assert_eq!(conv("-40 c to f"), Some((-40.0, "°F")));
        assert_eq!(conv("1,200 sq ft to sqm"), Some((111.484, "m²")));
        assert_eq!(conv("1 gib to mb"), Some((1073.742, "MB")));
        assert_eq!(conv("2 cups to ml"), Some((473.176, "ml")));
    }

    #[test]
    fn a_bare_amount_takes_its_counterpart() {
        assert_eq!(conv("6 ft"), Some((1.829, "m")));
        assert_eq!(conv("30 c"), Some((86.0, "°F")));
        assert_eq!(conv("100 kmh"), Some((62.137, "mph")));
    }

    #[test]
    fn in_is_the_inch_only_after_the_number() {
        assert_eq!(conv("12 in to cm"), Some((30.48, "cm")));
        assert_eq!(conv("12 in cm"), Some((30.48, "cm")));
        assert_eq!(conv("30 cm in in"), Some((11.811, "in")));
        assert_eq!(conv("5 in"), Some((12.7, "cm")));
    }

    #[test]
    fn leaves_other_text_alone() {
        for q in ["5", "km", "1password", "2 apples", "5 km to kg", "5 kg to kg", "-5 km", "3 mb", "100 usd to inr", "1.2.3 m"] {
            assert_eq!(parse(q), None, "{q}");
        }
    }
}
