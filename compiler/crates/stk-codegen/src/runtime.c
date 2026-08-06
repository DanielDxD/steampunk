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
