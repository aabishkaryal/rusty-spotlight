//! Calculator and currency parsing (FR-22, FR-32). Pure: no rates, no I/O.
//!
//! A query counts as a calculation only if the whole string parses and it
//! contains an operator or a currency. `1Password`, `2.0` and dates do not.

/// What the typed text asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Query {
    /// Plain arithmetic, already evaluated.
    Calc(f64),
    /// Convert `amount` of `from` into `to` (`None`: the configured default).
    Convert { amount: f64, from: String, to: Option<String> },
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Op(char), // + - * / ^ ( ) %
    Of,
    To,
    Cur(String),
}

/// Currency words and symbols. Ambiguous ones (`$`, `rs`, `rupees`) take the
/// defaults from the requirements: USD and INR.
fn currency(word: &str) -> Option<&'static str> {
    Some(match word {
        "usd" | "$" | "us$" | "dollar" | "dollars" => "USD",
        "aed" | "dirham" | "dirhams" | "dhs" | "dh" => "AED",
        "aud" | "a$" | "au$" => "AUD",
        "npr" | "nrs" => "NPR",
        "inr" | "₹" | "rs" | "rupee" | "rupees" => "INR",
        "eur" | "€" | "euro" | "euros" => "EUR",
        "gbp" | "£" | "pound" | "pounds" => "GBP",
        "jpy" | "¥" | "yen" => "JPY",
        "cad" => "CAD",
        "sgd" => "SGD",
        "cny" => "CNY",
        "chf" => "CHF",
        "sar" => "SAR",
        "qar" => "QAR",
        "myr" => "MYR",
        "thb" => "THB",
        "bdt" => "BDT",
        "pkr" => "PKR",
        "lkr" => "LKR",
        _ => return None,
    })
}

fn lex(input: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = input.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let mut text = String::new();
            let mut dots = 0;
            while i < chars.len() {
                let d = chars[i];
                if d.is_ascii_digit() {
                    text.push(d);
                } else if d == '.' {
                    dots += 1;
                    text.push(d);
                } else if d == ',' && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()) {
                    // thousands separator
                } else {
                    break;
                }
                i += 1;
            }
            if dots > 1 {
                return None; // a version number such as 1.2.3
            }
            toks.push(Tok::Num(text.parse().ok()?));
        } else if c.is_alphabetic() {
            let mut word = String::new();
            while i < chars.len() && chars[i].is_alphabetic() {
                word.extend(chars[i].to_lowercase());
                i += 1;
            }
            if i < chars.len() && chars[i] == '$' && matches!(word.as_str(), "us" | "a" | "au") {
                word.push('$');
                i += 1;
            }
            toks.push(match word.as_str() {
                "of" => Tok::Of,
                "to" | "in" => Tok::To,
                "x" => Tok::Op('*'),
                w => Tok::Cur(currency(w)?.to_string()),
            });
        } else {
            i += 1;
            toks.push(match c {
                '+' | '-' | '*' | '/' | '^' | '(' | ')' | '%' => Tok::Op(c),
                '×' => Tok::Op('*'),
                '÷' => Tok::Op('/'),
                '−' => Tok::Op('-'),
                '$' | '₹' | '€' | '£' | '¥' => Tok::Cur(currency(&c.to_string())?.to_string()),
                _ => return None,
            });
        }
    }
    Some(toks)
}

#[derive(Clone, Copy)]
struct Val {
    v: f64,
    /// A bare percentage such as `18%`, which `a + 18%` treats as a share of `a`.
    pct: bool,
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn eat(&mut self, op: char) -> bool {
        if self.peek() == Some(&Tok::Op(op)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Option<Val> {
        let mut left = self.term()?;
        loop {
            let sign = if self.eat('+') {
                1.0
            } else if self.eat('-') {
                -1.0
            } else {
                return Some(left);
            };
            let right = self.term()?;
            let delta = if right.pct { left.v * right.v } else { right.v };
            left = Val { v: left.v + sign * delta, pct: false };
        }
    }

    fn term(&mut self) -> Option<Val> {
        let mut left = self.power()?;
        loop {
            if self.eat('*') || self.peek() == Some(&Tok::Of) {
                if self.peek() == Some(&Tok::Of) {
                    self.pos += 1;
                }
                let right = self.power()?;
                left = Val { v: left.v * right.v, pct: false };
            } else if self.eat('/') {
                let right = self.power()?;
                if right.v == 0.0 {
                    return None;
                }
                left = Val { v: left.v / right.v, pct: false };
            } else {
                return Some(left);
            }
        }
    }

    fn power(&mut self) -> Option<Val> {
        let base = self.unary()?;
        if self.eat('^') {
            let exp = self.power()?;
            return Some(Val { v: base.v.powf(exp.v), pct: false });
        }
        Some(base)
    }

    fn unary(&mut self) -> Option<Val> {
        if self.eat('-') {
            let v = self.unary()?;
            return Some(Val { v: -v.v, pct: v.pct });
        }
        self.primary()
    }

    fn primary(&mut self) -> Option<Val> {
        let mut val = match self.peek()?.clone() {
            Tok::Num(n) => {
                self.pos += 1;
                Val { v: n, pct: false }
            }
            Tok::Op('(') => {
                self.pos += 1;
                let v = self.expr()?;
                if !self.eat(')') {
                    return None;
                }
                Val { v: v.v, pct: false }
            }
            _ => return None,
        };
        if self.eat('%') {
            val = Val { v: val.v / 100.0, pct: true };
        }
        Some(val)
    }
}

fn evaluate(toks: &[Tok]) -> Option<f64> {
    if toks.is_empty() {
        return None;
    }
    let mut parser = Parser { toks, pos: 0 };
    let val = parser.expr()?;
    (parser.pos == toks.len() && val.v.is_finite()).then_some(val.v)
}

/// `2026-11-21`, `21/11/2026`: three number groups joined by the same
/// separator and nothing else. These are dates, not sums.
fn looks_like_date(input: &str) -> bool {
    let s = input.trim();
    for sep in ['-', '/'] {
        let parts: Vec<&str> = s.split(sep).collect();
        if parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
            return true;
        }
    }
    false
}

pub fn parse(input: &str) -> Option<Query> {
    if looks_like_date(input) {
        return None;
    }
    let mut toks = lex(input)?;
    let has_currency = toks.iter().any(|t| matches!(t, Tok::Cur(_)));
    if !has_currency {
        let has_operator = toks.iter().any(|t| matches!(t, Tok::Op(_) | Tok::Of));
        return has_operator.then(|| evaluate(&toks)).flatten().map(Query::Calc);
    }

    // Peel the currencies off the ends; what is left must be the amount.
    let mut to = None;
    if toks.len() >= 2 && matches!(toks[toks.len() - 2], Tok::To) {
        let Some(Tok::Cur(c)) = toks.pop() else { return None };
        toks.pop();
        to = Some(c);
    }
    let from = if let Some(Tok::Cur(_)) = toks.last() {
        let Some(Tok::Cur(c)) = toks.pop() else { return None };
        if to.is_none() {
            // `100 usd inr`: two codes in a row mean from, then to.
            if let Some(Tok::Cur(_)) = toks.last() {
                let Some(Tok::Cur(first)) = toks.pop() else { return None };
                to = Some(c);
                first
            } else {
                c
            }
        } else {
            c
        }
    } else if let Some(Tok::Cur(_)) = toks.first() {
        let Tok::Cur(c) = toks.remove(0) else { return None };
        c
    } else {
        return None;
    };
    if toks.iter().any(|t| matches!(t, Tok::Cur(_) | Tok::To)) {
        return None;
    }
    let amount = evaluate(&toks)?;
    Some(Query::Convert { amount, from, to })
}

/// `1234567.5` → `1,234,567.5`. `decimals`: fixed places, or `None` for up to
/// ten significant digits with trailing zeros removed.
pub fn format_number(value: f64, decimals: Option<usize>) -> String {
    let text = match decimals {
        Some(d) => format!("{value:.d$}"),
        None => {
            let magnitude = if value == 0.0 { 0 } else { value.abs().log10().floor() as i32 };
            let places = (9 - magnitude).clamp(0, 10) as usize;
            let t = format!("{value:.places$}");
            if t.contains('.') { t.trim_end_matches('0').trim_end_matches('.').to_string() } else { t }
        }
    };
    let (sign, rest) = match text.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", text.as_str()),
    };
    let (int, frac) = match rest.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (rest, None),
    };
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    if sign == "-" && grouped == "0" && frac.is_none_or(|f| f.chars().all(|c| c == '0')) {
        return match frac {
            Some(f) => format!("0.{f}"),
            None => "0".into(),
        };
    }
    match frac {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc(s: &str) -> Option<f64> {
        match parse(s)? {
            Query::Calc(v) => Some(v),
            _ => None,
        }
    }

    fn close(a: Option<f64>, b: f64) {
        let a = a.expect("should be a calculation");
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn arithmetic_and_precedence() {
        close(calc("2+3*4"), 14.0);
        close(calc("(2+3)*4"), 20.0);
        close(calc("2^3^2"), 512.0);
        close(calc("-3 + 5"), 2.0);
        close(calc("10 / 4"), 2.5);
        close(calc("1,200 × 2"), 2400.0);
        close(calc("7 ÷ 2"), 3.5);
        close(calc("2400*1.18"), 2832.0);
    }

    #[test]
    fn percentages() {
        close(calc("2400 + 18%"), 2832.0);
        close(calc("2400 - 10%"), 2160.0);
        close(calc("15% of 2400"), 360.0);
        close(calc("200 * 5%"), 10.0);
        close(calc("50%"), 0.5);
    }

    #[test]
    fn things_that_are_not_calculations() {
        for s in ["1Password", "2.0", "42", "2026-11-21", "21/11/2026", "1.2.3", "terminal", "3 +", "(2+3", "a+b", "", "5 / 0"] {
            assert_eq!(parse(s), None, "{s:?} should not be a calculation");
        }
    }

    #[test]
    fn currency_forms() {
        let c = |amount, from: &str, to: Option<&str>| {
            Some(Query::Convert { amount, from: from.into(), to: to.map(Into::into) })
        };
        assert_eq!(parse("100 usd to inr"), c(100.0, "USD", Some("INR")));
        assert_eq!(parse("100 usd in inr"), c(100.0, "USD", Some("INR")));
        assert_eq!(parse("100 usd inr"), c(100.0, "USD", Some("INR")));
        assert_eq!(parse("100 usd"), c(100.0, "USD", None));
        assert_eq!(parse("$100"), c(100.0, "USD", None));
        assert_eq!(parse("100$"), c(100.0, "USD", None));
        assert_eq!(parse("rs100 to aed"), c(100.0, "INR", Some("AED")));
        assert_eq!(parse("₹ 2,500 to npr"), c(2500.0, "INR", Some("NPR")));
        assert_eq!(parse("A$50 to usd"), c(50.0, "AUD", Some("USD")));
        assert_eq!(parse("(120 + 80) aed to inr"), c(200.0, "AED", Some("INR")));
        assert_eq!(parse("500 dirhams to rupees"), c(500.0, "AED", Some("INR")));
        assert_eq!(parse("usd to inr"), None);
        assert_eq!(parse("100 usd to"), None);
        assert_eq!(parse("100 xyz"), None);
    }

    #[test]
    fn number_formatting() {
        assert_eq!(format_number(2832.0, None), "2,832");
        assert_eq!(format_number(1234567.5, None), "1,234,567.5");
        assert_eq!(format_number(9267.0, Some(2)), "9,267.00");
        assert_eq!(format_number(0.1 + 0.2, None), "0.3");
        assert_eq!(format_number(-1500.25, None), "-1,500.25");
        assert_eq!(format_number(1.0 / 3.0, None), "0.3333333333");
    }
}
