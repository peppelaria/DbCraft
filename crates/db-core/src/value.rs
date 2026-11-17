use std::fmt;

/// A database value. Types without a direct mapping (decimals, dates, UUIDs…) travel as `Text`
/// in the server's own textual form.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            Value::Text(t) => t.trim().parse().ok(),
            Value::Bool(b) => Some(i64::from(*b)),
            _ => None,
        }
    }
    pub fn is_numeric(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Float(_))
    }
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => (*b).into(),
            Value::Int(i) => (*i).into(),
            Value::Float(f) => serde_json::Number::from_f64(*f).map_or(serde_json::Value::Null, serde_json::Value::Number),
            Value::Text(t) => t.clone().into(),
            Value::Bytes(b) => hex(b).into(),
        }
    }
}

pub fn hex(b: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(b.len() * 2 + 2);
    s.push_str("0x");
    for x in b {
        let _ = write!(s, "{x:02X}");
    }
    s
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str("(NULL)"),
            Value::Bool(b) => write!(f, "{}", if *b { 1 } else { 0 }),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Text(t) => f.write_str(t),
            Value::Bytes(b) if b.len() > 64 => write!(f, "(BLOB) {} byte", b.len()),
            Value::Bytes(b) => f.write_str(&hex(b)),
        }
    }
}
