use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::runtime::value::*;
use crate::interpreter::evaluator::RuntimeError;

pub fn make_json_module() -> Value {
    let mut attrs = HashMap::new();

    macro_rules! bf {
        ($name:expr, $f:expr) => {
            attrs.insert($name.into(), Value::BuiltinFunction(Arc::new(BuiltinFn { name: $name, func: $f })));
        }
    }

    bf!("dumps", |args| {
        match args.first() {
            Some(v) => Ok(Value::Str(Arc::new(value_to_json(v, 0, None)?))),
            None    => Err(RuntimeError::type_error("dumps() requires an argument")),
        }
    });

    bf!("loads", |args| {
        match args.first() {
            Some(Value::Str(s)) => json_to_value(s),
            _ => Err(RuntimeError::type_error("loads() requires a string")),
        }
    });

    bf!("dump", |args| {
        let val  = match args.first() { Some(v) => v, None => return Err(RuntimeError::type_error("dump() requires value and file")) };
        let file = match args.get(1)  { Some(v) => v, None => return Err(RuntimeError::type_error("dump() requires a file object")) };
        let json = value_to_json(val, 0, None)?;
        call_file_method(file, "write", vec![Value::Str(Arc::new(json))])?;
        Ok(Value::None)
    });

    bf!("load", |args| {
        let file = match args.first() { Some(v) => v, None => return Err(RuntimeError::type_error("load() requires a file object")) };
        match call_file_method(file, "read", vec![])? {
            Value::Str(s) => json_to_value(&s),
            _ => Err(RuntimeError::new("ValueError", "file.read() did not return a string")),
        }
    });

    Value::Module(Arc::new(Mutex::new(ModuleObj { name: "json".into(), attrs })))
}

// ── Helper: call a method stored as BoundBuiltin in an instance ───────────────

fn call_file_method(inst: &Value, method: &str, mut extra_args: Vec<Value>) -> Result<Value, RuntimeError> {
    match inst {
        Value::Instance(h) => {
            let fn_val = h.lock().unwrap().fields.get(method).cloned();
            match fn_val {
                Some(Value::BoundBuiltin { receiver, func, .. }) => {
                    let mut args = vec![*receiver];
                    args.append(&mut extra_args);
                    func(args)
                }
                Some(Value::BuiltinFunction(bf)) => (bf.func)(extra_args),
                _ => Err(RuntimeError::new("AttributeError", &format!("file has no '{}' method", method))),
            }
        }
        _ => Err(RuntimeError::type_error("expected a file object")),
    }
}

// ── Serializer ────────────────────────────────────────────────────────────────

fn value_to_json(v: &Value, depth: usize, indent: Option<usize>) -> Result<String, RuntimeError> {
    if depth > 200 {
        return Err(RuntimeError::new("ValueError", "json: circular reference / nesting too deep"));
    }
    match v {
        Value::None         => Ok("null".into()),
        Value::Bool(true)   => Ok("true".into()),
        Value::Bool(false)  => Ok("false".into()),
        Value::Int(n)       => Ok(n.to_string()),
        Value::Float(f)     => {
            if f.is_nan() || f.is_infinite() {
                Err(RuntimeError::new("ValueError", "json: out of range float (NaN/Inf)"))
            } else {
                Ok(format!("{}", f))
            }
        }
        Value::Str(s)       => Ok(json_escape(s)),
        Value::List(l)      => {
            let items = l.lock().unwrap().iter()
                .map(|x| value_to_json(x, depth + 1, indent))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", items.join(",")))
        }
        Value::Tuple(t)     => {
            let items = t.iter()
                .map(|x| value_to_json(x, depth + 1, indent))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", items.join(",")))
        }
        Value::Dict(d)      => {
            let map = d.lock().unwrap();
            let mut pairs = Vec::new();
            for (k, val) in map.iter() {
                let key = match k {
                    HashableValue::Str(s) => json_escape(s),
                    HashableValue::Int(n) => format!("\"{}\"", n),
                    HashableValue::Bool(b) => format!("\"{}\"", if *b { "true" } else { "false" }),
                    HashableValue::None    => "\"null\"".into(),
                    _ => return Err(RuntimeError::new("TypeError", "json: dict keys must be strings")),
                };
                let v_str = value_to_json(val, depth + 1, indent)?;
                pairs.push(format!("{}:{}", key, v_str));
            }
            Ok(format!("{{{}}}", pairs.join(",")))
        }
        _ => Err(RuntimeError::new("TypeError", &format!("json: object of type '{}' is not JSON serializable", v.type_name()))),
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"'  => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c    => out.push(c),
        }
    }
    out.push('"');
    out
}

// ── Deserializer ──────────────────────────────────────────────────────────────

fn json_to_value(s: &str) -> Result<Value, RuntimeError> {
    let mut parser = JsonParser { src: s.as_bytes(), pos: 0 };
    let v = parser.parse_value()?;
    parser.skip_ws();
    if parser.pos < parser.src.len() {
        return Err(RuntimeError::new("json.JSONDecodeError", "Extra data after JSON value"));
    }
    Ok(v)
}

struct JsonParser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn peek(&self) -> Option<u8> { self.src.get(self.pos).copied() }
    fn advance(&mut self) { self.pos += 1; }
    fn err(&self, msg: &str) -> RuntimeError {
        RuntimeError::new("json.JSONDecodeError", &format!("{} at position {}", msg, self.pos))
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) { self.advance(); }
    }

    fn parse_value(&mut self) -> Result<Value, RuntimeError> {
        self.skip_ws();
        match self.peek() {
            Some(b'"')  => self.parse_string().map(|s| Value::Str(Arc::new(s))),
            Some(b'{')  => self.parse_object(),
            Some(b'[')  => self.parse_array(),
            Some(b't')  => { self.expect(b"true")?;  Ok(Value::Bool(true))  }
            Some(b'f')  => { self.expect(b"false")?; Ok(Value::Bool(false)) }
            Some(b'n')  => { self.expect(b"null")?;  Ok(Value::None)        }
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(c)     => Err(self.err(&format!("unexpected character '{}'", c as char))),
            None        => Err(self.err("unexpected end of input")),
        }
    }

    fn expect(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        for &b in bytes {
            if self.peek() != Some(b) { return Err(self.err("invalid literal")); }
            self.advance();
        }
        Ok(())
    }

    fn parse_string(&mut self) -> Result<String, RuntimeError> {
        self.advance(); // consume opening '"'
        let mut s = String::new();
        loop {
            match self.peek() {
                None       => return Err(self.err("unterminated string")),
                Some(b'"') => { self.advance(); return Ok(s); }
                Some(b'\\') => {
                    self.advance();
                    match self.peek() {
                        Some(b'"')  => { s.push('"');  self.advance(); }
                        Some(b'\\') => { s.push('\\'); self.advance(); }
                        Some(b'/')  => { s.push('/');  self.advance(); }
                        Some(b'n')  => { s.push('\n'); self.advance(); }
                        Some(b'r')  => { s.push('\r'); self.advance(); }
                        Some(b't')  => { s.push('\t'); self.advance(); }
                        Some(b'b')  => { s.push('\x08'); self.advance(); }
                        Some(b'f')  => { s.push('\x0C'); self.advance(); }
                        Some(b'u')  => {
                            self.advance();
                            let mut hex = String::new();
                            for _ in 0..4 {
                                match self.peek() {
                                    Some(c) => { hex.push(c as char); self.advance(); }
                                    None    => return Err(self.err("truncated \\uXXXX")),
                                }
                            }
                            let cp = u32::from_str_radix(&hex, 16)
                                .ok()
                                .and_then(char::from_u32)
                                .ok_or_else(|| self.err("invalid \\uXXXX"))?;
                            s.push(cp);
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                }
                Some(c) => { s.push(c as char); self.advance(); }
            }
        }
    }

    fn parse_number(&mut self) -> Result<Value, RuntimeError> {
        let start = self.pos;
        if self.peek() == Some(b'-') { self.advance(); }
        while matches!(self.peek(), Some(b'0'..=b'9')) { self.advance(); }
        let is_float = matches!(self.peek(), Some(b'.' | b'e' | b'E'));
        if is_float {
            if self.peek() == Some(b'.') {
                self.advance();
                while matches!(self.peek(), Some(b'0'..=b'9')) { self.advance(); }
            }
            if matches!(self.peek(), Some(b'e' | b'E')) {
                self.advance();
                if matches!(self.peek(), Some(b'+' | b'-')) { self.advance(); }
                while matches!(self.peek(), Some(b'0'..=b'9')) { self.advance(); }
            }
            let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
            s.parse::<f64>().map(Value::Float).map_err(|_| self.err("invalid float"))
        } else {
            let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
            s.parse::<i64>().map(Value::Int).map_err(|_| self.err("integer overflow"))
        }
    }

    fn parse_array(&mut self) -> Result<Value, RuntimeError> {
        self.advance(); // consume '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') { self.advance(); return Ok(Value::List(crate::runtime::gc::alloc_list(items))); }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => { self.advance(); }
                Some(b']') => { self.advance(); break; }
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
        Ok(Value::List(crate::runtime::gc::alloc_list(items)))
    }

    fn parse_object(&mut self) -> Result<Value, RuntimeError> {
        self.advance(); // consume '{'
        let mut map: HashMap<HashableValue, Value> = HashMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') { self.advance(); return Ok(Value::Dict(crate::runtime::gc::alloc_dict(map))); }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') { return Err(self.err("expected string key")); }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') { return Err(self.err("expected ':'")); }
            self.advance();
            let val = self.parse_value()?;
            map.insert(HashableValue::Str(key), val);
            self.skip_ws();
            match self.peek() {
                Some(b',') => { self.advance(); }
                Some(b'}') => { self.advance(); break; }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
        Ok(Value::Dict(crate::runtime::gc::alloc_dict(map)))
    }
}
