//! Host runtime for JIT. Same ABI as `runtime.c` for AOT.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::thread;
use std::time::Duration;

extern "C" {
    fn calloc(nmemb: usize, size: usize) -> *mut std::ffi::c_void;
    fn free(p: *mut std::ffi::c_void);
    fn malloc(size: usize) -> *mut std::ffi::c_void;
}

#[no_mangle]
pub unsafe extern "C" fn stk_alloc(size: i64) -> *mut u8 {
    let size = if size <= 0 { 8usize } else { size as usize };
    calloc(1, size) as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn stk_free(ptr: *mut u8) {
    if !ptr.is_null() {
        free(ptr as *mut std::ffi::c_void);
    }
}

struct FutureInner {
    ready: bool,
    value: i64,
}

struct Future {
    inner: Mutex<FutureInner>,
    cv: Condvar,
}

#[no_mangle]
pub unsafe extern "C" fn stk_future_new() -> i64 {
    let f = Box::new(Future {
        inner: Mutex::new(FutureInner {
            ready: false,
            value: 0,
        }),
        cv: Condvar::new(),
    });
    Box::into_raw(f) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_future_complete(handle: i64, value: i64) {
    let f = &*(handle as *const Future);
    let mut g = f.inner.lock().unwrap();
    if !g.ready {
        g.value = value;
        g.ready = true;
        f.cv.notify_all();
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_future_ready(value: i64) -> i64 {
    let h = stk_future_new();
    stk_future_complete(h, value);
    h
}

unsafe fn future_get(handle: i64) -> i64 {
    let f = &*(handle as *const Future);
    let mut g = f.inner.lock().unwrap();
    while !g.ready {
        g = f.cv.wait(g).unwrap();
    }
    g.value
}

#[no_mangle]
pub unsafe extern "C" fn stk_future_await(handle: i64) -> i64 {
    // Do not free: Future.race losers may still call complete after await.
    future_get(handle)
}

#[no_mangle]
pub unsafe extern "C" fn stk_future_join(h1: i64, h2: i64) -> i64 {
    let a = stk_future_await(h1);
    let b = stk_future_await(h2);
    let arr = malloc(16) as *mut i64;
    if arr.is_null() {
        panic!("stk_future_join: oom");
    }
    *arr = a;
    *arr.add(1) = b;
    stk_future_ready(arr as i64)
}

#[no_mangle]
pub unsafe extern "C" fn stk_future_race(h1: i64, h2: i64) -> i64 {
    let dest = stk_future_new();
    let dest_a = dest;
    thread::spawn(move || {
        let v = future_get(h1);
        stk_future_complete(dest_a, v);
    });
    let dest_b = dest;
    thread::spawn(move || {
        let v = future_get(h2);
        stk_future_complete(dest_b, v);
    });
    dest
}

static SPAWN_LIVE: AtomicIsize = AtomicIsize::new(0);

type Job = Box<dyn FnOnce() + Send + 'static>;

fn spawn_job(job: Job) {
    use std::sync::{mpsc, Arc, OnceLock};
    static TX: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        let n = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(2, 16);
        for _ in 0..n {
            let rx = Arc::clone(&rx);
            thread::spawn(move || loop {
                let job = {
                    let g = rx.lock().unwrap();
                    g.recv()
                };
                match job {
                    Ok(job) => job(),
                    Err(_) => break,
                }
            });
        }
        tx
    });
    let _ = tx.send(job);
}

#[no_mangle]
pub unsafe extern "C" fn stk_sleep_ms(ms: i64) {
    if ms > 0 {
        thread::sleep(Duration::from_millis(ms as u64));
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_spawn(fn_ptr: i64, env: i64) {
    SPAWN_LIVE.fetch_add(1, Ordering::SeqCst);
    let f: extern "C" fn(i64) = std::mem::transmute(fn_ptr as *const ());
    spawn_job(Box::new(move || {
        f(env);
        SPAWN_LIVE.fetch_sub(1, Ordering::SeqCst);
    }));
}

#[no_mangle]
pub unsafe extern "C" fn stk_spawn_drain() {
    while SPAWN_LIVE.load(Ordering::SeqCst) > 0 {
        thread::sleep(Duration::from_millis(1));
    }
}

struct ChannelInner {
    queue: VecDeque<i64>,
    closed: bool,
    max_len: usize, // 0 = unbounded
}

struct Channel {
    inner: Mutex<ChannelInner>,
    not_empty: Condvar,
    not_full: Condvar,
}

fn channel_create(max_len: usize) -> i64 {
    let ch = Box::new(Channel {
        inner: Mutex::new(ChannelInner {
            queue: VecDeque::new(),
            closed: false,
            max_len,
        }),
        not_empty: Condvar::new(),
        not_full: Condvar::new(),
    });
    Box::into_raw(ch) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_channel_new() -> i64 {
    channel_create(0)
}

#[no_mangle]
pub unsafe extern "C" fn stk_channel_buffered(n: i64) -> i64 {
    if n < 1 {
        eprintln!("stk: Channel.buffered requires n >= 1");
        std::process::abort();
    }
    channel_create(n as usize)
}

#[no_mangle]
pub unsafe extern "C" fn stk_channel_send(handle: i64, value: i64) {
    let ch = &*(handle as *const Channel);
    let mut g = ch.inner.lock().unwrap();
    if g.closed {
        drop(g);
        eprintln!("stk: send on closed channel");
        std::process::abort();
    }
    while g.max_len > 0 && g.queue.len() >= g.max_len {
        g = ch.not_full.wait(g).unwrap();
        if g.closed {
            drop(g);
            eprintln!("stk: send on closed channel");
            std::process::abort();
        }
    }
    g.queue.push_back(value);
    ch.not_empty.notify_one();
}

#[no_mangle]
pub unsafe extern "C" fn stk_channel_close(handle: i64) {
    let ch = &*(handle as *const Channel);
    let mut g = ch.inner.lock().unwrap();
    g.closed = true;
    ch.not_empty.notify_all();
    ch.not_full.notify_all();
}

#[no_mangle]
pub unsafe extern "C" fn stk_channel_recv_ok(handle: i64, out: *mut i64) -> i64 {
    let ch = &*(handle as *const Channel);
    let mut g = ch.inner.lock().unwrap();
    loop {
        if let Some(v) = g.queue.pop_front() {
            ch.not_full.notify_one();
            if !out.is_null() {
                *out = v;
            }
            return 1;
        }
        if g.closed {
            return 0;
        }
        g = ch.not_empty.wait(g).unwrap();
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_channel_recv(handle: i64) -> i64 {
    let mut v = 0i64;
    if stk_channel_recv_ok(handle, &mut v) == 0 {
        eprintln!("stk: recv on closed empty channel");
        std::process::abort();
    }
    v
}

#[no_mangle]
pub unsafe extern "C" fn stk_channel_recv_future(handle: i64) -> i64 {
    let dest = stk_future_new();
    let ch = handle;
    SPAWN_LIVE.fetch_add(1, Ordering::SeqCst);
    thread::spawn(move || {
        let v = stk_channel_recv(ch);
        stk_future_complete(dest, v);
        SPAWN_LIVE.fetch_sub(1, Ordering::SeqCst);
    });
    dest
}

struct WaitGroupInner {
    count: i64,
}

struct WaitGroup {
    inner: Mutex<WaitGroupInner>,
    cv: Condvar,
}

#[no_mangle]
pub unsafe extern "C" fn stk_waitgroup_new() -> i64 {
    let wg = Box::new(WaitGroup {
        inner: Mutex::new(WaitGroupInner { count: 0 }),
        cv: Condvar::new(),
    });
    Box::into_raw(wg) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_waitgroup_add(handle: i64, delta: i64) {
    let wg = &*(handle as *const WaitGroup);
    let mut g = wg.inner.lock().unwrap();
    g.count += delta;
    if g.count < 0 {
        drop(g);
        eprintln!("stk: WaitGroup counter negative");
        std::process::abort();
    }
    if g.count == 0 {
        wg.cv.notify_all();
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_waitgroup_done(handle: i64) {
    stk_waitgroup_add(handle, -1);
}

#[no_mangle]
pub unsafe extern "C" fn stk_waitgroup_wait(handle: i64) {
    let wg = &*(handle as *const WaitGroup);
    let mut g = wg.inner.lock().unwrap();
    while g.count > 0 {
        g = wg.cv.wait(g).unwrap();
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_waitgroup_wait_future(handle: i64) -> i64 {
    let dest = stk_future_new();
    let wg = handle;
    SPAWN_LIVE.fetch_add(1, Ordering::SeqCst);
    thread::spawn(move || {
        stk_waitgroup_wait(wg);
        stk_future_complete(dest, 0);
        SPAWN_LIVE.fetch_sub(1, Ordering::SeqCst);
    });
    dest
}

struct StkMutexInner {
    locked: bool,
    value: i64,
}

struct StkMutex {
    inner: Mutex<StkMutexInner>,
    cv: Condvar,
}

#[no_mangle]
pub unsafe extern "C" fn stk_mutex_new(initial: i64) -> i64 {
    let m = Box::new(StkMutex {
        inner: Mutex::new(StkMutexInner {
            locked: false,
            value: initial,
        }),
        cv: Condvar::new(),
    });
    Box::into_raw(m) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_mutex_lock(handle: i64) {
    let m = &*(handle as *const StkMutex);
    let mut g = m.inner.lock().unwrap();
    while g.locked {
        g = m.cv.wait(g).unwrap();
    }
    g.locked = true;
}

#[no_mangle]
pub unsafe extern "C" fn stk_mutex_unlock(handle: i64) {
    let m = &*(handle as *const StkMutex);
    let mut g = m.inner.lock().unwrap();
    if !g.locked {
        drop(g);
        eprintln!("stk: Mutex.unlock without lock");
        std::process::abort();
    }
    g.locked = false;
    m.cv.notify_one();
}

#[no_mangle]
pub unsafe extern "C" fn stk_mutex_get(handle: i64) -> i64 {
    let m = &*(handle as *const StkMutex);
    m.inner.lock().unwrap().value
}

#[no_mangle]
pub unsafe extern "C" fn stk_mutex_set(handle: i64, value: i64) {
    let m = &*(handle as *const StkMutex);
    m.inner.lock().unwrap().value = value;
}

/// Readers-writer lock (MVP): many readers OR one writer.
struct StkRwLockInner {
    readers: i64,
    writer: bool,
    value: i64,
}

struct StkRwLock {
    inner: Mutex<StkRwLockInner>,
    cv: Condvar,
}

#[no_mangle]
pub unsafe extern "C" fn stk_rwlock_new(initial: i64) -> i64 {
    let m = Box::new(StkRwLock {
        inner: Mutex::new(StkRwLockInner {
            readers: 0,
            writer: false,
            value: initial,
        }),
        cv: Condvar::new(),
    });
    Box::into_raw(m) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_rwlock_read_lock(handle: i64) {
    let m = &*(handle as *const StkRwLock);
    let mut g = m.inner.lock().unwrap();
    while g.writer {
        g = m.cv.wait(g).unwrap();
    }
    g.readers += 1;
}

#[no_mangle]
pub unsafe extern "C" fn stk_rwlock_read_unlock(handle: i64) {
    let m = &*(handle as *const StkRwLock);
    let mut g = m.inner.lock().unwrap();
    g.readers -= 1;
    if g.readers == 0 {
        m.cv.notify_all();
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_rwlock_write_lock(handle: i64) {
    let m = &*(handle as *const StkRwLock);
    let mut g = m.inner.lock().unwrap();
    while g.writer || g.readers > 0 {
        g = m.cv.wait(g).unwrap();
    }
    g.writer = true;
}

#[no_mangle]
pub unsafe extern "C" fn stk_rwlock_write_unlock(handle: i64) {
    let m = &*(handle as *const StkRwLock);
    let mut g = m.inner.lock().unwrap();
    g.writer = false;
    m.cv.notify_all();
}

#[no_mangle]
pub unsafe extern "C" fn stk_rwlock_get(handle: i64) -> i64 {
    let m = &*(handle as *const StkRwLock);
    m.inner.lock().unwrap().value
}

#[no_mangle]
pub unsafe extern "C" fn stk_rwlock_set(handle: i64, value: i64) {
    let m = &*(handle as *const StkRwLock);
    m.inner.lock().unwrap().value = value;
}

/// Apply `fn(i64) -> i64` to each List<int> element in parallel; returns new list.
#[no_mangle]
pub unsafe extern "C" fn stk_parallel_map_int(list: i64, fn_ptr: i64) -> i64 {
    let f: extern "C" fn(i64) -> i64 = std::mem::transmute(fn_ptr as *const ());
    let src = &*(list as *const StkList);
    let items = src.items.lock().unwrap().clone();
    let out = stk_list_new();
    let results: Vec<i64> = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for v in items {
            handles.push(s.spawn(move || f(v)));
        }
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for v in results {
        stk_list_push(out, v);
    }
    out
}

/// Blocking HTTP GET (http:// only, MVP). Returns Result<string,string> tagged.
#[no_mangle]
pub unsafe extern "C" fn stk_http_get(url: i64) -> i64 {
    let url = cstr_to_string(url);
    match http_get_blocking(&url) {
        Ok(body) => make_tagged(0, string_to_cstr(body)),
        Err(e) => make_tagged(1, string_to_cstr(e)),
    }
}

fn http_get_blocking(url: &str) -> Result<String, String> {
    let url = url
        .strip_prefix("http://")
        .ok_or_else(|| "MVP std.http.get only supports http:// URLs".to_string())?;
    let (host_port, path) = match url.split_once('/') {
        Some((h, p)) => (h, format!("/{p}")),
        None => (url, "/".to_string()),
    };
    let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
        (h, p.parse::<u16>().map_err(|_| "bad port".to_string())?)
    } else {
        (host_port, 80u16)
    };
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let mut stream = TcpStream::connect((host, port)).map_err(|e| e.to_string())?;
    let req = format!(
        "GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut buf = String::new();
    stream.read_to_string(&mut buf).map_err(|e| e.to_string())?;
    if let Some(idx) = buf.find("\r\n\r\n") {
        Ok(buf[idx + 4..].to_string())
    } else {
        Ok(buf)
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_task_yield() {
    thread::yield_now();
}

/// Cooperative cancel flag (0/1).
#[no_mangle]
pub unsafe extern "C" fn stk_cancel_token_new() -> i64 {
    let t = Box::new(std::sync::atomic::AtomicI64::new(0));
    Box::into_raw(t) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_cancel_token_cancel(handle: i64) {
    let t = &*(handle as *const std::sync::atomic::AtomicI64);
    t.store(1, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn stk_cancel_token_is_cancelled(handle: i64) -> i64 {
    let t = &*(handle as *const std::sync::atomic::AtomicI64);
    t.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn stk_std_log(
    fmt: *const u8,
    fmt_len: i64,
    vals: *const i64,
    lens: *const i64,
    kinds: *const i64,
    n: i64,
) {
    if fmt.is_null() || fmt_len < 0 {
        return;
    }
    let fmt = std::slice::from_raw_parts(fmt, fmt_len as usize);
    let mut i = 0usize;
    while i < fmt.len() {
        if fmt[i] == b'$' && i + 1 < fmt.len() && (b'1'..=b'9').contains(&fmt[i + 1]) {
            let idx = (fmt[i + 1] - b'1') as i64;
            i += 2;
            if idx >= 0 && idx < n {
                let kinds_s = std::slice::from_raw_parts(kinds, n as usize);
                let vals_s = std::slice::from_raw_parts(vals, n as usize);
                let lens_s = std::slice::from_raw_parts(lens, n as usize);
                if kinds_s[idx as usize] == 0 {
                    print!("{}", vals_s[idx as usize]);
                } else if kinds_s[idx as usize] == 2 {
                    let bits = vals_s[idx as usize] as u64;
                    print!("{}", f64::from_bits(bits));
                } else {
                    let ptr = vals_s[idx as usize] as *const u8;
                    if !ptr.is_null() {
                        let len = if lens_s[idx as usize] < 0 {
                            libc_strlen(ptr)
                        } else {
                            lens_s[idx as usize] as usize
                        };
                        let s = std::slice::from_raw_parts(ptr, len);
                        print!("{}", String::from_utf8_lossy(s));
                    }
                }
            }
            continue;
        }
        if fmt[i] == b'$' && i + 1 < fmt.len() && fmt[i + 1] == b'$' {
            print!("$");
            i += 2;
            continue;
        }
        print!("{}", fmt[i] as char);
        i += 1;
    }
    println!();
}

unsafe fn libc_strlen(ptr: *const u8) -> usize {
    let mut n = 0usize;
    while *ptr.add(n) != 0 {
        n += 1;
    }
    n
}

fn cstr_to_string(ptr: i64) -> String {
    if ptr == 0 {
        return String::new();
    }
    unsafe {
        let p = ptr as *const u8;
        let len = libc_strlen(p);
        String::from_utf8_lossy(std::slice::from_raw_parts(p, len)).into_owned()
    }
}

fn string_to_cstr(s: String) -> i64 {
    let mut bytes = s.into_bytes();
    bytes.push(0);
    let len = bytes.len();
    unsafe {
        let p = malloc(len) as *mut u8;
        if p.is_null() {
            panic!("oom");
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, len);
        p as i64
    }
}

fn make_tagged(tag: i64, payload: i64) -> i64 {
    unsafe {
        let p = malloc(16) as *mut i64;
        if p.is_null() {
            panic!("oom");
        }
        *p = tag;
        *p.add(1) = payload;
        p as i64
    }
}

// --- argv for std.env.args ---
static ARGV: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[no_mangle]
pub unsafe extern "C" fn stk_set_argv(argc: i64, argv: *const i64) {
    let mut g = ARGV.lock().unwrap();
    g.clear();
    if argv.is_null() || argc <= 0 {
        return;
    }
    let slice = std::slice::from_raw_parts(argv, argc as usize);
    for &p in slice {
        g.push(cstr_to_string(p));
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_panic(msg: i64) {
    let s = cstr_to_string(msg);
    eprintln!("panic: {s}");
    std::process::abort();
}

#[no_mangle]
pub unsafe extern "C" fn stk_process_exit(code: i64) {
    std::process::exit(code as i32);
}

#[no_mangle]
pub unsafe extern "C" fn stk_time_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

struct StkList {
    items: Mutex<Vec<i64>>,
}

#[no_mangle]
pub unsafe extern "C" fn stk_list_new() -> i64 {
    let l = Box::new(StkList {
        items: Mutex::new(Vec::new()),
    });
    Box::into_raw(l) as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_list_push(list: i64, value: i64) {
    let l = &*(list as *const StkList);
    l.items.lock().unwrap().push(value);
}

#[no_mangle]
pub unsafe extern "C" fn stk_list_get(list: i64, index: i64) -> i64 {
    let l = &*(list as *const StkList);
    let g = l.items.lock().unwrap();
    if index < 0 || index as usize >= g.len() {
        eprintln!("stk: List.get out of bounds");
        std::process::abort();
    }
    g[index as usize]
}

#[no_mangle]
pub unsafe extern "C" fn stk_list_set(list: i64, index: i64, value: i64) {
    let l = &*(list as *const StkList);
    let mut g = l.items.lock().unwrap();
    if index < 0 || index as usize >= g.len() {
        eprintln!("stk: List.set out of bounds");
        std::process::abort();
    }
    g[index as usize] = value;
}

#[no_mangle]
pub unsafe extern "C" fn stk_list_len(list: i64) -> i64 {
    let l = &*(list as *const StkList);
    l.items.lock().unwrap().len() as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_env_args() -> i64 {
    let list = stk_list_new();
    let args = ARGV.lock().unwrap().clone();
    for a in args {
        stk_list_push(list, string_to_cstr(a));
    }
    list
}

#[no_mangle]
pub unsafe extern "C" fn stk_env_get(name: i64) -> i64 {
    let key = cstr_to_string(name);
    match std::env::var(&key) {
        Ok(v) => make_tagged(0, string_to_cstr(v)),
        Err(_) => make_tagged(1, 0),
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_env_set(name: i64, value: i64) {
    let k = cstr_to_string(name);
    let v = cstr_to_string(value);
    std::env::set_var(k, v);
}

#[no_mangle]
pub unsafe extern "C" fn stk_fs_read_to_string(path: i64) -> i64 {
    let p = cstr_to_string(path);
    match std::fs::read_to_string(&p) {
        Ok(s) => make_tagged(0, string_to_cstr(s)),
        Err(e) => make_tagged(1, string_to_cstr(e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_fs_write_string(path: i64, contents: i64) -> i64 {
    let p = cstr_to_string(path);
    let c = cstr_to_string(contents);
    match std::fs::write(&p, c) {
        Ok(()) => make_tagged(0, 0),
        Err(e) => make_tagged(1, string_to_cstr(e.to_string())),
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_string_len(s: i64) -> i64 {
    cstr_to_string(s).len() as i64
}

#[no_mangle]
pub unsafe extern "C" fn stk_string_concat(a: i64, b: i64) -> i64 {
    let mut s = cstr_to_string(a);
    s.push_str(&cstr_to_string(b));
    string_to_cstr(s)
}

#[no_mangle]
pub unsafe extern "C" fn stk_string_slice(s: i64, start: i64, end: i64) -> i64 {
    let str = cstr_to_string(s);
    let len = str.len() as i64;
    let start = start.clamp(0, len) as usize;
    let end = end.clamp(0, len) as usize;
    let end = end.max(start);
    string_to_cstr(str[start..end].to_string())
}

#[no_mangle]
pub unsafe extern "C" fn stk_string_contains(hay: i64, needle: i64) -> i64 {
    if cstr_to_string(hay).contains(&cstr_to_string(needle)) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn stk_string_from_int(n: i64) -> i64 {
    string_to_cstr(n.to_string())
}

#[no_mangle]
pub unsafe extern "C" fn stk_string_parse_int(s: i64) -> i64 {
    let t = cstr_to_string(s);
    match t.parse::<i64>() {
        Ok(n) => make_tagged(0, n),
        Err(_) => make_tagged(1, string_to_cstr(format!("invalid int: {t}"))),
    }
}
