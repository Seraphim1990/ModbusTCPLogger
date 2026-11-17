const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const source = fs.readFileSync(require('node:path').join(__dirname, '../static/admin.js'), 'utf8');

function environment() {
    let clock = 0, nextTimer = 0;
    const timers = new Map(), sockets = [], events = [];
    const documentEvents = {}, windowEvents = {};
    const storage = () => {
        const data = new Map();
        return { getItem: key => data.get(key) ?? null,
            setItem: (key, value) => data.set(key, String(value)), removeItem: key => data.delete(key) };
    };
    const epoch = 1800000000000;
    const token = seconds => `header.${Buffer.from(JSON.stringify({ exp: (epoch + clock) / 1000 + seconds })).toString('base64url')}.signature`;
    const timer = (fn, ms, period = 0) => {
        const id = ++nextTimer;
        timers.set(id, { fn, at: clock + ms, period });
        return id;
    };
    class Socket {
        static OPEN = 1;
        constructor(url) { this.url = url; this.sent = []; this.readyState = 0; sockets.push(this); }
        send(message) { if (this.readyState !== 1) throw Error('closed'); this.sent.push(message); }
        open() { this.readyState = 1; this.onopen?.({}); }
        fail(code = 1006) { this.readyState = 3; this.onclose?.({ code }); }
        close() { this.fail(1000); }
        message(data) { this.onmessage?.({ data }); }
    }
    let loginShown = false;
    const context = vm.createContext({
        console, AbortController, WebSocket: Socket,
        Date: class extends Date { static now() { return epoch + clock; } },
        atob: value => Buffer.from(value, 'base64').toString('binary'),
        setTimeout: (fn, ms) => timer(fn, ms), clearTimeout: id => timers.delete(id),
        setInterval: (fn, ms) => timer(fn, ms, ms), clearInterval: id => timers.delete(id),
        sessionStorage: storage(), localStorage: storage(),
        location: { protocol: 'http:', host: 'localhost' },
        document: { hidden: false, addEventListener: (name, fn) => { documentEvents[name] = fn; },
            getElementById: () => ({ classList: { remove: () => { loginShown = true; } } }) },
        window: { addEventListener: (name, fn) => { windowEvents[name] = fn; } },
        fetch: async () => ({ ok: true, status: 200, json: async () => ({ access_token: token(900), refresh_token: 'refresh' }) }),
    });
    vm.runInContext(source + `\n globalThis.api = {
        state, auth, connectViewWebSocket, disconnectViewWebSocket, tryRefresh, goToLogin,
        loadViewPanel, onGroupChange,
        setup(id = 1) { state.currentMode = 'view'; state.currentGroupId = id;
            viewData = { nodes: [id], devices: [id + 10] }; },
        installHandlers(fn) { handleValueEvents = handleNodeEvents = handleDeviceEvents = fn; },
        setFetchPanel(fn) { apiFetch = fn; renderSubgroupList = () => {}; },
    };`, context);
    const api = context.api;
    api.auth.setTokens(token(900), 'refresh');
    api.setup();
    api.installHandlers(data => events.push(data));
    const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
    return { api, sockets, events, context, token, timers, documentEvents, windowEvents,
        get loginShown() { return loginShown; },
        connect: () => api.connectViewWebSocket([{ values: [{ value_tag: 'sensor' }] }]),
        flush,
        async advance(ms) {
            const until = clock + ms;
            while (true) {
                let selected;
                for (const entry of timers) {
                    if (entry[1].at <= until && (!selected || entry[1].at < selected[1].at)) selected = entry;
                }
                if (!selected) break;
                const [id, task] = selected;
                clock = task.at;
                if (task.period) task.at += task.period; else timers.delete(id);
                task.fn();
                await flush();
            }
            clock = until;
            await flush();
        },
    };
}

for (const code of [1000, 1001, 1006]) {
    test(`close ${code} reconnects and restores values, nodes and devices`, async () => {
        const env = environment();
        env.connect();
        env.sockets[0].open();
        const subscription = env.sockets[0].sent[0];
        env.sockets[0].fail(code);
        await env.advance(1000);
        assert.equal(env.sockets.length, 2);
        env.sockets[1].open();
        assert.equal(env.sockets[1].sent[0], subscription);
        env.sockets[1].message('{"values":[{"tag":"sensor","value":42}]}');
        assert.equal(env.events.length, 1);
    });
}

test('late callbacks from old sockets cannot clear or reconnect a new socket', async () => {
    const env = environment();
    env.connect();
    const old = env.sockets[0], oldClose = old.onclose, oldMessage = old.onmessage;
    env.api.setup(2);
    env.connect();
    const current = env.sockets[1];
    oldClose({ code: 1000 });
    oldMessage({ data: '{"values":[1]}' });
    await env.flush();
    assert.equal(env.api.state.ws, current);
    assert.equal(env.events.length, 0);
    assert.equal(env.sockets.length, 2);
});

test('intentional disconnect cancels pending retries', async () => {
    const env = environment();
    env.connect();
    env.sockets[0].fail();
    env.api.disconnectViewWebSocket();
    await env.advance(120000);
    assert.equal(env.sockets.length, 1);
    assert.equal(env.timers.size, 0);
});

test('half-open connections reconnect when pong does not arrive', async () => {
    const env = environment();
    env.connect(); env.sockets[0].open();
    await env.advance(20000);
    assert.equal(env.sockets[0].sent.at(-1), 'ping');
    await env.advance(11000);
    assert.equal(env.sockets.length, 2);
});

test('pong keeps a quiet subscription connected', async () => {
    const env = environment();
    env.connect(); env.sockets[0].open();
    await env.advance(20000);
    env.sockets[0].message('pong');
    await env.advance(11000);
    assert.equal(env.sockets.length, 1);
    assert.equal(env.events.length, 0);
});

test('network and server refresh failures preserve credentials and retry', async () => {
    for (const status of [0, 500]) {
        const env = environment();
        env.api.auth.setTokens(env.token(-1), 'refresh');
        env.context.fetch = async () => {
            if (status === 0) throw Error('offline');
            return { ok: false, status };
        };
        env.connect(); await env.flush();
        assert.equal(env.api.auth.getRefresh(), 'refresh');
        assert.equal(env.loginShown, false);
        env.context.fetch = async () => ({ ok: true, json: async () => ({ access_token: env.token(900) }) });
        await env.advance(1000);
        assert.equal(env.sockets.length, 1);
    }
});

test('revoked refresh token stops reconnects and opens login', async () => {
    const env = environment();
    env.api.auth.setTokens(env.token(-1), 'refresh');
    env.context.fetch = async () => ({ ok: false, status: 401 });
    env.connect(); await env.flush(); await env.advance(60000);
    assert.equal(env.api.auth.getRefresh(), null);
    assert.equal(env.sockets.length, 0);
    assert.equal(env.loginShown, true);
});

test('logout during refresh cannot restore credentials or create a socket', async () => {
    const env = environment();
    let resolve;
    env.context.fetch = () => new Promise(done => { resolve = done; });
    env.api.auth.setTokens(env.token(-1), 'refresh');
    env.connect();
    env.api.goToLogin();
    resolve({ ok: true, json: async () => ({ access_token: env.token(900), refresh_token: 'refresh' }) });
    await env.flush();
    assert.equal(env.api.auth.getRefresh(), null);
    assert.equal(env.sockets.length, 0);
});

test('a connection stuck during handshake is retried', async () => {
    const env = environment();
    env.connect();
    await env.advance(16000);
    assert.equal(env.sockets.length, 2);
});

test('late group responses cannot replace the current subscription', async () => {
    const env = environment();
    const responses = [];
    env.api.setFetchPanel(() => new Promise(resolve => responses.push(resolve)));
    const first = env.api.loadViewPanel(1);
    env.api.setup(2);
    const second = env.api.loadViewPanel(2);
    responses[1]({ sub_groups: [{ values: [{ value_tag: 'new' }] }], nodes: [2], devices: [12] });
    await second;
    responses[0]({ sub_groups: [{ values: [{ value_tag: 'old' }] }], nodes: [1], devices: [11] });
    await first;
    assert.equal(env.sockets.length, 1);
    env.sockets[0].open();
    assert.deepEqual(JSON.parse(env.sockets[0].sent[0]), { values: ['new'], nodes: [2], devices: [12] });
});

test('repeated failures back off to thirty seconds without a tight retry loop', async () => {
    const env = environment();
    let attempts = 0;
    env.api.auth.setTokens(env.token(-1), 'refresh');
    env.context.fetch = async () => { attempts++; throw Error('offline'); };
    env.connect(); await env.flush();
    await env.advance(31000);
    assert.equal(attempts, 6); // 0, 1, 3, 7, 15, 31 seconds
    await env.advance(29999);
    assert.equal(attempts, 6);
    await env.advance(1);
    assert.equal(attempts, 7);
});

test('a stalled refresh is aborted and credentials survive the timeout', async () => {
    const env = environment();
    env.api.auth.setTokens(env.token(-1), 'refresh');
    env.context.fetch = (_, options) => new Promise((_, reject) => {
        options.signal.addEventListener('abort', () => reject(Error('aborted')));
    });
    env.connect(); await env.advance(10000);
    assert.equal(env.api.auth.getRefresh(), 'refresh');
    assert.equal(env.loginShown, false);
    assert.equal(env.sockets.length, 0);
    assert.equal(env.timers.size, 1); // retry, not another stuck fetch
});

test('concurrent refresh calls share one request', async () => {
    const env = environment();
    let calls = 0, resolve;
    env.context.fetch = () => { calls++; return new Promise(done => { resolve = done; }); };
    const first = env.api.tryRefresh(), second = env.api.tryRefresh();
    resolve({ ok: true, json: async () => ({ access_token: env.token(900) }) });
    assert.deepEqual(await Promise.all([first, second]), [true, true]);
    assert.equal(calls, 1);
});

test('returning online wakes a pending reconnect immediately', async () => {
    const env = environment();
    env.connect(); env.sockets[0].fail();
    env.windowEvents.online(); await env.flush();
    assert.equal(env.sockets.length, 2);
    env.sockets[1].open();
    await env.advance(1000);
    assert.equal(env.sockets.length, 2);
});
