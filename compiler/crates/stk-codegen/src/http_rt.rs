//! HTTP/1.1 client + Express-style server (`http://` only).

use crate::runtime::{
    cstr_to_string, make_tagged, stk_future_complete, stk_future_new, string_to_cstr,
};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use std::thread;

#[derive(Default)]
pub struct HttpHeaders {
    pub map: HashMap<String, String>,
}

pub struct HttpResponse {
    pub status: i64,
    pub headers: HttpHeaders,
    pub body: String,
}

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub params: HashMap<String, String>,
    pub body: String,
}

struct Route {
    method: String,
    pattern: String,
    /// Fat pointer { code, env } for `fn(Request) Response`.
    handler_fat: i64,
}

struct HttpServer {
    routes: Mutex<Vec<Route>>,
}

fn header_get_ci(map: &HashMap<String, String>, key: &str) -> Option<String> {
    let want = key.to_ascii_lowercase();
    map.iter()
        .find(|(k, _)| k.to_ascii_lowercase() == want)
        .map(|(_, v)| v.clone())
}

fn parse_query(q: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for part in q.split('&') {
        if part.is_empty() {
            continue;
        }
        if let Some((k, v)) = part.split_once('=') {
            out.insert(k.to_string(), v.to_string());
        } else {
            out.insert(part.to_string(), String::new());
        }
    }
    out
}

fn match_route(pattern: &str, path: &str) -> Option<HashMap<String, String>> {
    let p_segs: Vec<&str> = pattern.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    let t_segs: Vec<&str> = path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    if p_segs.len() != t_segs.len() {
        return None;
    }
    let mut params = HashMap::new();
    for (p, t) in p_segs.iter().zip(t_segs.iter()) {
        if let Some(name) = p.strip_prefix(':') {
            params.insert(name.to_string(), (*t).to_string());
        } else if *p != *t {
            return None;
        }
    }
    Some(params)
}

fn parse_url(url: &str) -> Result<(String, u16, String, HashMap<String, String>), String> {
    let url = url
        .strip_prefix("http://")
        .ok_or_else(|| "std.http only supports http:// URLs (HTTPS not in MVP)".to_string())?;
    let (host_port, path_q) = match url.split_once('/') {
        Some((h, rest)) => (h, format!("/{rest}")),
        None => (url, "/".to_string()),
    };
    let (path, query) = match path_q.split_once('?') {
        Some((p, q)) => (p.to_string(), parse_query(q)),
        None => (path_q, HashMap::new()),
    };
    let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
        (
            h.to_string(),
            p.parse::<u16>().map_err(|_| "bad port".to_string())?,
        )
    } else {
        (host_port.to_string(), 80u16)
    };
    Ok((host, port, path, query))
}

fn format_query(q: &HashMap<String, String>) -> String {
    if q.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = q.iter().map(|(k, v)| format!("{k}={v}")).collect();
    parts.sort();
    format!("?{}", parts.join("&"))
}

fn http_exchange(
    method: &str,
    url: &str,
    body: &str,
    headers: Option<&HttpHeaders>,
) -> Result<HttpResponse, String> {
    let (host, port, path, query) = parse_url(url)?;
    let path_q = format!("{path}{}", format_query(&query));
    let mut stream = TcpStream::connect((host.as_str(), port)).map_err(|e| e.to_string())?;
    let mut req = format!("{method} {path_q} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    if !body.is_empty() {
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    if let Some(h) = headers {
        for (k, v) in &h.map {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
    }
    req.push_str("\r\n");
    req.push_str(body);
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    parse_http_response(&text)
}

fn parse_http_response(raw: &str) -> Result<HttpResponse, String> {
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .unwrap_or((raw, ""));
    let mut lines = head.lines();
    let status_line = lines.next().ok_or_else(|| "empty response".to_string())?;
    let mut parts = status_line.split_whitespace();
    let _http = parts.next();
    let status: i64 = parts
        .next()
        .ok_or_else(|| "bad status line".to_string())?
        .parse()
        .map_err(|_| "bad status code".to_string())?;
    let mut headers = HttpHeaders::default();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers
                .map
                .insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    Ok(HttpResponse {
        status,
        headers,
        body: body.to_string(),
    })
}

fn parse_http_request(raw: &str) -> Result<HttpRequest, String> {
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .unwrap_or((raw, ""));
    let mut lines = head.lines();
    let start = lines.next().ok_or_else(|| "empty request".to_string())?;
    let mut parts = start.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "bad request line".to_string())?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| "bad request path".to_string())?
        .to_string();
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), parse_query(q)),
        None => (target, HashMap::new()),
    };
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    let mut body = body.to_string();
    if let Some(cl) = header_get_ci(&headers, "Content-Length") {
        if let Ok(n) = cl.parse::<usize>() {
            if body.len() > n {
                body.truncate(n);
            }
        }
    }
    Ok(HttpRequest {
        method,
        path,
        query,
        headers,
        params: HashMap::new(),
        body,
    })
}

fn write_http_response(stream: &mut TcpStream, resp: &HttpResponse) -> Result<(), String> {
    let reason = match resp.status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let mut out = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        resp.status,
        reason,
        resp.body.len()
    );
    for (k, v) in &resp.headers.map {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str("\r\n");
    out.push_str(&resp.body);
    stream.write_all(out.as_bytes()).map_err(|e| e.to_string())
}

unsafe fn call_handler(fat: i64, req: i64) -> i64 {
    let code = *(fat as *const i64);
    let env = *((fat as *const i64).add(1));
    let f: unsafe extern "C" fn(i64, i64) -> i64 = std::mem::transmute(code);
    f(env, req)
}

fn handle_connection(server: &HttpServer, mut stream: TcpStream) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n")
                    || buf.windows(2).any(|w| w == b"\n\n")
                {
                    // If Content-Length, try to read remaining body.
                    let raw = String::from_utf8_lossy(&buf);
                    if let Ok(req) = parse_http_request(&raw) {
                        if let Some(cl) = header_get_ci(&req.headers, "Content-Length") {
                            if let Ok(need) = cl.parse::<usize>() {
                                let head_end = raw
                                    .find("\r\n\r\n")
                                    .map(|i| i + 4)
                                    .or_else(|| raw.find("\n\n").map(|i| i + 2))
                                    .unwrap_or(raw.len());
                                let have = buf.len().saturating_sub(head_end);
                                if have < need {
                                    continue;
                                }
                            }
                        }
                    }
                    break;
                }
                if buf.len() > 1_000_000 {
                    break;
                }
            }
            Err(_) => return,
        }
    }
    let raw = String::from_utf8_lossy(&buf);
    let mut req = match parse_http_request(&raw) {
        Ok(r) => r,
        Err(_) => {
            let _ = write_http_response(
                &mut stream,
                &HttpResponse {
                    status: 400,
                    headers: HttpHeaders::default(),
                    body: "bad request".into(),
                },
            );
            return;
        }
    };
    let routes = server.routes.lock().unwrap();
    let mut matched: Option<&Route> = None;
    for r in routes.iter() {
        if r.method != req.method {
            continue;
        }
        if let Some(params) = match_route(&r.pattern, &req.path) {
            req.params = params;
            matched = Some(r);
            break;
        }
    }
    let resp = if let Some(route) = matched {
        let req_ptr = Box::into_raw(Box::new(req)) as i64;
        let resp_ptr = unsafe { call_handler(route.handler_fat, req_ptr) };
        // Request is consumed by handler ownership model — free if still allocated.
        // Handlers only read; we free request after call.
        unsafe {
            let _ = Box::from_raw(req_ptr as *mut HttpRequest);
        }
        if resp_ptr == 0 {
            HttpResponse {
                status: 500,
                headers: HttpHeaders::default(),
                body: "null response".into(),
            }
        } else {
            unsafe { *Box::from_raw(resp_ptr as *mut HttpResponse) }
        }
    } else {
        HttpResponse {
            status: 404,
            headers: HttpHeaders::default(),
            body: "not found".into(),
        }
    };
    let _ = write_http_response(&mut stream, &resp);
}

// ---- C ABI ----

#[no_mangle]
pub unsafe extern "C" fn stk_http_headers_new() -> i64 {
    Box::into_raw(Box::new(HttpHeaders::default())) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_headers_set(h: i64, key: i64, value: i64) {
    let headers = &mut *(h as *mut HttpHeaders);
    headers
        .map
        .insert(cstr_to_string(key), cstr_to_string(value));
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_headers_get(h: i64, key: i64) -> i64 {
    let headers = &*(h as *const HttpHeaders);
    let k = cstr_to_string(key);
    match header_get_ci(&headers.map, &k) {
        Some(v) => make_tagged(0, string_to_cstr(v)),
        None => make_tagged(1, 0),
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_response_text(status: i64, body: i64) -> i64 {
    let mut headers = HttpHeaders::default();
    headers
        .map
        .insert("Content-Type".into(), "text/plain; charset=utf-8".into());
    Box::into_raw(Box::new(HttpResponse {
        status,
        headers,
        body: cstr_to_string(body),
    })) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_response_json(status: i64, body: i64) -> i64 {
    let mut headers = HttpHeaders::default();
    headers
        .map
        .insert("Content-Type".into(), "application/json".into());
    Box::into_raw(Box::new(HttpResponse {
        status,
        headers,
        body: cstr_to_string(body),
    })) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_response_empty(status: i64) -> i64 {
    Box::into_raw(Box::new(HttpResponse {
        status,
        headers: HttpHeaders::default(),
        body: String::new(),
    })) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_response_status(r: i64) -> i64 {
    (*(r as *const HttpResponse)).status
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_response_body(r: i64) -> i64 {
    string_to_cstr((*(r as *const HttpResponse)).body.clone())
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_response_set_header(r: i64, key: i64, value: i64) {
    let resp = &mut *(r as *mut HttpResponse);
    resp.headers
        .map
        .insert(cstr_to_string(key), cstr_to_string(value));
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_response_header(r: i64, key: i64) -> i64 {
    let resp = &*(r as *const HttpResponse);
    let k = cstr_to_string(key);
    match header_get_ci(&resp.headers.map, &k) {
        Some(v) => make_tagged(0, string_to_cstr(v)),
        None => make_tagged(1, 0),
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_request_method(r: i64) -> i64 {
    string_to_cstr((*(r as *const HttpRequest)).method.clone())
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_request_path(r: i64) -> i64 {
    string_to_cstr((*(r as *const HttpRequest)).path.clone())
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_request_body(r: i64) -> i64 {
    string_to_cstr((*(r as *const HttpRequest)).body.clone())
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_request_query(r: i64, name: i64) -> i64 {
    let req = &*(r as *const HttpRequest);
    let n = cstr_to_string(name);
    match req.query.get(&n) {
        Some(v) => make_tagged(0, string_to_cstr(v.clone())),
        None => make_tagged(1, 0),
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_request_header(r: i64, name: i64) -> i64 {
    let req = &*(r as *const HttpRequest);
    let n = cstr_to_string(name);
    match header_get_ci(&req.headers, &n) {
        Some(v) => make_tagged(0, string_to_cstr(v)),
        None => make_tagged(1, 0),
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_request_param(r: i64, name: i64) -> i64 {
    let req = &*(r as *const HttpRequest);
    let n = cstr_to_string(name);
    string_to_cstr(req.params.get(&n).cloned().unwrap_or_default())
}

/// method: 0=GET 1=POST 2=PUT 3=DELETE 4=PATCH
/// headers may be 0; body may be 0 for GET/DELETE.
/// Returns Future<Result<Response,string>>
#[no_mangle]
pub unsafe extern "C" fn stk_http_client(
    method: i64,
    url: i64,
    body: i64,
    headers: i64,
) -> i64 {
    let fut = stk_future_new();
    let url_s = cstr_to_string(url);
    let body_s = if body == 0 {
        String::new()
    } else {
        cstr_to_string(body)
    };
    let headers_owned: Option<HttpHeaders> = if headers == 0 {
        None
    } else {
        Some((*(headers as *const HttpHeaders)).clone_map())
    };
    let method_s = match method {
        1 => "POST",
        2 => "PUT",
        3 => "DELETE",
        4 => "PATCH",
        _ => "GET",
    }
    .to_string();
    thread::spawn(move || {
        let result = http_exchange(
            &method_s,
            &url_s,
            &body_s,
            headers_owned.as_ref(),
        );
        let tagged = match result {
            Ok(resp) => make_tagged(0, Box::into_raw(Box::new(resp)) as i64),
            Err(e) => make_tagged(1, string_to_cstr(e)),
        };
        unsafe { stk_future_complete(fut, tagged) };
    });
    fut
}

impl HttpHeaders {
    fn clone_map(&self) -> HttpHeaders {
        HttpHeaders {
            map: self.map.clone(),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_server_new() -> i64 {
    Box::into_raw(Box::new(HttpServer {
        routes: Mutex::new(Vec::new()),
    })) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_http_server_route(
    server: i64,
    method: i64,
    path: i64,
    handler_fat: i64,
) {
    let s = &*(server as *const HttpServer);
    let mut routes = s.routes.lock().unwrap();
    routes.push(Route {
        method: cstr_to_string(method).to_ascii_uppercase(),
        pattern: cstr_to_string(path),
        handler_fat,
    });
}

/// Returns Future<Result<int,string>> — completes with err on bind failure;
/// on success runs accept loop (future stays pending until process exit).
#[no_mangle]
pub unsafe extern "C" fn stk_http_server_listen(server: i64, port: i64) -> i64 {
    let fut = stk_future_new();
    let server_ptr = server;
    thread::spawn(move || {
        let s = unsafe { &*(server_ptr as *const HttpServer) };
        let addr = format!("0.0.0.0:{port}");
        if !(1..=65535).contains(&port) {
            unsafe {
                stk_future_complete(
                    fut,
                    make_tagged(1, string_to_cstr(format!("invalid port {port}"))),
                );
            }
            return;
        }
        let listener = match TcpListener::bind(&addr) {
            Ok(l) => l,
            Err(e) => {
                unsafe {
                    stk_future_complete(fut, make_tagged(1, string_to_cstr(e.to_string())));
                }
                return;
            }
        };
        for conn in listener.incoming() {
            match conn {
                Ok(stream) => {
                    handle_connection(s, stream);
                }
                Err(e) => {
                    eprintln!("stk http accept error: {e}");
                }
            }
        }
    });
    fut
}

/// Legacy sync get → Result<string,string>.
#[no_mangle]
pub unsafe extern "C" fn stk_http_get(url: i64) -> i64 {
    let url = cstr_to_string(url);
    match http_exchange("GET", &url, "", None) {
        Ok(resp) => make_tagged(0, string_to_cstr(resp.body)),
        Err(e) => make_tagged(1, string_to_cstr(e)),
    }
}
