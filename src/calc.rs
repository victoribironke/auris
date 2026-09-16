//! A small, dependency-free expression evaluator.
//!
//! Supports `+ - * / % ^` (and `**`, `×`, `÷`), parentheses, unary signs,
//! scientific notation, the constants `pi`, `tau`, and `e`, and common functions.

#[derive(Clone, Debug, PartialEq)]
enum Token { Number(f64), Ident(String), Op(char), LParen, RParen, Comma }

pub fn evaluate(input: &str) -> Result<f64, String> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() { return Err("empty expression".into()); }
    let mut parser = Parser { tokens, pos: 0, depth: 0 };
    let value = parser.expression()?;
    if parser.pos != parser.tokens.len() { return Err("unexpected input".into()); }
    if value.is_finite() { Ok(value) } else { Err("result is not a finite number".into()) }
}

/// True when the input contains something that only makes sense as math,
/// so plain numbers and ordinary words are not treated as calculations.
pub fn looks_like_math(input: &str) -> bool {
    let has_digit_or_constant = input.chars().any(|c| c.is_ascii_digit())
        || ["pi", "tau"].iter().any(|name| input.to_ascii_lowercase().contains(name));
    let has_operator = input.chars().any(|c| matches!(c, '+' | '-' | '*' | '/' | '%' | '^' | '(' | '×' | '÷'));
    has_digit_or_constant && has_operator
}

pub fn format_number(value: f64) -> String {
    if value == 0.0 { return "0".into(); }
    let magnitude = value.abs();
    if !(1e-9..1e15).contains(&magnitude) { return format!("{value:e}"); }
    let mut text = format!("{value:.10}");
    if text.contains('.') {
        while text.ends_with('0') { text.pop(); }
        if text.ends_with('.') { text.pop(); }
    }
    if text == "-0" { "0".into() } else { text }
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() { i += 1; continue; }
        if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_') { i += 1; }
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let sign = usize::from(i + 1 < chars.len() && matches!(chars[i + 1], '+' | '-'));
                if chars.get(i + 1 + sign).is_some_and(|d| d.is_ascii_digit()) {
                    i += 1 + sign;
                    while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                }
            }
            let text: String = chars[start..i].iter().filter(|c| **c != '_').collect();
            let number = text.parse::<f64>().map_err(|_| format!("invalid number '{text}'"))?;
            tokens.push(Token::Number(number));
            continue;
        }
        if c.is_alphabetic() {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric()) { i += 1; }
            tokens.push(Token::Ident(chars[start..i].iter().collect::<String>().to_lowercase()));
            continue;
        }
        let token = match c {
            '*' if chars.get(i + 1) == Some(&'*') => { i += 1; Token::Op('^') }
            '+' | '-' | '*' | '/' | '%' | '^' => Token::Op(c),
            '×' => Token::Op('*'),
            '÷' => Token::Op('/'),
            '(' => Token::LParen,
            ')' => Token::RParen,
            ',' => Token::Comma,
            _ => return Err(format!("unexpected character '{c}'")),
        };
        tokens.push(token);
        i += 1;
    }
    Ok(tokens)
}

struct Parser { tokens: Vec<Token>, pos: usize, depth: usize }

const MAX_DEPTH: usize = 128;

impl Parser {
    fn peek(&self) -> Option<&Token> { self.tokens.get(self.pos) }

    fn eat_op(&mut self, ops: &[char]) -> Option<char> {
        match self.peek() {
            Some(Token::Op(op)) if ops.contains(op) => { let op = *op; self.pos += 1; Some(op) }
            _ => None,
        }
    }

    fn expect(&mut self, token: Token) -> Result<(), String> {
        if self.peek() == Some(&token) { self.pos += 1; Ok(()) } else { Err(format!("expected {token:?}")) }
    }

    fn expression(&mut self) -> Result<f64, String> {
        let mut value = self.term()?;
        while let Some(op) = self.eat_op(&['+', '-']) {
            let rhs = self.term()?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Ok(value)
    }

    fn term(&mut self) -> Result<f64, String> {
        let mut value = self.unary()?;
        while let Some(op) = self.eat_op(&['*', '/', '%']) {
            let rhs = self.unary()?;
            value = match op {
                '*' => value * rhs,
                _ if rhs == 0.0 => return Err("division by zero".into()),
                '/' => value / rhs,
                _ => value % rhs,
            };
        }
        Ok(value)
    }

    fn unary(&mut self) -> Result<f64, String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH { return Err("expression is nested too deeply".into()); }
        let result = match self.eat_op(&['+', '-']) {
            Some('-') => self.unary().map(|value| -value),
            Some(_) => self.unary(),
            None => self.power(),
        };
        self.depth -= 1;
        result
    }

    fn power(&mut self) -> Result<f64, String> {
        let base = self.primary()?;
        if self.eat_op(&['^']).is_some() {
            let exponent = self.unary()?;
            return Ok(base.powf(exponent));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<f64, String> {
        match self.tokens.get(self.pos).cloned() {
            Some(Token::Number(value)) => { self.pos += 1; Ok(value) }
            Some(Token::LParen) => {
                self.pos += 1;
                let value = self.expression()?;
                self.expect(Token::RParen)?;
                Ok(value)
            }
            Some(Token::Ident(name)) => {
                self.pos += 1;
                if self.peek() == Some(&Token::LParen) {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        args.push(self.expression()?);
                        while self.peek() == Some(&Token::Comma) { self.pos += 1; args.push(self.expression()?); }
                    }
                    self.expect(Token::RParen)?;
                    call(&name, &args)
                } else {
                    constant(&name)
                }
            }
            Some(token) => Err(format!("unexpected {token:?}")),
            None => Err("unexpected end of expression".into()),
        }
    }
}

fn constant(name: &str) -> Result<f64, String> {
    match name {
        "pi" | "π" => Ok(std::f64::consts::PI),
        "tau" => Ok(std::f64::consts::TAU),
        "e" => Ok(std::f64::consts::E),
        _ => Err(format!("unknown name '{name}'")),
    }
}

fn call(name: &str, args: &[f64]) -> Result<f64, String> {
    let one = || match args { [x] => Ok(*x), _ => Err(format!("{name} takes one argument")) };
    let value = match name {
        "sqrt" => { let x = one()?; if x < 0.0 { return Err("square root of a negative number".into()); } x.sqrt() }
        "cbrt" => one()?.cbrt(),
        "abs" => one()?.abs(),
        "sin" => one()?.sin(),
        "cos" => one()?.cos(),
        "tan" => one()?.tan(),
        "asin" => one()?.asin(),
        "acos" => one()?.acos(),
        "atan" => one()?.atan(),
        "exp" => one()?.exp(),
        "ln" => one()?.ln(),
        "log2" => one()?.log2(),
        "log" => match args { [x] => x.log10(), [x, base] => x.log(*base), _ => return Err("log takes one or two arguments".into()) },
        "floor" => one()?.floor(),
        "ceil" => one()?.ceil(),
        "round" => one()?.round(),
        "rad" => one()?.to_radians(),
        "deg" => one()?.to_degrees(),
        "pow" => match args { [x, y] => x.powf(*y), _ => return Err("pow takes two arguments".into()) },
        "min" if !args.is_empty() => args.iter().copied().fold(f64::INFINITY, f64::min),
        "max" if !args.is_empty() => args.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        _ => return Err(format!("unknown function '{name}'")),
    };
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(input: &str) -> f64 { evaluate(input).unwrap() }

    #[test]
    fn precedence_and_parentheses() {
        assert_eq!(eval("2 + 3 * 4"), 14.0);
        assert_eq!(eval("(2 + 3) * 4"), 20.0);
        assert_eq!(eval("10 - 4 - 3"), 3.0);
        assert_eq!(eval("45 * 12"), 540.0);
    }

    #[test]
    fn unary_and_power() {
        assert_eq!(eval("-5 + 3"), -2.0);
        assert_eq!(eval("-2^2"), -4.0);
        assert_eq!(eval("2^3^2"), 512.0);
        assert_eq!(eval("2 ** 10"), 1024.0);
        assert_eq!(eval("2^-1"), 0.5);
    }

    #[test]
    fn functions_and_constants() {
        assert_eq!(eval("sqrt(16) + abs(-2)"), 6.0);
        assert_eq!(eval("max(1, 7, 3)"), 7.0);
        assert!((eval("2 * pi") - std::f64::consts::TAU).abs() < 1e-12);
        assert_eq!(eval("log(1000)"), 3.0);
        assert_eq!(eval("1.5e3 / 3"), 500.0);
        assert_eq!(eval("1_000 × 2"), 2000.0);
    }

    #[test]
    fn errors() {
        assert!(evaluate("1 / 0").is_err());
        assert!(evaluate("2 +").is_err());
        assert!(evaluate("(1 + 2").is_err());
        assert!(evaluate("foo(2)").is_err());
        assert!(evaluate("hello").is_err());
    }

    #[test]
    fn detection() {
        assert!(looks_like_math("45 * 12"));
        assert!(looks_like_math("sqrt(2)"));
        assert!(!looks_like_math("2024"));
        assert!(!looks_like_math("visual studio"));
    }

    #[test]
    fn formatting() {
        assert_eq!(format_number(0.1 + 0.2), "0.3");
        assert_eq!(format_number(540.0), "540");
        assert_eq!(format_number(-2.5), "-2.5");
        assert_eq!(format_number(1e20), "1e20");
    }
}
