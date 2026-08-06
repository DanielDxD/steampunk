//! Compile-time schema driven encode/decode for JSON / YAML / TOML / TOON.
//! Schema grammar (no spaces):
//!   i | f | s | b | o(SCHEMA) | L(SCHEMA) | CSIZE(wire:SCHEMA:off,...)

use crate::runtime::{
    cstr_to_string, make_tagged, stk_alloc, stk_list_get, stk_list_len, stk_list_new, stk_list_push,
    string_to_cstr,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
enum Schema {
    Int,
    Float,
    String,
    Bool,
    Option(Box<Schema>),
    List(Box<Schema>),
    Class {
        size: i64,
        fields: Vec<(String, Schema, i64)>,
    },
}

fn parse_schema(s: &str) -> Result<Schema, String> {
    let (sch, rest) = parse_schema_at(s)?;
    if !rest.is_empty() {
        return Err(format!("trailing schema junk: {rest}"));
    }
    Ok(sch)
}

fn parse_schema_at(s: &str) -> Result<(Schema, &str), String> {
    let s = s.trim_start();
    if s.is_empty() {
        return Err("empty schema".into());
    }
    match s.as_bytes()[0] {
        b'i' => Ok((Schema::Int, &s[1..])),
        b'f' => Ok((Schema::Float, &s[1..])),
        b's' => Ok((Schema::String, &s[1..])),
        b'b' => Ok((Schema::Bool, &s[1..])),
        b'o' => {
            let rest = expect(s, "o(")?;
            let (inner, rest) = parse_schema_at(rest)?;
            let rest = expect(rest, ")")?;
            Ok((Schema::Option(Box::new(inner)), rest))
        }
        b'L' => {
            let rest = expect(s, "L(")?;
            let (inner, rest) = parse_schema_at(rest)?;
            let rest = expect(rest, ")")?;
            Ok((Schema::List(Box::new(inner)), rest))
        }
        b'C' => {
            let mut i = 1;
            while i < s.len() && s.as_bytes()[i].is_ascii_digit() {
                i += 1;
            }
            let size: i64 = s[1..i]
                .parse()
                .map_err(|_| "bad class size".to_string())?;
            let rest = expect(&s[i..], "(")?;
            let mut fields = Vec::new();
            let mut rest = rest;
            if rest.starts_with(')') {
                return Ok((
                    Schema::Class {
                        size,
                        fields,
                    },
                    &rest[1..],
                ));
            }
            loop {
                let (wire, r) = split_until(rest, ':')?;
                let (ty, r) = parse_schema_at(r)?;
                let r = expect(r, ":")?;
                let (off_s, r) = split_field_end(r)?;
                let off: i64 = off_s
                    .parse()
                    .map_err(|_| format!("bad offset {off_s}"))?;
                fields.push((wire.to_string(), ty, off));
                if r.starts_with(',') {
                    rest = &r[1..];
                    continue;
                }
                let r = expect(r, ")")?;
                return Ok((
                    Schema::Class {
                        size,
                        fields,
                    },
                    r,
                ));
            }
        }
        _ => Err(format!("bad schema start: {s}")),
    }
}

fn expect<'a>(s: &'a str, prefix: &str) -> Result<&'a str, String> {
    if let Some(rest) = s.strip_prefix(prefix) {
        Ok(rest)
    } else {
        Err(format!("expected '{prefix}' in '{s}'"))
    }
}

fn split_until(s: &str, sep: char) -> Result<(&str, &str), String> {
    let Some(i) = s.find(sep) else {
        return Err(format!("missing '{sep}' in '{s}'"));
    };
    Ok((&s[..i], &s[i + 1..]))
}

fn split_field_end(s: &str) -> Result<(&str, &str), String> {
    let mut i = 0;
    while i < s.len() {
        let c = s.as_bytes()[i];
        if c == b',' || c == b')' {
            break;
        }
        i += 1;
    }
    Ok((&s[..i], &s[i..]))
}

#[derive(Clone, Debug)]
enum Val {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Val>),
    Obj(BTreeMap<String, Val>),
}

unsafe fn load_i64(obj: i64, off: i64) -> i64 {
    *((obj as *const u8).add(off as usize) as *const i64)
}

unsafe fn store_i64(obj: i64, off: i64, v: i64) {
    *((obj as *mut u8).add(off as usize) as *mut i64) = v;
}

unsafe fn encode_val(schema: &Schema, ptr: i64) -> Result<Val, String> {
    match schema {
        Schema::Int => Ok(Val::Int(ptr)),
        Schema::Float => Ok(Val::Float(f64::from_bits(ptr as u64))),
        Schema::String => Ok(Val::Str(cstr_to_string(ptr))),
        Schema::Bool => Ok(Val::Bool(ptr != 0)),
        Schema::Option(inner) => {
            // Option is tagged { tag:i64, payload:i64 } at ptr
            let tag = load_i64(ptr, 0);
            let payload = load_i64(ptr, 8);
            if tag == 1 {
                Ok(Val::Null)
            } else {
                encode_val(inner, payload)
            }
        }
        Schema::List(inner) => {
            let len = stk_list_len(ptr);
            let mut arr = Vec::with_capacity(len as usize);
            for i in 0..len {
                let el = stk_list_get(ptr, i);
                arr.push(encode_val(inner, el)?);
            }
            Ok(Val::Arr(arr))
        }
        Schema::Class { fields, .. } => {
            let mut map = BTreeMap::new();
            for (wire, ty, off) in fields {
                let slot = load_i64(ptr, *off);
                map.insert(wire.clone(), encode_val(ty, slot)?);
            }
            Ok(Val::Obj(map))
        }
    }
}

unsafe fn decode_val(schema: &Schema, v: &Val) -> Result<i64, String> {
    match (schema, v) {
        (Schema::Int, Val::Int(n)) => Ok(*n),
        (Schema::Float, Val::Float(f)) => Ok(f.to_bits() as i64),
        (Schema::Float, Val::Int(n)) => Ok((*n as f64).to_bits() as i64),
        (Schema::String, Val::Str(s)) => Ok(string_to_cstr(s.clone())),
        (Schema::Bool, Val::Bool(b)) => Ok(if *b { 1 } else { 0 }),
        (Schema::Option(_inner), Val::Null) => {
            let p = stk_alloc(16) as i64;
            store_i64(p, 0, 1);
            store_i64(p, 8, 0);
            Ok(p)
        }
        (Schema::Option(inner), other) => {
            let payload = decode_val(inner, other)?;
            let p = stk_alloc(16) as i64;
            store_i64(p, 0, 0);
            store_i64(p, 8, payload);
            Ok(p)
        }
        (Schema::List(inner), Val::Arr(items)) => {
            let list = stk_list_new();
            for it in items {
                stk_list_push(list, decode_val(inner, it)?);
            }
            Ok(list)
        }
        (Schema::Class { size, fields }, Val::Obj(map)) => {
            let obj = stk_alloc(*size) as i64;
            for i in 0..(*size / 8) {
                store_i64(obj, i * 8, 0);
            }
            for (wire, ty, off) in fields {
                let Some(fv) = map.get(wire) else {
                    // missing key: ok for Option fields as None
                    if let Schema::Option(_) = ty {
                        let p = stk_alloc(16) as i64;
                        store_i64(p, 0, 1);
                        store_i64(p, 8, 0);
                        store_i64(obj, *off, p);
                        continue;
                    }
                    return Err(format!("missing field '{wire}'"));
                };
                store_i64(obj, *off, decode_val(ty, fv)?);
            }
            Ok(obj)
        }
        _ => Err(format!("type mismatch for schema {:?}", schema)),
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn val_to_json(v: &Val) -> String {
    match v {
        Val::Null => "null".into(),
        Val::Bool(true) => "true".into(),
        Val::Bool(false) => "false".into(),
        Val::Int(n) => n.to_string(),
        Val::Float(f) => {
            if f.is_finite() {
                let mut s = f.to_string();
                if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                    s.push_str(".0");
                }
                s
            } else {
                "null".into()
            }
        }
        Val::Str(s) => json_escape(s),
        Val::Arr(a) => {
            let parts: Vec<_> = a.iter().map(val_to_json).collect();
            format!("[{}]", parts.join(","))
        }
        Val::Obj(m) => {
            let parts: Vec<_> = m
                .iter()
                .map(|(k, v)| format!("{}:{}", json_escape(k), val_to_json(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

struct JsonParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> JsonParser<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            s: s.as_bytes(),
            i: 0,
        }
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn bump(&mut self) {
        self.i += 1;
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.bump();
        }
    }
    fn parse_value(&mut self) -> Result<Val, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'n') => {
                self.eat("null")?;
                Ok(Val::Null)
            }
            Some(b't') => {
                self.eat("true")?;
                Ok(Val::Bool(true))
            }
            Some(b'f') => {
                self.eat("false")?;
                Ok(Val::Bool(false))
            }
            Some(b'"') => Ok(Val::Str(self.parse_string()?)),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            other => Err(format!("unexpected {:?}", other.map(|c| c as char))),
        }
    }
    fn eat(&mut self, lit: &str) -> Result<(), String> {
        for b in lit.bytes() {
            if self.peek() != Some(b) {
                return Err(format!("expected {lit}"));
            }
            self.bump();
        }
        Ok(())
    }
    fn parse_string(&mut self) -> Result<String, String> {
        self.eat("\"")?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err("unterminated string".into()),
                Some(b'"') => {
                    self.bump();
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.bump();
                    match self.peek() {
                        Some(b'"') => {
                            out.push('"');
                            self.bump();
                        }
                        Some(b'\\') => {
                            out.push('\\');
                            self.bump();
                        }
                        Some(b'n') => {
                            out.push('\n');
                            self.bump();
                        }
                        Some(b'r') => {
                            out.push('\r');
                            self.bump();
                        }
                        Some(b't') => {
                            out.push('\t');
                            self.bump();
                        }
                        Some(b'u') => {
                            self.bump();
                            let mut hex = String::new();
                            for _ in 0..4 {
                                let c = self.peek().ok_or("bad \\u")?;
                                hex.push(c as char);
                                self.bump();
                            }
                            let cp = u32::from_str_radix(&hex, 16)
                                .map_err(|_| "bad \\u".to_string())?;
                            out.push(char::from_u32(cp).ok_or("bad \\u")?);
                        }
                        _ => return Err("bad escape".into()),
                    }
                }
                Some(c) => {
                    out.push(c as char);
                    self.bump();
                }
            }
        }
    }
    fn parse_number(&mut self) -> Result<Val, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.bump();
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.bump();
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump();
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.bump();
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.bump();
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump();
            }
        }
        let raw = std::str::from_utf8(&self.s[start..self.i]).unwrap();
        if is_float {
            Ok(Val::Float(
                raw.parse().map_err(|_| format!("bad float {raw}"))?,
            ))
        } else {
            Ok(Val::Int(
                raw.parse().map_err(|_| format!("bad int {raw}"))?,
            ))
        }
    }
    fn parse_array(&mut self) -> Result<Val, String> {
        self.eat("[")?;
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.bump();
            return Ok(Val::Arr(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                    continue;
                }
                Some(b']') => {
                    self.bump();
                    return Ok(Val::Arr(items));
                }
                _ => return Err("expected , or ]".into()),
            }
        }
    }
    fn parse_object(&mut self) -> Result<Val, String> {
        self.eat("{")?;
        self.skip_ws();
        let mut map = BTreeMap::new();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(Val::Obj(map));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            self.eat(":")?;
            let val = self.parse_value()?;
            map.insert(key, val);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                    continue;
                }
                Some(b'}') => {
                    self.bump();
                    return Ok(Val::Obj(map));
                }
                _ => return Err("expected , or }".into()),
            }
        }
    }
}

fn parse_json(s: &str) -> Result<Val, String> {
    let mut p = JsonParser::new(s);
    let v = p.parse_value()?;
    p.skip_ws();
    if p.i != p.s.len() {
        return Err("trailing junk after JSON".into());
    }
    Ok(v)
}

/// Minimal YAML: maps/lists/scalars. Encode as JSON (valid YAML 1.2) for a reliable
/// round-trip with the MVP parser; decode accepts JSON or flat `key: value` lines.
fn val_to_yaml(v: &Val, _indent: usize) -> String {
    val_to_json(v)
}

fn parse_yaml_simple(s: &str) -> Result<Val, String> {
    // MVP: accept JSON as YAML, or flat `key: value` lines / nested via indent.
    let t = s.trim();
    if t.starts_with('{') || t.starts_with('[') {
        return parse_json(t);
    }
    parse_yaml_lines(t)
}

fn parse_yaml_lines(s: &str) -> Result<Val, String> {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#')).collect();
    if lines.is_empty() {
        return Ok(Val::Null);
    }
    if lines.iter().all(|l| l.trim_start().starts_with('-')) {
        let mut arr = Vec::new();
        for l in lines {
            let item = l.trim_start().trim_start_matches('-').trim();
            arr.push(parse_yaml_scalar(item)?);
        }
        return Ok(Val::Arr(arr));
    }
    let mut map = BTreeMap::new();
    for l in lines {
        let Some((k, v)) = l.split_once(':') else {
            return Err(format!("bad yaml line: {l}"));
        };
        map.insert(k.trim().to_string(), parse_yaml_scalar(v.trim())?);
    }
    Ok(Val::Obj(map))
}

fn parse_yaml_scalar(s: &str) -> Result<Val, String> {
    if s.is_empty() || s == "null" || s == "~" {
        return Ok(Val::Null);
    }
    if s == "true" {
        return Ok(Val::Bool(true));
    }
    if s == "false" {
        return Ok(Val::Bool(false));
    }
    if s.starts_with('"') || s.starts_with('[') || s.starts_with('{') {
        return parse_json(s);
    }
    if let Ok(n) = s.parse::<i64>() {
        return Ok(Val::Int(n));
    }
    if let Ok(f) = s.parse::<f64>() {
        return Ok(Val::Float(f));
    }
    Ok(Val::Str(s.to_string()))
}

fn val_to_toml(v: &Val) -> Result<String, String> {
    let Val::Obj(map) = v else {
        return Err("TOML root must be an object".into());
    };
    let mut out = String::new();
    // Scalars/arrays first so a following [table] does not capture later root keys.
    for (k, val) in map {
        match val {
            Val::Null | Val::Obj(_) => {}
            other => {
                out.push_str(&format!("{k} = {}\n", toml_scalar(other)?));
            }
        }
    }
    for (k, val) in map {
        if let Val::Obj(nested) = val {
            out.push_str(&format!("\n[{k}]\n"));
            for (nk, nv) in nested {
                if matches!(nv, Val::Null) {
                    continue;
                }
                out.push_str(&format!("{nk} = {}\n", toml_scalar(nv)?));
            }
        }
    }
    Ok(out)
}

fn toml_scalar(v: &Val) -> Result<String, String> {
    Ok(match v {
        Val::Bool(b) => b.to_string(),
        Val::Int(n) => n.to_string(),
        Val::Float(f) => f.to_string(),
        Val::Str(s) => json_escape(s),
        Val::Arr(a) => {
            let parts: Result<Vec<_>, _> = a.iter().map(toml_scalar).collect();
            format!("[{}]", parts?.join(", "))
        }
        Val::Null => return Err("TOML cannot encode null".into()),
        Val::Obj(_) => return Err("nested TOML tables only one level in MVP".into()),
    })
}

fn parse_toml_simple(s: &str) -> Result<Val, String> {
    let mut map = BTreeMap::new();
    let mut section: Option<String> = None;
    for raw in s.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = Some(line[1..line.len() - 1].to_string());
            map.entry(section.clone().unwrap())
                .or_insert_with(|| Val::Obj(BTreeMap::new()));
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!("bad toml: {line}"));
        };
        let key = k.trim().to_string();
        let val = parse_yaml_scalar(v.trim())?;
        if let Some(sec) = &section {
            let Val::Obj(ref mut m) = map.get_mut(sec).unwrap() else {
                return Err("internal toml".into());
            };
            m.insert(key, val);
        } else {
            map.insert(key, val);
        }
    }
    Ok(Val::Obj(map))
}

/// TOON: compact JSON-model text. MVP encodes as JSON (valid TOON-compatible data model).
fn val_to_toon(v: &Val, _indent: usize) -> String {
    val_to_json(v)
}

fn parse_toon(s: &str) -> Result<Val, String> {
    let t = s.trim();
    if t.starts_with('{') || t.starts_with('[') && t.contains('{') {
        // try JSON first
        if let Ok(v) = parse_json(t) {
            return Ok(v);
        }
    }
    // fall back to yaml-like
    parse_yaml_simple(t)
}

fn format_id(fmt: i64) -> &'static str {
    match fmt {
        0 => "json",
        1 => "yaml",
        2 => "toml",
        3 => "toon",
        _ => "json",
    }
}

unsafe fn encode_with_format(fmt: i64, schema: &str, ptr: i64) -> Result<String, String> {
    let sch = parse_schema(schema)?;
    let val = encode_val(&sch, ptr)?;
    match format_id(fmt) {
        "yaml" => Ok(val_to_yaml(&val, 0)),
        "toml" => val_to_toml(&val),
        "toon" => Ok(val_to_toon(&val, 0)),
        _ => Ok(val_to_json(&val)),
    }
}

unsafe fn decode_with_format(fmt: i64, schema: &str, text: &str) -> Result<i64, String> {
    let sch = parse_schema(schema)?;
    let val = match format_id(fmt) {
        "yaml" => parse_yaml_simple(text)?,
        "toml" => parse_toml_simple(text)?,
        "toon" => parse_toon(text)?,
        _ => parse_json(text)?,
    };
    decode_val(&sch, &val)
}

/// `stk_serde_encode(format, schema_cstr, value) -> string`
/// format: 0=json 1=yaml 2=toml 3=toon
#[no_mangle]
pub unsafe extern "C" fn stk_serde_encode(format: i64, schema: i64, value: i64) -> i64 {
    let sch = cstr_to_string(schema);
    match encode_with_format(format, &sch, value) {
        Ok(s) => string_to_cstr(s),
        Err(e) => {
            eprintln!("stk serde encode error: {e}");
            string_to_cstr(String::new())
        }
    }
}

/// `stk_serde_decode(format, schema_cstr, text) -> Result` (tagged ok=0/err=1)
#[no_mangle]
pub unsafe extern "C" fn stk_serde_decode(format: i64, schema: i64, text: i64) -> i64 {
    let sch = cstr_to_string(schema);
    let t = cstr_to_string(text);
    match decode_with_format(format, &sch, &t) {
        Ok(v) => make_tagged(0, v),
        Err(e) => make_tagged(1, string_to_cstr(e)),
    }
}
