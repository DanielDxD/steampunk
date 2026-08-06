#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <pthread.h>
#include <sched.h>
#include <time.h>
#include <unistd.h>

void stk_sleep_ms(int64_t ms) {
    if (ms <= 0) {
        return;
    }
    struct timespec ts;
    ts.tv_sec = (time_t)(ms / 1000);
    ts.tv_nsec = (long)((ms % 1000) * 1000000L);
    nanosleep(&ts, NULL);
}

void *stk_alloc(int64_t size) {
    if (size <= 0) {
        size = 8;
    }
    void *p = calloc(1, (size_t)size);
    return p;
}

void stk_free(void *p) {
    free(p);
}

/* ---- Future (pending / complete) ---- */

struct stk_future {
    pthread_mutex_t mu;
    pthread_cond_t cv;
    int ready;
    int64_t value;
};

int64_t stk_future_new(void) {
    struct stk_future *f = (struct stk_future *)calloc(1, sizeof(struct stk_future));
    if (!f) {
        abort();
    }
    pthread_mutex_init(&f->mu, NULL);
    pthread_cond_init(&f->cv, NULL);
    return (int64_t)(uintptr_t)f;
}

void stk_future_complete(int64_t handle, int64_t value) {
    struct stk_future *f = (struct stk_future *)(uintptr_t)handle;
    pthread_mutex_lock(&f->mu);
    if (!f->ready) {
        f->value = value;
        f->ready = 1;
        pthread_cond_broadcast(&f->cv);
    }
    pthread_mutex_unlock(&f->mu);
}

int64_t stk_future_ready(int64_t value) {
    int64_t h = stk_future_new();
    stk_future_complete(h, value);
    return h;
}

static int64_t stk_future_get(int64_t handle) {
    struct stk_future *f = (struct stk_future *)(uintptr_t)handle;
    pthread_mutex_lock(&f->mu);
    while (!f->ready) {
        pthread_cond_wait(&f->cv, &f->mu);
    }
    int64_t v = f->value;
    pthread_mutex_unlock(&f->mu);
    return v;
}

int64_t stk_future_await(int64_t handle) {
    /* Intentionally does not free: race losers may still complete() a handle
       after the winner was already awaited. MVP accepts the leak. */
    return stk_future_get(handle);
}

int64_t stk_future_join(int64_t h1, int64_t h2) {
    int64_t a = stk_future_await(h1);
    int64_t b = stk_future_await(h2);
    int64_t *arr = (int64_t *)malloc(sizeof(int64_t) * 2);
    if (!arr) {
        abort();
    }
    arr[0] = a;
    arr[1] = b;
    return stk_future_ready((int64_t)(uintptr_t)arr);
}

struct stk_race_job {
    int64_t src;
    int64_t dest;
};

static void *stk_race_entry(void *arg) {
    struct stk_race_job *j = (struct stk_race_job *)arg;
    /* Non-destroying wait: both racers may observe the same sources. */
    int64_t v = stk_future_get(j->src);
    stk_future_complete(j->dest, v);
    free(j);
    return NULL;
}

int64_t stk_future_race(int64_t h1, int64_t h2) {
    int64_t dest = stk_future_new();
    struct stk_race_job *j1 = (struct stk_race_job *)malloc(sizeof(struct stk_race_job));
    struct stk_race_job *j2 = (struct stk_race_job *)malloc(sizeof(struct stk_race_job));
    if (!j1 || !j2) {
        abort();
    }
    j1->src = h1;
    j1->dest = dest;
    j2->src = h2;
    j2->dest = dest;
    pthread_t t1, t2;
    if (pthread_create(&t1, NULL, stk_race_entry, j1) != 0) {
        abort();
    }
    if (pthread_create(&t2, NULL, stk_race_entry, j2) != 0) {
        abort();
    }
    pthread_detach(t1);
    pthread_detach(t2);
    return dest;
}

/* ---- spawn ---- */

static volatile int stk_spawn_live = 0;
static pthread_mutex_t stk_spawn_mu = PTHREAD_MUTEX_INITIALIZER;

typedef void (*stk_thunk_fn)(int64_t env);

struct stk_thunk {
    stk_thunk_fn fn;
    int64_t env;
};

static void *stk_spawn_entry(void *arg) {
    struct stk_thunk *t = (struct stk_thunk *)arg;
    t->fn(t->env);
    free(t);
    pthread_mutex_lock(&stk_spawn_mu);
    stk_spawn_live -= 1;
    pthread_mutex_unlock(&stk_spawn_mu);
    return NULL;
}

void stk_spawn(int64_t fn_ptr, int64_t env) {
    struct stk_thunk *t = (struct stk_thunk *)malloc(sizeof(struct stk_thunk));
    if (!t) {
        abort();
    }
    t->fn = (stk_thunk_fn)(uintptr_t)fn_ptr;
    t->env = env;
    pthread_mutex_lock(&stk_spawn_mu);
    stk_spawn_live += 1;
    pthread_mutex_unlock(&stk_spawn_mu);
    pthread_t th;
    if (pthread_create(&th, NULL, stk_spawn_entry, t) != 0) {
        abort();
    }
    pthread_detach(th);
}

void stk_spawn_drain(void) {
    for (;;) {
        pthread_mutex_lock(&stk_spawn_mu);
        int live = stk_spawn_live;
        pthread_mutex_unlock(&stk_spawn_mu);
        if (live <= 0) {
            break;
        }
        sched_yield();
    }
}

/* ---- Channel ---- */

struct stk_channel {
    pthread_mutex_t mu;
    pthread_cond_t not_empty;
    pthread_cond_t not_full;
    int64_t *buf;
    size_t cap;
    size_t len;
    size_t head;
    /* 0 = unbounded; >0 = buffered capacity */
    size_t max_len;
    int closed;
};

static int64_t stk_channel_create(size_t max_len) {
    struct stk_channel *ch = (struct stk_channel *)calloc(1, sizeof(struct stk_channel));
    if (!ch) {
        abort();
    }
    pthread_mutex_init(&ch->mu, NULL);
    pthread_cond_init(&ch->not_empty, NULL);
    pthread_cond_init(&ch->not_full, NULL);
    ch->max_len = max_len;
    ch->cap = max_len > 0 ? max_len : 8;
    ch->buf = (int64_t *)malloc(sizeof(int64_t) * ch->cap);
    if (!ch->buf) {
        abort();
    }
    return (int64_t)(uintptr_t)ch;
}

int64_t stk_channel_new(void) {
    return stk_channel_create(0);
}

int64_t stk_channel_buffered(int64_t n) {
    if (n < 1) {
        fprintf(stderr, "stk: Channel.buffered requires n >= 1\n");
        abort();
    }
    return stk_channel_create((size_t)n);
}

void stk_channel_send(int64_t handle, int64_t value) {
    struct stk_channel *ch = (struct stk_channel *)(uintptr_t)handle;
    pthread_mutex_lock(&ch->mu);
    if (ch->closed) {
        pthread_mutex_unlock(&ch->mu);
        fprintf(stderr, "stk: send on closed channel\n");
        abort();
    }
    while (ch->max_len > 0 && ch->len >= ch->max_len) {
        pthread_cond_wait(&ch->not_full, &ch->mu);
        if (ch->closed) {
            pthread_mutex_unlock(&ch->mu);
            fprintf(stderr, "stk: send on closed channel\n");
            abort();
        }
    }
    if (ch->max_len == 0 && ch->len == ch->cap) {
        size_t ncap = ch->cap * 2;
        int64_t *nbuf = (int64_t *)malloc(sizeof(int64_t) * ncap);
        if (!nbuf) {
            abort();
        }
        for (size_t i = 0; i < ch->len; i++) {
            nbuf[i] = ch->buf[(ch->head + i) % ch->cap];
        }
        free(ch->buf);
        ch->buf = nbuf;
        ch->cap = ncap;
        ch->head = 0;
    }
    ch->buf[(ch->head + ch->len) % ch->cap] = value;
    ch->len += 1;
    pthread_cond_signal(&ch->not_empty);
    pthread_mutex_unlock(&ch->mu);
}

void stk_channel_close(int64_t handle) {
    struct stk_channel *ch = (struct stk_channel *)(uintptr_t)handle;
    pthread_mutex_lock(&ch->mu);
    ch->closed = 1;
    pthread_cond_broadcast(&ch->not_empty);
    pthread_cond_broadcast(&ch->not_full);
    pthread_mutex_unlock(&ch->mu);
}

int64_t stk_channel_recv_ok(int64_t handle, int64_t *out) {
    struct stk_channel *ch = (struct stk_channel *)(uintptr_t)handle;
    pthread_mutex_lock(&ch->mu);
    while (ch->len == 0 && !ch->closed) {
        pthread_cond_wait(&ch->not_empty, &ch->mu);
    }
    if (ch->len == 0) {
        pthread_mutex_unlock(&ch->mu);
        return 0;
    }
    int64_t v = ch->buf[ch->head];
    ch->head = (ch->head + 1) % ch->cap;
    ch->len -= 1;
    pthread_cond_signal(&ch->not_full);
    pthread_mutex_unlock(&ch->mu);
    if (out) {
        *out = v;
    }
    return 1;
}

int64_t stk_channel_recv(int64_t handle) {
    int64_t v = 0;
    if (!stk_channel_recv_ok(handle, &v)) {
        fprintf(stderr, "stk: recv on closed empty channel\n");
        abort();
    }
    return v;
}

struct stk_recv_job {
    int64_t ch;
    int64_t dest;
};

static void *stk_recv_future_entry(void *arg) {
    struct stk_recv_job *j = (struct stk_recv_job *)arg;
    int64_t v = stk_channel_recv(j->ch);
    stk_future_complete(j->dest, v);
    free(j);
    pthread_mutex_lock(&stk_spawn_mu);
    stk_spawn_live -= 1;
    pthread_mutex_unlock(&stk_spawn_mu);
    return NULL;
}

int64_t stk_channel_recv_future(int64_t handle) {
    int64_t dest = stk_future_new();
    struct stk_recv_job *j = (struct stk_recv_job *)malloc(sizeof(struct stk_recv_job));
    if (!j) {
        abort();
    }
    j->ch = handle;
    j->dest = dest;
    pthread_mutex_lock(&stk_spawn_mu);
    stk_spawn_live += 1;
    pthread_mutex_unlock(&stk_spawn_mu);
    pthread_t th;
    if (pthread_create(&th, NULL, stk_recv_future_entry, j) != 0) {
        abort();
    }
    pthread_detach(th);
    return dest;
}

/* ---- WaitGroup ---- */

struct stk_waitgroup {
    pthread_mutex_t mu;
    pthread_cond_t cv;
    int64_t count;
};

int64_t stk_waitgroup_new(void) {
    struct stk_waitgroup *wg = (struct stk_waitgroup *)calloc(1, sizeof(struct stk_waitgroup));
    if (!wg) {
        abort();
    }
    pthread_mutex_init(&wg->mu, NULL);
    pthread_cond_init(&wg->cv, NULL);
    return (int64_t)(uintptr_t)wg;
}

void stk_waitgroup_add(int64_t handle, int64_t delta) {
    struct stk_waitgroup *wg = (struct stk_waitgroup *)(uintptr_t)handle;
    pthread_mutex_lock(&wg->mu);
    wg->count += delta;
    if (wg->count < 0) {
        pthread_mutex_unlock(&wg->mu);
        fprintf(stderr, "stk: WaitGroup counter negative\n");
        abort();
    }
    if (wg->count == 0) {
        pthread_cond_broadcast(&wg->cv);
    }
    pthread_mutex_unlock(&wg->mu);
}

void stk_waitgroup_done(int64_t handle) {
    stk_waitgroup_add(handle, -1);
}

void stk_waitgroup_wait(int64_t handle) {
    struct stk_waitgroup *wg = (struct stk_waitgroup *)(uintptr_t)handle;
    pthread_mutex_lock(&wg->mu);
    while (wg->count > 0) {
        pthread_cond_wait(&wg->cv, &wg->mu);
    }
    pthread_mutex_unlock(&wg->mu);
}

struct stk_wait_job {
    int64_t wg;
    int64_t dest;
};

static void *stk_wait_future_entry(void *arg) {
    struct stk_wait_job *j = (struct stk_wait_job *)arg;
    stk_waitgroup_wait(j->wg);
    stk_future_complete(j->dest, 0);
    free(j);
    pthread_mutex_lock(&stk_spawn_mu);
    stk_spawn_live -= 1;
    pthread_mutex_unlock(&stk_spawn_mu);
    return NULL;
}

int64_t stk_waitgroup_wait_future(int64_t handle) {
    int64_t dest = stk_future_new();
    struct stk_wait_job *j = (struct stk_wait_job *)malloc(sizeof(struct stk_wait_job));
    if (!j) {
        abort();
    }
    j->wg = handle;
    j->dest = dest;
    pthread_mutex_lock(&stk_spawn_mu);
    stk_spawn_live += 1;
    pthread_mutex_unlock(&stk_spawn_mu);
    pthread_t th;
    if (pthread_create(&th, NULL, stk_wait_future_entry, j) != 0) {
        abort();
    }
    pthread_detach(th);
    return dest;
}

/* ---- Mutex ---- */

struct stk_mutex {
    pthread_mutex_t mu;
    int64_t value;
};

int64_t stk_mutex_new(int64_t initial) {
    struct stk_mutex *m = (struct stk_mutex *)calloc(1, sizeof(struct stk_mutex));
    if (!m) {
        abort();
    }
    pthread_mutex_init(&m->mu, NULL);
    m->value = initial;
    return (int64_t)(uintptr_t)m;
}

void stk_mutex_lock(int64_t handle) {
    struct stk_mutex *m = (struct stk_mutex *)(uintptr_t)handle;
    pthread_mutex_lock(&m->mu);
}

void stk_mutex_unlock(int64_t handle) {
    struct stk_mutex *m = (struct stk_mutex *)(uintptr_t)handle;
    pthread_mutex_unlock(&m->mu);
}

int64_t stk_mutex_get(int64_t handle) {
    struct stk_mutex *m = (struct stk_mutex *)(uintptr_t)handle;
    return m->value;
}

void stk_mutex_set(int64_t handle, int64_t value) {
    struct stk_mutex *m = (struct stk_mutex *)(uintptr_t)handle;
    m->value = value;
}

/* ---- strings, lists, env, fs, time ---- */

static char *stk_str_dup(const char *s) {
    if (!s) {
        s = "";
    }
    size_t n = strlen(s);
    char *p = (char *)malloc(n + 1);
    if (!p) {
        abort();
    }
    memcpy(p, s, n + 1);
    return p;
}

/* Boxed { tag, payload } used for Result/Option values. */
static int64_t stk_tagged(int64_t tag, int64_t payload) {
    int64_t *p = (int64_t *)malloc(sizeof(int64_t) * 2);
    if (!p) {
        abort();
    }
    p[0] = tag;
    p[1] = payload;
    return (int64_t)(uintptr_t)p;
}

/* Shortest positional form that round-trips, matching the JIT runtime's output
   (Rust's `{}` for f64 never switches to exponent notation). */
static void stk_print_double(double d) {
    char buf[512];
    if (d != d || d - d != 0.0) {
        snprintf(buf, sizeof buf, "%s", d != d ? "NaN" : (d > 0 ? "inf" : "-inf"));
        fputs(buf, stdout);
        return;
    }
    for (int decimals = 0; decimals <= 340; decimals++) {
        snprintf(buf, sizeof buf, "%.*f", decimals, d);
        if (strtod(buf, NULL) == d) {
            fputs(buf, stdout);
            return;
        }
    }
    snprintf(buf, sizeof buf, "%.17g", d);
    fputs(buf, stdout);
}

struct stk_list {
    pthread_mutex_t mu;
    int64_t *items;
    size_t len;
    size_t cap;
};

int64_t stk_list_new(void) {
    struct stk_list *l = (struct stk_list *)calloc(1, sizeof(struct stk_list));
    if (!l) {
        abort();
    }
    pthread_mutex_init(&l->mu, NULL);
    l->cap = 8;
    l->items = (int64_t *)malloc(sizeof(int64_t) * l->cap);
    if (!l->items) {
        abort();
    }
    return (int64_t)(uintptr_t)l;
}

void stk_list_push(int64_t handle, int64_t value) {
    struct stk_list *l = (struct stk_list *)(uintptr_t)handle;
    pthread_mutex_lock(&l->mu);
    if (l->len == l->cap) {
        size_t ncap = l->cap * 2;
        int64_t *nb = (int64_t *)realloc(l->items, sizeof(int64_t) * ncap);
        if (!nb) {
            abort();
        }
        l->items = nb;
        l->cap = ncap;
    }
    l->items[l->len++] = value;
    pthread_mutex_unlock(&l->mu);
}

int64_t stk_list_get(int64_t handle, int64_t index) {
    struct stk_list *l = (struct stk_list *)(uintptr_t)handle;
    pthread_mutex_lock(&l->mu);
    if (index < 0 || (size_t)index >= l->len) {
        pthread_mutex_unlock(&l->mu);
        fprintf(stderr, "stk: List.get out of bounds\n");
        abort();
    }
    int64_t v = l->items[index];
    pthread_mutex_unlock(&l->mu);
    return v;
}

void stk_list_set(int64_t handle, int64_t index, int64_t value) {
    struct stk_list *l = (struct stk_list *)(uintptr_t)handle;
    pthread_mutex_lock(&l->mu);
    if (index < 0 || (size_t)index >= l->len) {
        pthread_mutex_unlock(&l->mu);
        fprintf(stderr, "stk: List.set out of bounds\n");
        abort();
    }
    l->items[index] = value;
    pthread_mutex_unlock(&l->mu);
}

int64_t stk_list_len(int64_t handle) {
    struct stk_list *l = (struct stk_list *)(uintptr_t)handle;
    pthread_mutex_lock(&l->mu);
    int64_t n = (int64_t)l->len;
    pthread_mutex_unlock(&l->mu);
    return n;
}

static char **stk_argv = NULL;
static int64_t stk_argc = 0;

void stk_set_argv(int64_t argc, const int64_t *argv) {
    if (stk_argv) {
        for (int64_t i = 0; i < stk_argc; i++) {
            free(stk_argv[i]);
        }
        free(stk_argv);
        stk_argv = NULL;
        stk_argc = 0;
    }
    if (!argv || argc <= 0) {
        return;
    }
    stk_argv = (char **)calloc((size_t)argc, sizeof(char *));
    if (!stk_argv) {
        abort();
    }
    for (int64_t i = 0; i < argc; i++) {
        stk_argv[i] = stk_str_dup((const char *)(uintptr_t)argv[i]);
    }
    stk_argc = argc;
}

/* glibc and Apple libc both hand argc/argv to constructors. */
__attribute__((constructor)) static void stk_capture_argv(int argc, char **argv) {
    if (argc <= 0 || !argv) {
        return;
    }
    stk_argv = (char **)calloc((size_t)argc, sizeof(char *));
    if (!stk_argv) {
        return;
    }
    for (int i = 0; i < argc; i++) {
        stk_argv[i] = stk_str_dup(argv[i]);
    }
    stk_argc = argc;
}

int64_t stk_env_args(void) {
    int64_t list = stk_list_new();
    for (int64_t i = 0; i < stk_argc; i++) {
        stk_list_push(list, (int64_t)(uintptr_t)stk_str_dup(stk_argv[i]));
    }
    return list;
}

int64_t stk_env_get(int64_t name) {
    const char *k = (const char *)(uintptr_t)name;
    const char *v = k ? getenv(k) : NULL;
    if (!v) {
        return stk_tagged(1, 0);
    }
    return stk_tagged(0, (int64_t)(uintptr_t)stk_str_dup(v));
}

void stk_env_set(int64_t name, int64_t value) {
    const char *k = (const char *)(uintptr_t)name;
    const char *v = (const char *)(uintptr_t)value;
    if (!k) {
        return;
    }
    setenv(k, v ? v : "", 1);
}

void stk_panic(int64_t msg) {
    const char *s = (const char *)(uintptr_t)msg;
    fprintf(stderr, "panic: %s\n", s ? s : "");
    fflush(stderr);
    abort();
}

void stk_process_exit(int64_t code) {
    fflush(stdout);
    exit((int)code);
}

int64_t stk_time_now_ms(void) {
    struct timespec ts;
    if (clock_gettime(CLOCK_REALTIME, &ts) != 0) {
        return 0;
    }
    return (int64_t)ts.tv_sec * 1000 + (int64_t)(ts.tv_nsec / 1000000L);
}

int64_t stk_fs_read_to_string(int64_t path) {
    const char *p = (const char *)(uintptr_t)path;
    FILE *fp = p ? fopen(p, "rb") : NULL;
    if (!fp) {
        return stk_tagged(1, (int64_t)(uintptr_t)stk_str_dup(strerror(errno)));
    }
    size_t cap = 4096;
    size_t len = 0;
    char *buf = (char *)malloc(cap);
    if (!buf) {
        fclose(fp);
        abort();
    }
    for (;;) {
        if (len + 1024 > cap) {
            cap *= 2;
            char *nb = (char *)realloc(buf, cap);
            if (!nb) {
                free(buf);
                fclose(fp);
                abort();
            }
            buf = nb;
        }
        size_t got = fread(buf + len, 1, 1024, fp);
        len += got;
        if (got < 1024) {
            break;
        }
    }
    int failed = ferror(fp);
    fclose(fp);
    if (failed) {
        free(buf);
        return stk_tagged(1, (int64_t)(uintptr_t)stk_str_dup("read error"));
    }
    buf[len] = '\0';
    return stk_tagged(0, (int64_t)(uintptr_t)buf);
}

int64_t stk_fs_write_string(int64_t path, int64_t contents) {
    const char *p = (const char *)(uintptr_t)path;
    const char *c = (const char *)(uintptr_t)contents;
    FILE *fp = p ? fopen(p, "wb") : NULL;
    if (!fp) {
        return stk_tagged(1, (int64_t)(uintptr_t)stk_str_dup(strerror(errno)));
    }
    size_t n = c ? strlen(c) : 0;
    size_t wrote = n > 0 ? fwrite(c, 1, n, fp) : 0;
    int failed = (wrote != n) || fclose(fp) != 0;
    if (failed) {
        return stk_tagged(1, (int64_t)(uintptr_t)stk_str_dup("write error"));
    }
    return stk_tagged(0, 0);
}

int64_t stk_string_len(int64_t s) {
    const char *p = (const char *)(uintptr_t)s;
    return p ? (int64_t)strlen(p) : 0;
}

int64_t stk_string_concat(int64_t a, int64_t b) {
    const char *x = (const char *)(uintptr_t)a;
    const char *y = (const char *)(uintptr_t)b;
    if (!x) {
        x = "";
    }
    if (!y) {
        y = "";
    }
    size_t nx = strlen(x);
    size_t ny = strlen(y);
    char *p = (char *)malloc(nx + ny + 1);
    if (!p) {
        abort();
    }
    memcpy(p, x, nx);
    memcpy(p + nx, y, ny + 1);
    return (int64_t)(uintptr_t)p;
}

int64_t stk_string_slice(int64_t s, int64_t start, int64_t end) {
    const char *p = (const char *)(uintptr_t)s;
    if (!p) {
        p = "";
    }
    int64_t n = (int64_t)strlen(p);
    if (start < 0) {
        start = 0;
    }
    if (start > n) {
        start = n;
    }
    if (end < 0) {
        end = 0;
    }
    if (end > n) {
        end = n;
    }
    if (end < start) {
        end = start;
    }
    size_t len = (size_t)(end - start);
    char *out = (char *)malloc(len + 1);
    if (!out) {
        abort();
    }
    memcpy(out, p + start, len);
    out[len] = '\0';
    return (int64_t)(uintptr_t)out;
}

int64_t stk_string_contains(int64_t hay, int64_t needle) {
    const char *h = (const char *)(uintptr_t)hay;
    const char *n = (const char *)(uintptr_t)needle;
    if (!h) {
        h = "";
    }
    if (!n) {
        n = "";
    }
    return strstr(h, n) != NULL ? 1 : 0;
}

int64_t stk_string_from_int(int64_t n) {
    char buf[32];
    snprintf(buf, sizeof buf, "%lld", (long long)n);
    return (int64_t)(uintptr_t)stk_str_dup(buf);
}

int64_t stk_string_parse_int(int64_t s) {
    const char *p = (const char *)(uintptr_t)s;
    if (!p || *p == '\0') {
        return stk_tagged(1, (int64_t)(uintptr_t)stk_str_dup("invalid int: "));
    }
    char *endp = NULL;
    errno = 0;
    long long v = strtoll(p, &endp, 10);
    if (errno != 0 || !endp || *endp != '\0') {
        char buf[128];
        snprintf(buf, sizeof buf, "invalid int: %s", p);
        return stk_tagged(1, (int64_t)(uintptr_t)stk_str_dup(buf));
    }
    return stk_tagged(0, (int64_t)v);
}

void stk_std_log(
    const uint8_t *fmt,
    int64_t fmt_len,
    const int64_t *vals,
    const int64_t *lens,
    const int64_t *kinds,
    int64_t n
) {
    int64_t i = 0;
    while (i < fmt_len) {
        if (fmt[i] == '$' && i + 1 < fmt_len && fmt[i + 1] >= '1' && fmt[i + 1] <= '9') {
            int idx = fmt[i + 1] - '1';
            i += 2;
            if (idx >= 0 && idx < n) {
                if (kinds[idx] == 0) {
                    printf("%lld", (long long)vals[idx]);
                } else if (kinds[idx] == 2) {
                    /* float: value carries the raw IEEE-754 bits */
                    double d;
                    uint64_t bits = (uint64_t)vals[idx];
                    memcpy(&d, &bits, sizeof(d));
                    stk_print_double(d);
                } else {
                    const char *s = (const char *)(uintptr_t)vals[idx];
                    int64_t len = lens[idx];
                    if (len < 0) {
                        fputs(s, stdout);
                    } else {
                        fwrite(s, 1, (size_t)len, stdout);
                    }
                }
            }
            continue;
        }
        if (fmt[i] == '$' && i + 1 < fmt_len && fmt[i + 1] == '$') {
            fputc('$', stdout);
            i += 2;
            continue;
        }
        fputc((int)fmt[i], stdout);
        i += 1;
    }
    fputc('\n', stdout);
    fflush(stdout);
}

/* ---- RwLock / parallel / http / task (AOT stubs — prefer Rust JIT runtime) ---- */

int64_t stk_rwlock_new(int64_t initial);
void stk_rwlock_read_lock(int64_t handle);
void stk_rwlock_read_unlock(int64_t handle);
void stk_rwlock_write_lock(int64_t handle);
void stk_rwlock_write_unlock(int64_t handle);
int64_t stk_rwlock_get(int64_t handle);
void stk_rwlock_set(int64_t handle, int64_t value);
int64_t stk_parallel_map_int(int64_t list, int64_t fn_ptr);
int64_t stk_http_get(int64_t url);
void stk_task_yield(void);
int64_t stk_cancel_token_new(void);
void stk_cancel_token_cancel(int64_t handle);
int64_t stk_cancel_token_is_cancelled(int64_t handle);

/* Minimal AOT implementations: mutex-shaped rwlock, serial map, http unsupported. */

struct stk_rwlock {
    pthread_mutex_t mu;
    int64_t value;
};

int64_t stk_rwlock_new(int64_t initial) {
    struct stk_rwlock *m = calloc(1, sizeof(*m));
    pthread_mutex_init(&m->mu, NULL);
    m->value = initial;
    return (int64_t)(uintptr_t)m;
}
void stk_rwlock_read_lock(int64_t handle) {
    struct stk_rwlock *m = (struct stk_rwlock *)(uintptr_t)handle;
    pthread_mutex_lock(&m->mu);
}
void stk_rwlock_read_unlock(int64_t handle) {
    struct stk_rwlock *m = (struct stk_rwlock *)(uintptr_t)handle;
    pthread_mutex_unlock(&m->mu);
}
void stk_rwlock_write_lock(int64_t handle) { stk_rwlock_read_lock(handle); }
void stk_rwlock_write_unlock(int64_t handle) { stk_rwlock_read_unlock(handle); }
int64_t stk_rwlock_get(int64_t handle) {
    struct stk_rwlock *m = (struct stk_rwlock *)(uintptr_t)handle;
    return m->value;
}
void stk_rwlock_set(int64_t handle, int64_t value) {
    struct stk_rwlock *m = (struct stk_rwlock *)(uintptr_t)handle;
    m->value = value;
}

int64_t stk_parallel_map_int(int64_t list, int64_t fn_ptr) {
    typedef int64_t (*fn_t)(int64_t);
    fn_t f = (fn_t)(uintptr_t)fn_ptr;
    int64_t n = stk_list_len(list);
    int64_t out = stk_list_new();
    for (int64_t i = 0; i < n; i++) {
        stk_list_push(out, f(stk_list_get(list, i)));
    }
    return out;
}

int64_t stk_http_get(int64_t url) {
    (void)url;
    return stk_tagged(1, (int64_t)(uintptr_t)stk_str_dup("AOT http.get not implemented"));
}

/* ---- typed serde (schema-driven; mirrors JIT serde_rt.rs) ---- */

typedef struct StkBuf {
    char *data;
    size_t len;
    size_t cap;
} StkBuf;

static void stk_buf_init(StkBuf *b) {
    b->data = NULL;
    b->len = 0;
    b->cap = 0;
}

static int stk_buf_reserve(StkBuf *b, size_t need) {
    if (b->len + need + 1 <= b->cap) return 0;
    size_t ncap = b->cap ? b->cap * 2 : 64;
    while (ncap < b->len + need + 1) ncap *= 2;
    char *p = (char *)realloc(b->data, ncap);
    if (!p) return -1;
    b->data = p;
    b->cap = ncap;
    return 0;
}

static int stk_buf_append(StkBuf *b, const char *s, size_t n) {
    if (stk_buf_reserve(b, n) != 0) return -1;
    memcpy(b->data + b->len, s, n);
    b->len += n;
    b->data[b->len] = 0;
    return 0;
}

static int stk_buf_putc(StkBuf *b, char c) {
    return stk_buf_append(b, &c, 1);
}

static int stk_buf_puts(StkBuf *b, const char *s) {
    return stk_buf_append(b, s, strlen(s));
}

static char *stk_buf_finish(StkBuf *b) {
    if (!b->data) return stk_str_dup("");
    char *out = b->data;
    b->data = NULL;
    b->len = b->cap = 0;
    return out;
}

static void stk_buf_free(StkBuf *b) {
    free(b->data);
    b->data = NULL;
    b->len = b->cap = 0;
}

typedef enum {
    SCH_INT,
    SCH_FLOAT,
    SCH_STR,
    SCH_BOOL,
    SCH_OPT,
    SCH_LIST,
    SCH_CLASS
} SchKind;

typedef struct Schema Schema;
struct Schema {
    SchKind kind;
    Schema *inner; /* opt/list */
    int64_t size;  /* class */
    int nfields;
    char **wires;
    Schema **ftys;
    int64_t *offs;
};

static const char *sch_expect(const char *s, const char *pfx) {
    size_t n = strlen(pfx);
    if (strncmp(s, pfx, n) != 0) return NULL;
    return s + n;
}

static const char *sch_parse(const char *s, Schema **out);

static const char *sch_parse_field_end(const char *s, char *offbuf, size_t offbuf_sz) {
    size_t i = 0;
    while (s[i] && s[i] != ',' && s[i] != ')') {
        if (i + 1 < offbuf_sz) offbuf[i] = s[i];
        i++;
    }
    offbuf[i < offbuf_sz ? i : offbuf_sz - 1] = 0;
    return s + i;
}

static const char *sch_parse(const char *s, Schema **out) {
    while (*s == ' ' || *s == '\t') s++;
    if (!*s) return NULL;
    Schema *sch = (Schema *)calloc(1, sizeof(Schema));
    if (!sch) return NULL;
    if (*s == 'i') {
        sch->kind = SCH_INT;
        *out = sch;
        return s + 1;
    }
    if (*s == 'f') {
        sch->kind = SCH_FLOAT;
        *out = sch;
        return s + 1;
    }
    if (*s == 's') {
        sch->kind = SCH_STR;
        *out = sch;
        return s + 1;
    }
    if (*s == 'b') {
        sch->kind = SCH_BOOL;
        *out = sch;
        return s + 1;
    }
    if (*s == 'o') {
        const char *r = sch_expect(s, "o(");
        if (!r) {
            free(sch);
            return NULL;
        }
        Schema *inner = NULL;
        r = sch_parse(r, &inner);
        if (!r || *r != ')') {
            free(sch);
            return NULL;
        }
        sch->kind = SCH_OPT;
        sch->inner = inner;
        *out = sch;
        return r + 1;
    }
    if (*s == 'L') {
        const char *r = sch_expect(s, "L(");
        if (!r) {
            free(sch);
            return NULL;
        }
        Schema *inner = NULL;
        r = sch_parse(r, &inner);
        if (!r || *r != ')') {
            free(sch);
            return NULL;
        }
        sch->kind = SCH_LIST;
        sch->inner = inner;
        *out = sch;
        return r + 1;
    }
    if (*s == 'C') {
        s++;
        char *endp = NULL;
        long size = strtol(s, &endp, 10);
        if (endp == s || *endp != '(') {
            free(sch);
            return NULL;
        }
        s = endp + 1;
        sch->kind = SCH_CLASS;
        sch->size = (int64_t)size;
        if (*s == ')') {
            *out = sch;
            return s + 1;
        }
        while (1) {
            const char *colon = strchr(s, ':');
            if (!colon) {
                free(sch);
                return NULL;
            }
            size_t wlen = (size_t)(colon - s);
            char *wire = (char *)malloc(wlen + 1);
            memcpy(wire, s, wlen);
            wire[wlen] = 0;
            s = colon + 1;
            Schema *fty = NULL;
            s = sch_parse(s, &fty);
            if (!s || *s != ':') {
                free(wire);
                free(sch);
                return NULL;
            }
            s++;
            char offbuf[32];
            s = sch_parse_field_end(s, offbuf, sizeof offbuf);
            int64_t off = (int64_t)strtoll(offbuf, NULL, 10);
            int n = sch->nfields + 1;
            sch->wires = (char **)realloc(sch->wires, (size_t)n * sizeof(char *));
            sch->ftys = (Schema **)realloc(sch->ftys, (size_t)n * sizeof(Schema *));
            sch->offs = (int64_t *)realloc(sch->offs, (size_t)n * sizeof(int64_t));
            sch->wires[sch->nfields] = wire;
            sch->ftys[sch->nfields] = fty;
            sch->offs[sch->nfields] = off;
            sch->nfields = n;
            if (*s == ',') {
                s++;
                continue;
            }
            if (*s == ')') {
                *out = sch;
                return s + 1;
            }
            free(sch);
            return NULL;
        }
    }
    free(sch);
    return NULL;
}

static void sch_free(Schema *sch) {
    if (!sch) return;
    if (sch->inner) sch_free(sch->inner);
    for (int i = 0; i < sch->nfields; i++) {
        free(sch->wires[i]);
        sch_free(sch->ftys[i]);
    }
    free(sch->wires);
    free(sch->ftys);
    free(sch->offs);
    free(sch);
}

static int64_t stk_load_i64(int64_t obj, int64_t off) {
    return *(int64_t *)((uint8_t *)(uintptr_t)obj + (size_t)off);
}

static void stk_store_i64(int64_t obj, int64_t off, int64_t v) {
    *(int64_t *)((uint8_t *)(uintptr_t)obj + (size_t)off) = v;
}

/* Value IR for format writers/parsers */
typedef enum { V_NULL, V_BOOL, V_INT, V_FLOAT, V_STR, V_ARR, V_OBJ } VKind;

typedef struct Val Val;
struct Val {
    VKind kind;
    int64_t i;
    double f;
    char *s;
    Val **items;
    int nitems;
    char **keys;
    Val **vals;
    int nobj;
};

static Val *val_null(void) {
    Val *v = (Val *)calloc(1, sizeof(Val));
    v->kind = V_NULL;
    return v;
}
static Val *val_bool(int b) {
    Val *v = (Val *)calloc(1, sizeof(Val));
    v->kind = V_BOOL;
    v->i = b ? 1 : 0;
    return v;
}
static Val *val_int(int64_t n) {
    Val *v = (Val *)calloc(1, sizeof(Val));
    v->kind = V_INT;
    v->i = n;
    return v;
}
static Val *val_float(double f) {
    Val *v = (Val *)calloc(1, sizeof(Val));
    v->kind = V_FLOAT;
    v->f = f;
    return v;
}
static Val *val_str(char *s) {
    Val *v = (Val *)calloc(1, sizeof(Val));
    v->kind = V_STR;
    v->s = s;
    return v;
}
static void val_free(Val *v) {
    if (!v) return;
    free(v->s);
    for (int i = 0; i < v->nitems; i++) val_free(v->items[i]);
    free(v->items);
    for (int i = 0; i < v->nobj; i++) {
        free(v->keys[i]);
        val_free(v->vals[i]);
    }
    free(v->keys);
    free(v->vals);
    free(v);
}

static Val *encode_val(Schema *sch, int64_t ptr);
static int64_t decode_val(Schema *sch, Val *v);

static Val *encode_val(Schema *sch, int64_t ptr) {
    switch (sch->kind) {
    case SCH_INT:
        return val_int(ptr);
    case SCH_FLOAT: {
        double d;
        uint64_t bits = (uint64_t)ptr;
        memcpy(&d, &bits, sizeof(d));
        return val_float(d);
    }
    case SCH_STR:
        return val_str(stk_str_dup((const char *)(uintptr_t)ptr));
    case SCH_BOOL:
        return val_bool(ptr != 0);
    case SCH_OPT: {
        int64_t tag = stk_load_i64(ptr, 0);
        int64_t payload = stk_load_i64(ptr, 8);
        if (tag == 1) return val_null();
        return encode_val(sch->inner, payload);
    }
    case SCH_LIST: {
        Val *arr = (Val *)calloc(1, sizeof(Val));
        arr->kind = V_ARR;
        int64_t len = stk_list_len(ptr);
        for (int64_t i = 0; i < len; i++) {
            Val *el = encode_val(sch->inner, stk_list_get(ptr, i));
            arr->items = (Val **)realloc(arr->items, (size_t)(arr->nitems + 1) * sizeof(Val *));
            arr->items[arr->nitems++] = el;
        }
        return arr;
    }
    case SCH_CLASS: {
        Val *obj = (Val *)calloc(1, sizeof(Val));
        obj->kind = V_OBJ;
        for (int i = 0; i < sch->nfields; i++) {
            int64_t slot = stk_load_i64(ptr, sch->offs[i]);
            Val *fv = encode_val(sch->ftys[i], slot);
            obj->keys = (char **)realloc(obj->keys, (size_t)(obj->nobj + 1) * sizeof(char *));
            obj->vals = (Val **)realloc(obj->vals, (size_t)(obj->nobj + 1) * sizeof(Val *));
            obj->keys[obj->nobj] = stk_str_dup(sch->wires[i]);
            obj->vals[obj->nobj] = fv;
            obj->nobj++;
        }
        return obj;
    }
    }
    return val_null();
}

static int64_t decode_val(Schema *sch, Val *v) {
    switch (sch->kind) {
    case SCH_INT:
        if (v->kind == V_INT) return v->i;
        return 0;
    case SCH_FLOAT:
        if (v->kind == V_FLOAT) {
            uint64_t bits;
            memcpy(&bits, &v->f, sizeof(bits));
            return (int64_t)bits;
        }
        if (v->kind == V_INT) {
            double d = (double)v->i;
            uint64_t bits;
            memcpy(&bits, &d, sizeof(bits));
            return (int64_t)bits;
        }
        return 0;
    case SCH_STR:
        if (v->kind == V_STR) return (int64_t)(uintptr_t)stk_str_dup(v->s ? v->s : "");
        return (int64_t)(uintptr_t)stk_str_dup("");
    case SCH_BOOL:
        if (v->kind == V_BOOL) return v->i;
        return 0;
    case SCH_OPT: {
        int64_t p = (int64_t)(uintptr_t)stk_alloc(16);
        if (v->kind == V_NULL) {
            stk_store_i64(p, 0, 1);
            stk_store_i64(p, 8, 0);
            return p;
        }
        stk_store_i64(p, 0, 0);
        stk_store_i64(p, 8, decode_val(sch->inner, v));
        return p;
    }
    case SCH_LIST: {
        int64_t list = stk_list_new();
        if (v->kind == V_ARR) {
            for (int i = 0; i < v->nitems; i++) {
                stk_list_push(list, decode_val(sch->inner, v->items[i]));
            }
        }
        return list;
    }
    case SCH_CLASS: {
        int64_t obj = (int64_t)(uintptr_t)stk_alloc(sch->size);
        if (v->kind != V_OBJ) return obj;
        for (int i = 0; i < sch->nfields; i++) {
            Val *fv = NULL;
            for (int j = 0; j < v->nobj; j++) {
                if (strcmp(v->keys[j], sch->wires[i]) == 0) {
                    fv = v->vals[j];
                    break;
                }
            }
            if (!fv) {
                if (sch->ftys[i]->kind == SCH_OPT) {
                    int64_t p = (int64_t)(uintptr_t)stk_alloc(16);
                    stk_store_i64(p, 0, 1);
                    stk_store_i64(p, 8, 0);
                    stk_store_i64(obj, sch->offs[i], p);
                }
                continue;
            }
            stk_store_i64(obj, sch->offs[i], decode_val(sch->ftys[i], fv));
        }
        return obj;
    }
    }
    return 0;
}

static int json_escape_to(StkBuf *b, const char *s) {
    if (stk_buf_putc(b, '"') != 0) return -1;
    for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
        if (*p == '"' || *p == '\\') {
            if (stk_buf_putc(b, '\\') != 0 || stk_buf_putc(b, (char)*p) != 0) return -1;
        } else if (*p == '\n') {
            if (stk_buf_puts(b, "\\n") != 0) return -1;
        } else if (*p == '\r') {
            if (stk_buf_puts(b, "\\r") != 0) return -1;
        } else if (*p == '\t') {
            if (stk_buf_puts(b, "\\t") != 0) return -1;
        } else {
            if (stk_buf_putc(b, (char)*p) != 0) return -1;
        }
    }
    return stk_buf_putc(b, '"');
}

static int val_to_json(StkBuf *b, Val *v) {
    char num[64];
    switch (v->kind) {
    case V_NULL:
        return stk_buf_puts(b, "null");
    case V_BOOL:
        return stk_buf_puts(b, v->i ? "true" : "false");
    case V_INT:
        snprintf(num, sizeof num, "%lld", (long long)v->i);
        return stk_buf_puts(b, num);
    case V_FLOAT:
        snprintf(num, sizeof num, "%.17g", v->f);
        if (!strchr(num, '.') && !strchr(num, 'e') && !strchr(num, 'E')) {
            strcat(num, ".0");
        }
        return stk_buf_puts(b, num);
    case V_STR:
        return json_escape_to(b, v->s ? v->s : "");
    case V_ARR: {
        if (stk_buf_putc(b, '[') != 0) return -1;
        for (int i = 0; i < v->nitems; i++) {
            if (i && stk_buf_putc(b, ',') != 0) return -1;
            if (val_to_json(b, v->items[i]) != 0) return -1;
        }
        return stk_buf_putc(b, ']');
    }
    case V_OBJ: {
        if (stk_buf_putc(b, '{') != 0) return -1;
        for (int i = 0; i < v->nobj; i++) {
            if (i && stk_buf_putc(b, ',') != 0) return -1;
            if (json_escape_to(b, v->keys[i]) != 0) return -1;
            if (stk_buf_putc(b, ':') != 0) return -1;
            if (val_to_json(b, v->vals[i]) != 0) return -1;
        }
        return stk_buf_putc(b, '}');
    }
    }
    return -1;
}

typedef struct {
    const char *s;
    size_t i;
    size_t n;
} Jp;

static int jp_peek(Jp *p) {
    return p->i < p->n ? (unsigned char)p->s[p->i] : -1;
}
static void jp_bump(Jp *p) {
    if (p->i < p->n) p->i++;
}
static void jp_skip(Jp *p) {
    while (1) {
        int c = jp_peek(p);
        if (c == ' ' || c == '\n' || c == '\r' || c == '\t') jp_bump(p);
        else break;
    }
}
static int jp_eat(Jp *p, const char *lit) {
    for (; *lit; lit++) {
        if (jp_peek(p) != (unsigned char)*lit) return -1;
        jp_bump(p);
    }
    return 0;
}

static Val *jp_parse_value(Jp *p);

static Val *jp_parse_string(Jp *p) {
    if (jp_eat(p, "\"") != 0) return NULL;
    StkBuf b;
    stk_buf_init(&b);
    while (1) {
        int c = jp_peek(p);
        if (c < 0) {
            stk_buf_free(&b);
            return NULL;
        }
        if (c == '"') {
            jp_bump(p);
            char *s = stk_buf_finish(&b);
            return val_str(s);
        }
        if (c == '\\') {
            jp_bump(p);
            int e = jp_peek(p);
            if (e < 0) {
                stk_buf_free(&b);
                return NULL;
            }
            char ch = (char)e;
            if (e == 'n') ch = '\n';
            else if (e == 'r') ch = '\r';
            else if (e == 't') ch = '\t';
            jp_bump(p);
            if (stk_buf_putc(&b, ch) != 0) {
                stk_buf_free(&b);
                return NULL;
            }
        } else {
            jp_bump(p);
            if (stk_buf_putc(&b, (char)c) != 0) {
                stk_buf_free(&b);
                return NULL;
            }
        }
    }
}

static Val *jp_parse_number(Jp *p) {
    size_t start = p->i;
    if (jp_peek(p) == '-') jp_bump(p);
    while (jp_peek(p) >= '0' && jp_peek(p) <= '9') jp_bump(p);
    int is_float = 0;
    if (jp_peek(p) == '.') {
        is_float = 1;
        jp_bump(p);
        while (jp_peek(p) >= '0' && jp_peek(p) <= '9') jp_bump(p);
    }
    if (jp_peek(p) == 'e' || jp_peek(p) == 'E') {
        is_float = 1;
        jp_bump(p);
        if (jp_peek(p) == '+' || jp_peek(p) == '-') jp_bump(p);
        while (jp_peek(p) >= '0' && jp_peek(p) <= '9') jp_bump(p);
    }
    char tmp[128];
    size_t len = p->i - start;
    if (len >= sizeof tmp) return NULL;
    memcpy(tmp, p->s + start, len);
    tmp[len] = 0;
    if (is_float) return val_float(strtod(tmp, NULL));
    return val_int((int64_t)strtoll(tmp, NULL, 10));
}

static Val *jp_parse_array(Jp *p) {
    if (jp_eat(p, "[") != 0) return NULL;
    jp_skip(p);
    Val *arr = (Val *)calloc(1, sizeof(Val));
    arr->kind = V_ARR;
    if (jp_peek(p) == ']') {
        jp_bump(p);
        return arr;
    }
    while (1) {
        Val *el = jp_parse_value(p);
        if (!el) {
            val_free(arr);
            return NULL;
        }
        arr->items = (Val **)realloc(arr->items, (size_t)(arr->nitems + 1) * sizeof(Val *));
        arr->items[arr->nitems++] = el;
        jp_skip(p);
        if (jp_peek(p) == ',') {
            jp_bump(p);
            continue;
        }
        if (jp_peek(p) == ']') {
            jp_bump(p);
            return arr;
        }
        val_free(arr);
        return NULL;
    }
}

static Val *jp_parse_object(Jp *p) {
    if (jp_eat(p, "{") != 0) return NULL;
    jp_skip(p);
    Val *obj = (Val *)calloc(1, sizeof(Val));
    obj->kind = V_OBJ;
    if (jp_peek(p) == '}') {
        jp_bump(p);
        return obj;
    }
    while (1) {
        jp_skip(p);
        Val *ks = jp_parse_string(p);
        if (!ks) {
            val_free(obj);
            return NULL;
        }
        jp_skip(p);
        if (jp_eat(p, ":") != 0) {
            val_free(ks);
            val_free(obj);
            return NULL;
        }
        Val *vv = jp_parse_value(p);
        if (!vv) {
            val_free(ks);
            val_free(obj);
            return NULL;
        }
        obj->keys = (char **)realloc(obj->keys, (size_t)(obj->nobj + 1) * sizeof(char *));
        obj->vals = (Val **)realloc(obj->vals, (size_t)(obj->nobj + 1) * sizeof(Val *));
        obj->keys[obj->nobj] = ks->s;
        ks->s = NULL;
        val_free(ks);
        obj->vals[obj->nobj] = vv;
        obj->nobj++;
        jp_skip(p);
        if (jp_peek(p) == ',') {
            jp_bump(p);
            continue;
        }
        if (jp_peek(p) == '}') {
            jp_bump(p);
            return obj;
        }
        val_free(obj);
        return NULL;
    }
}

static Val *jp_parse_value(Jp *p) {
    jp_skip(p);
    int c = jp_peek(p);
    if (c == 'n') {
        if (jp_eat(p, "null") != 0) return NULL;
        return val_null();
    }
    if (c == 't') {
        if (jp_eat(p, "true") != 0) return NULL;
        return val_bool(1);
    }
    if (c == 'f') {
        if (jp_eat(p, "false") != 0) return NULL;
        return val_bool(0);
    }
    if (c == '"') return jp_parse_string(p);
    if (c == '[') return jp_parse_array(p);
    if (c == '{') return jp_parse_object(p);
    if (c == '-' || (c >= '0' && c <= '9')) return jp_parse_number(p);
    return NULL;
}

static Val *parse_json_c(const char *text) {
    Jp p = {text, 0, strlen(text)};
    Val *v = jp_parse_value(&p);
    if (!v) return NULL;
    jp_skip(&p);
    if (p.i != p.n) {
        val_free(v);
        return NULL;
    }
    return v;
}

static int val_to_yaml(StkBuf *b, Val *v, int indent) {
    (void)indent;
    return val_to_json(b, v);
}

static Val *parse_yaml_scalar_c(const char *s) {
    if (!s || !*s || strcmp(s, "null") == 0 || strcmp(s, "~") == 0) return val_null();
    if (strcmp(s, "true") == 0) return val_bool(1);
    if (strcmp(s, "false") == 0) return val_bool(0);
    if (s[0] == '"' || s[0] == '[' || s[0] == '{') {
        Val *j = parse_json_c(s);
        return j ? j : val_str(stk_str_dup(s));
    }
    char *endp = NULL;
    long long n = strtoll(s, &endp, 10);
    if (endp && *endp == 0) return val_int((int64_t)n);
    double d = strtod(s, &endp);
    if (endp && *endp == 0) return val_float(d);
    return val_str(stk_str_dup(s));
}

static Val *parse_yaml_simple_c(const char *text) {
    while (*text == ' ' || *text == '\n' || *text == '\t') text++;
    if (*text == '{' || *text == '[') return parse_json_c(text);
    Val *obj = (Val *)calloc(1, sizeof(Val));
    obj->kind = V_OBJ;
    char *copy = stk_str_dup(text);
    char *save = NULL;
    for (char *line = strtok_r(copy, "\n", &save); line; line = strtok_r(NULL, "\n", &save)) {
        while (*line == ' ' || *line == '\t') line++;
        if (!*line || *line == '#') continue;
        char *colon = strchr(line, ':');
        if (!colon) continue;
        *colon = 0;
        char *key = line;
        char *val = colon + 1;
        while (*val == ' ' || *val == '\t') val++;
        while (key[0] && (key[strlen(key) - 1] == ' ' || key[strlen(key) - 1] == '\t'))
            key[strlen(key) - 1] = 0;
        obj->keys = (char **)realloc(obj->keys, (size_t)(obj->nobj + 1) * sizeof(char *));
        obj->vals = (Val **)realloc(obj->vals, (size_t)(obj->nobj + 1) * sizeof(Val *));
        obj->keys[obj->nobj] = stk_str_dup(key);
        obj->vals[obj->nobj] = parse_yaml_scalar_c(val);
        obj->nobj++;
    }
    free(copy);
    return obj;
}

static int toml_scalar(StkBuf *b, Val *v) {
    if (v->kind == V_NULL) return -1;
    if (v->kind == V_OBJ) return -1;
    if (v->kind == V_ARR) {
        if (stk_buf_putc(b, '[') != 0) return -1;
        for (int i = 0; i < v->nitems; i++) {
            if (i && stk_buf_puts(b, ", ") != 0) return -1;
            if (toml_scalar(b, v->items[i]) != 0) return -1;
        }
        return stk_buf_putc(b, ']');
    }
    return val_to_json(b, v);
}

static int val_to_toml(StkBuf *b, Val *v) {
    if (v->kind != V_OBJ) return -1;
    for (int i = 0; i < v->nobj; i++) {
        if (v->vals[i]->kind == V_NULL || v->vals[i]->kind == V_OBJ) continue;
        if (stk_buf_puts(b, v->keys[i]) != 0 || stk_buf_puts(b, " = ") != 0) return -1;
        if (toml_scalar(b, v->vals[i]) != 0) return -1;
        if (stk_buf_putc(b, '\n') != 0) return -1;
    }
    for (int i = 0; i < v->nobj; i++) {
        if (v->vals[i]->kind != V_OBJ) continue;
        if (stk_buf_puts(b, "\n[") != 0 || stk_buf_puts(b, v->keys[i]) != 0 ||
            stk_buf_puts(b, "]\n") != 0)
            return -1;
        Val *nested = v->vals[i];
        for (int j = 0; j < nested->nobj; j++) {
            if (nested->vals[j]->kind == V_NULL) continue;
            if (stk_buf_puts(b, nested->keys[j]) != 0 || stk_buf_puts(b, " = ") != 0)
                return -1;
            if (toml_scalar(b, nested->vals[j]) != 0) return -1;
            if (stk_buf_putc(b, '\n') != 0) return -1;
        }
    }
    return 0;
}

static Val *parse_toml_simple_c(const char *text) {
    Val *root = (Val *)calloc(1, sizeof(Val));
    root->kind = V_OBJ;
    char *section = NULL;
    char *copy = stk_str_dup(text);
    char *save = NULL;
    for (char *line = strtok_r(copy, "\n", &save); line; line = strtok_r(NULL, "\n", &save)) {
        while (*line == ' ' || *line == '\t') line++;
        if (!*line || *line == '#') continue;
        size_t L = strlen(line);
        if (line[0] == '[' && L >= 2 && line[L - 1] == ']') {
            free(section);
            section = (char *)malloc(L - 1);
            memcpy(section, line + 1, L - 2);
            section[L - 2] = 0;
            /* ensure section object exists */
            int found = 0;
            for (int i = 0; i < root->nobj; i++) {
                if (strcmp(root->keys[i], section) == 0) {
                    found = 1;
                    break;
                }
            }
            if (!found) {
                root->keys = (char **)realloc(root->keys, (size_t)(root->nobj + 1) * sizeof(char *));
                root->vals = (Val **)realloc(root->vals, (size_t)(root->nobj + 1) * sizeof(Val *));
                root->keys[root->nobj] = stk_str_dup(section);
                Val *nested = (Val *)calloc(1, sizeof(Val));
                nested->kind = V_OBJ;
                root->vals[root->nobj] = nested;
                root->nobj++;
            }
            continue;
        }
        char *eq = strchr(line, '=');
        if (!eq) continue;
        *eq = 0;
        char *key = line;
        char *val = eq + 1;
        while (*val == ' ' || *val == '\t') val++;
        while (key[0] && (key[strlen(key) - 1] == ' ' || key[strlen(key) - 1] == '\t'))
            key[strlen(key) - 1] = 0;
        Val *target = root;
        if (section) {
            for (int i = 0; i < root->nobj; i++) {
                if (strcmp(root->keys[i], section) == 0) {
                    target = root->vals[i];
                    break;
                }
            }
        }
        target->keys = (char **)realloc(target->keys, (size_t)(target->nobj + 1) * sizeof(char *));
        target->vals = (Val **)realloc(target->vals, (size_t)(target->nobj + 1) * sizeof(Val *));
        target->keys[target->nobj] = stk_str_dup(key);
        target->vals[target->nobj] = parse_yaml_scalar_c(val);
        target->nobj++;
    }
    free(section);
    free(copy);
    return root;
}

static int val_to_toon(StkBuf *b, Val *v, int indent) {
    (void)indent;
    return val_to_json(b, v);
}

static Val *parse_toon_c(const char *text) {
    while (*text == ' ' || *text == '\n' || *text == '\t') text++;
    if (*text == '{' || *text == '[') {
        Val *j = parse_json_c(text);
        if (j) return j;
    }
    return parse_yaml_simple_c(text);
}

int64_t stk_serde_encode(int64_t format, int64_t schema, int64_t value) {
    const char *schs = (const char *)(uintptr_t)schema;
    Schema *sch = NULL;
    if (!sch_parse(schs, &sch) || !sch) {
        return (int64_t)(uintptr_t)stk_str_dup("");
    }
    Val *v = encode_val(sch, value);
    StkBuf b;
    stk_buf_init(&b);
    int rc = -1;
    if (format == 1)
        rc = val_to_yaml(&b, v, 0);
    else if (format == 2)
        rc = val_to_toml(&b, v);
    else if (format == 3)
        rc = val_to_toon(&b, v, 0);
    else
        rc = val_to_json(&b, v);
    val_free(v);
    sch_free(sch);
    if (rc != 0) {
        stk_buf_free(&b);
        return (int64_t)(uintptr_t)stk_str_dup("");
    }
    return (int64_t)(uintptr_t)stk_buf_finish(&b);
}

int64_t stk_serde_decode(int64_t format, int64_t schema, int64_t text) {
    const char *schs = (const char *)(uintptr_t)schema;
    const char *t = (const char *)(uintptr_t)text;
    Schema *sch = NULL;
    const char *rest = sch_parse(schs, &sch);
    if (!rest || *rest || !sch) {
        return stk_tagged(1, (int64_t)(uintptr_t)stk_str_dup("bad schema"));
    }
    Val *v = NULL;
    if (format == 1)
        v = parse_yaml_simple_c(t);
    else if (format == 2)
        v = parse_toml_simple_c(t);
    else if (format == 3)
        v = parse_toon_c(t);
    else
        v = parse_json_c(t);
    if (!v) {
        sch_free(sch);
        return stk_tagged(1, (int64_t)(uintptr_t)stk_str_dup("parse error"));
    }
    int64_t out = decode_val(sch, v);
    val_free(v);
    sch_free(sch);
    return stk_tagged(0, out);
}

void stk_task_yield(void) { sched_yield(); }

int64_t stk_cancel_token_new(void) {
    int64_t *t = calloc(1, sizeof(int64_t));
    return (int64_t)(uintptr_t)t;
}
void stk_cancel_token_cancel(int64_t handle) {
    int64_t *t = (int64_t *)(uintptr_t)handle;
    *t = 1;
}
int64_t stk_cancel_token_is_cancelled(int64_t handle) {
    int64_t *t = (int64_t *)(uintptr_t)handle;
    return *t;
}
