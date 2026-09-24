// =====================================================
// admin.js — логіка адмін панелі
// =====================================================

const API = ''; // базовий URL, якщо потрібен префікс — додай тут

let viewChart = null;
let viewData  = null;

const ROLES = {
    1: 'Адміністратор',
    2: 'Супер-юзер',
    3: 'Користувач',
    4: 'Сервіс',
};


// Зберігаємо діапазон годин графіку в localStorage
const CHART_HOURS_KEY = 'chart_hours';
function getChartHours() { return parseInt(localStorage.getItem(CHART_HOURS_KEY) ?? '24'); }
function saveChartHours(h) { localStorage.setItem(CHART_HOURS_KEY, String(h)); }

// =====================================================
// СТАН ДОДАТКУ
// =====================================================
const state = {
    currentUser:    null,   // { id, name, role }
    currentGroupId: null,   // вибрана група
    currentMode:    'view', // view | physical | logical
    ws:             null,   // WebSocket
    subgroupValues: {},     // subgroup_id → [value_id, ...]
    nodes:          [],     // кеш нод
    devices:        [],     // кеш пристроїв
    values:         [],     // кеш value_units
};

// =====================================================
// ІНІЦІАЛІЗАЦІЯ
// =====================================================
document.addEventListener('DOMContentLoaded', async () => {
    initLogin();

    const hasToken   = !!auth.getAccess();
    const hasRefresh = !!auth.getRefresh();

    if (!hasToken && !hasRefresh) {
        document.getElementById('login-overlay').classList.remove('hidden');
        return;
    }

    if (!hasToken && hasRefresh) {
        const ok = await tryRefresh();
        if (!ok) {
            document.getElementById('login-overlay').classList.remove('hidden');
            return;
        }
    }

    await initApp();
});

async function initApp() {
    document.getElementById('login-overlay').classList.add('hidden');

    const user = await apiFetch('/auth/me');
    if (!user || user.__error) { goToLogin(); return; }
    loadUser({ id: user.id, name: user.name, role: user.role_id === 1 ? 'admin' : 'user' });

    initModeTabs();
    initViewPanel();
    initPhysicalTabs();
    initLogicalTabs();
    initModalClose();
    initConfirmCheckbox();
    initFooter();



    bindClick('btn-add-node',     () => openNodeForm());
    bindClick('btn-add-user',     () => openUserForm());
    bindClick('btn-add-group',    () => openGroupForm());
    bindClick('btn-add-subgroup', () => openSubgroupForm());

    bindChange('assign-user-select',     onAssignUserChange);
    bindChange('assign-subgroup-select', onAssignSubgroupChange);
    bindClick('btn-assign-groups',  saveAssignGroups);
    bindClick('btn-clean-groups',   cleanAssignGroups);
    bindClick('btn-assign-values',  saveAssignValues);
    bindClick('btn-clean-values',   cleanAssignValues);

    const logoutBtn = document.getElementById('btn-logout');
    if (logoutBtn) {
        logoutBtn.replaceWith(logoutBtn.cloneNode(true)); // знімаємо старі обробники
        document.getElementById('btn-logout').addEventListener('click', onLogout);
    }

    await loadGroups();
    const grpSel = document.getElementById('group-select');
    grpSel.addEventListener('change', onGroupChange);
    // Якщо автовибір вже виставив значення — тригеримо
    if (grpSel.value) grpSel.dispatchEvent(new Event('change'));
}

function initViewPanel() {
    const sel = document.getElementById('chart-hours-select');
    if (!sel) return;
    sel.value = getChartHours();
    sel.addEventListener('change', () => {
        saveChartHours(parseInt(sel.value));
        if (viewChart && viewChart._currentValues) {
            loadChart(viewChart._currentValues, viewChart._currentTitle);
        }
    });
}


// =====================================================
// КОРИСТУВАЧ
// =====================================================
// =====================================================
// ЛОГІН
// =====================================================
function initLogin() {
    const btn = document.getElementById('login-btn');
    const usernameEl = document.getElementById('login-username');
    const passwordEl = document.getElementById('login-password');
    const errorEl = document.getElementById('login-error');

    const doLogin = async () => {
        const username = usernameEl.value.trim();
        const password = passwordEl.value;
        if (!username || !password) return;

        btn.disabled = true;
        btn.textContent = 'Вхід...';
        errorEl.classList.add('hidden');

        try {
            const res = await fetch(`${API}/auth/login`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ username, password }),
            });

            if (res.status === 401) {
                errorEl.textContent = 'Невірний логін або пароль';
                errorEl.classList.remove('hidden');
                return;
            }
            if (!res.ok) {
                errorEl.textContent = 'Помилка сервера, спробуйте ще раз';
                errorEl.classList.remove('hidden');
                return;
            }

            const data = await res.json();
            auth.setTokens(data.access_token, data.refresh_token);
            passwordEl.value = '';
            // await initApp();
            window.location.reload();
        } catch {
            errorEl.textContent = 'Не вдалось підключитись до сервера';
            errorEl.classList.remove('hidden');
        } finally {
            btn.disabled = false;
            btn.textContent = 'Увійти';
        }
    };

    btn.addEventListener('click', doLogin);
    passwordEl.addEventListener('keydown', e => { if (e.key === 'Enter') doLogin(); });
    usernameEl.addEventListener('keydown', e => { if (e.key === 'Enter') passwordEl.focus(); });
}

function loadUser(user) {
    state.currentUser = user;
    document.getElementById('user-name').textContent = user.name;
    document.getElementById('user-role').textContent = user.role === 'admin' ? 'адміністратор' : 'користувач';
    const initials = user.name.split('.').map(p => p[0]?.toUpperCase() ?? '').join('').slice(0, 2);
    document.getElementById('user-avatar').textContent = initials;

    // Режим-бар тільки для адміна (role_id === 1)
    const isAdmin = user.role === 'admin' || user.role === 1;
    if (!isAdmin) {
        document.getElementById('mode-bar').classList.add('hidden');
    }
}

async function onLogout() {
    const refreshToken = auth.getRefresh();
    if (refreshToken) {
        await fetch(`${API}/auth/logout`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ refresh_token: refreshToken }),
        }).catch(() => {});
    }
    auth.clear();
    if (state.ws) { state.ws.close(); state.ws = null; }
    goToLogin();
}

// =====================================================
// ГРУПИ (комбобокс)
// =====================================================
async function loadGroups() {
    // /user_group/me повертає групи поточного юзера по токену
    const groups = await apiFetch('/user_group/me');
    if (!groups || groups.__error) return;

    const sel = document.getElementById('group-select');
    sel.innerHTML = '<option value="">— оберіть групу —</option>';
    groups.forEach(g => {
        const opt = document.createElement('option');
        opt.value = g.id;
        opt.textContent = g.group_name;
        sel.appendChild(opt);
    });

    // Якщо є тільки одна група — вибираємо автоматично
    // dispatchEvent після підключення обробника (в initApp)
    if (groups.length === 1) {
        sel.value = groups[0].id;
    }
}

/*
async function onGroupChange(e) {
    const groupId = parseInt(e.target.value);
    if (!groupId) return;
    state.currentGroupId = groupId;

    // Перепідписуємо вебсокет на нову групу
    if (state.currentMode === 'view') {
        await loadViewPanel(groupId);
    }
    // В інших режимах алерт-стрічка оновлюється через WS
}
 */
async function onGroupChange(e) {
    const groupId = parseInt(e.target.value);
    if (!groupId) return;
    state.currentGroupId = groupId;

    // 1. Спочатку повністю вичищаємо все старе
    destroyChartCleanly();
    clearAlertsAndLeds();
    viewData = null; // Жорстке скидання, щоб старі підгрупи не висіли в пам'яті!

    if (state.currentMode === 'view') {
        // loadViewPanel сама зробить apiFetch і запустить новий сокет з новими даними
        await loadViewPanel(groupId);
    } else {
        // Якщо ми в конфігурації — просто гасимо сокет і не ганяємо байти вхолосту
        disconnectViewWebSocket();
    }
}
function clearAlertsAndLeds() {
    // 1. Очищаємо стрічку алертів
    const strip = document.getElementById('alert-strip');
    if (strip) strip.innerHTML = '';

    // 2. Скидаємо всі LED-індикатори пристроїв у дереві до стану за замовчуванням
    document.querySelectorAll('.led.some-part, .led.failed').forEach(led => {
        led.className = 'led'; // повертаємо дефолтний сірий/зелений клас
    });
}
// =====================================================
// ПЕРЕМИКАЧ РЕЖИМІВ
// =====================================================
// Безпечне підключення подій
function bindClick(id, fn) {
    const el = document.getElementById(id);
    if (el) el.addEventListener('click', fn);
}
function bindChange(id, fn) {
    const el = document.getElementById(id);
    if (el) el.addEventListener('change', fn);
}

function initModeTabs() {
    document.querySelectorAll('.mode-tab').forEach(btn => {
        btn.addEventListener('click', () => {
            document.querySelectorAll('.mode-tab').forEach(t => t.classList.remove('active'));
            btn.classList.add('active');

            const mode = btn.dataset.mode;
            state.currentMode = mode;

            document.querySelectorAll('.panel').forEach(p => p.classList.add('hidden'));
            document.getElementById(`panel-${mode}`).classList.remove('hidden');

            if (mode === 'physical') loadPhysicalPanel();
            if (mode === 'logical')  loadLogicalPanel();
            if (mode === 'view' && state.currentGroupId) loadViewPanel(state.currentGroupId);
        });
    });
}

// =====================================================
// ПЕРЕГЛЯД
// =====================================================


async function loadViewPanel(groupId) {
    const data = await apiFetch(`/user_group/ui/${groupId}`);
    if (!data) return;

    // Зберігаємо завантажені дані в глобальний стан, щоб інші функції мали до них доступ
    viewData = data;

    renderSubgroupList(data.sub_groups);

    // Оживляємо вебсокет: передаємо підгрупи для збору тегів і підписки
    connectViewWebSocket(data.sub_groups);
}

function renderSubgroupList(subGroups) {
    const container = document.getElementById('subgroup-list');
    container.innerHTML = '';

    subGroups.forEach(sg => {
        const item = document.createElement('div');
        item.className = 'view-subgroup';

        // Заголовок підгрупи
        const header = document.createElement('div');
        header.className = 'view-subgroup-header';
        header.innerHTML = `
            <span class="view-subgroup-arrow">▶</span>
            <span style="flex:1">${sg.group_state.subgroup_name}</span>`;

        // Контейнер values — прихований до кліку
        const valueList = document.createElement('div');
        valueList.className = 'hidden';

        // Сітка карток values
        const grid = document.createElement('div');
        grid.className = 'view-values-grid';

        sg.values.forEach(v => {
            const card = document.createElement('div');
            card.className = 'view-value-card';
            card.dataset.valueId  = v.id;
            card.dataset.valueTag = v.value_tag;

            // Чекбокс для футера
            const cb = document.createElement('input');
            cb.type = 'checkbox';
            cb.className = 'view-value-cb';
            cb.dataset.valueId   = v.id;
            cb.dataset.valueTag  = v.value_tag;
            cb.dataset.valueName = v.value_name;
            cb.addEventListener('change', updateFooterSelected);
            cb.addEventListener('click', e => e.stopPropagation());

            // Назва
            const nameSpan = document.createElement('span');
            nameSpan.className = 'view-value-card-name';
            nameSpan.textContent = v.value_name;

            // Live значення
            const liveSpan = document.createElement('span');
            liveSpan.className = 'view-value-card-live';
            liveSpan.id = `live-${v.value_tag}`;
            // liveSpan.textContent = '—';
            liveSpan.textContent = v.last_value ?? v.value ?? '—';

            card.appendChild(cb);
            card.appendChild(nameSpan);
            card.appendChild(liveSpan);

            // Клік на картку — графік для одного value
            card.addEventListener('click', () => {
                document.querySelectorAll('.view-value-card.active').forEach(r => r.classList.remove('active'));
                document.querySelectorAll('.view-subgroup-header.active').forEach(r => r.classList.remove('active'));
                card.classList.add('active');
                loadChart([v], v.value_name);
            });

            grid.appendChild(card);
        });

        valueList.appendChild(grid);

        // Клік на підгрупу — графік для всіх values підгрупи
        header.addEventListener('click', () => {
            const isOpen = header.classList.contains('open');
            header.classList.toggle('open', !isOpen);
            valueList.classList.toggle('hidden', isOpen);

            if (!isOpen) {
                document.querySelectorAll('.view-value-card.active').forEach(r => r.classList.remove('active'));
                document.querySelectorAll('.view-subgroup-header.active').forEach(r => r.classList.remove('active'));
                header.classList.add('active');
                loadChart(sg.values, sg.group_state.subgroup_name);
            }
        });

        item.appendChild(header);
        item.appendChild(valueList);
        container.appendChild(item);
    });
}

// =====================================================
// ФІЗИЧНЕ
// =====================================================
function initPhysicalTabs() {
    // Таби прибрано — використовується дерево
}

async function loadPhysicalPanel() {
    const [nodes, devices, values] = await Promise.all([
        apiFetch('/nodes/get_all'),
        apiFetch('/devices/get_all'),
        apiFetch('/values/get_all'),
    ]);
    state.nodes   = nodes   || [];
    state.devices = devices || [];
    state.values  = values  || [];
    renderPhysTree();
}

// --- Повне дерево нода → пристрій → values ---
// Оновлює тільки картку одного пристрою
async function refreshDeviceCard(deviceId) {
    const [freshDev, values] = await Promise.all([
        apiFetch(`/devices/get_device/${deviceId}`),
        apiFetch('/values/get_all'),
    ]);

    if (values && !values.__error) state.values = values;

    let dev = state.devices.find(d => d.id === deviceId);
    if (freshDev && !freshDev.__error) {
        const idx = state.devices.findIndex(d => d.id === deviceId);
        if (idx !== -1) state.devices[idx] = freshDev;
        dev = freshDev;
    }
    if (!dev) { renderPhysTree(); return; }

    const devValues = state.values.filter(v => v.parent_device_id === deviceId);
    const newCard = buildDeviceCard(dev, devValues);

    const oldCard = document.querySelector(`.phys-device[data-device-id="${deviceId}"]`);
    if (oldCard) {
        oldCard.replaceWith(newCard);
    } else {
        await refreshNodeBranch(dev.parent_node_id);
    }
}

// Оновлює гілку однієї ноди
async function refreshNodeBranch(nodeId) {
    const [devices, values] = await Promise.all([
        apiFetch('/devices/get_all'),
        apiFetch('/values/get_all'),
    ]);
    if (devices) state.devices = devices;
    if (values)  state.values  = values;

    const node = state.nodes.find(n => n.id === nodeId);
    if (!node) return;

    const nodeDevices = state.devices.filter(d => d.parent_node_id === nodeId);
    const newCard = buildNodeCard(node, nodeDevices);

    const oldCard = document.querySelector(`.phys-node[data-node-card="${nodeId}"]`);
    if (oldCard) {
        oldCard.replaceWith(newCard);
    } else {
        renderPhysTree();
    }
}

// Видаляє картку пристрою з DOM
function removeDeviceCard(deviceId) {
    const card = document.querySelector(`[data-device-id="${deviceId}"]`);
    if (card) {
        card.remove();
        state.devices = state.devices.filter(d => d.id !== deviceId);
    }
}

function renderPhysTree() {
    const tree = document.getElementById('phys-tree');
    if (!tree) return;
    tree.innerHTML = '';

    if (!Array.isArray(state.nodes)) return;
    state.nodes.forEach(node => {
        const nodeDevices = state.devices.filter(d => d.parent_node_id === node.id);
        const nodeEl = buildNodeCard(node, nodeDevices);
        tree.appendChild(nodeEl);
    });
}

function buildNodeCard(node, devices) {
    const card = document.createElement('div');
    card.className = 'phys-node';

    card.dataset.nodeCard = node.id;
    const header = document.createElement('div');
    header.className = 'phys-node-header open';
    header.dataset.nodeId = node.id;
    header.innerHTML = `
        <span class="phys-arrow">▶</span>
        <div class="led online" id="led-node-${node.id}"></div>
        <span class="phys-node-ip">${node.ip}</span>
        <span class="phys-node-meta">порт: ${node.port ?? 502}${node.description ? ' · ' + node.description : ''}</span>
        <div class="phys-node-actions">
            <button class="btn-add-sm" onclick="openDeviceForm(null, ${node.id})">+ пристрій</button>
            <button class="btn-edit" onclick="openNodeForm(${node.id})">ред.</button>
            <button class="btn-del"  onclick="deleteNode(${node.id})">вид.</button>
        </div>`;

    const body = document.createElement('div');
    body.className = 'phys-node-body';

    devices.forEach(dev => {
        const devValues = state.values.filter(v => v.parent_device_id === dev.id);
        body.appendChild(buildDeviceCard(dev, devValues));
    });

    // Якщо нема пристроїв — підказка
    if (!devices.length) {
        const empty = document.createElement('div');
        empty.style.cssText = 'color:var(--text-muted);font-size:12px;padding:6px 0';
        empty.textContent = 'Немає пристроїв';
        body.appendChild(empty);
    }

    // Розгорнути/згорнути
    header.addEventListener('click', e => {
        if (e.target.closest('button')) return;
        const open = header.classList.toggle('open');
        body.classList.toggle('hidden', !open);
    });

    card.appendChild(header);
    card.appendChild(body);
    return card;
}

function buildDeviceCard(dev, values) {
    const card = document.createElement('div');
    card.className = 'phys-device';
    card.dataset.deviceId = dev.id;

    const activeClass = dev.is_active ? 'badge-on' : 'badge-off';
    const activeLabel = dev.is_active ? 'активний' : 'вимкнено';

    const header = document.createElement('div');
    header.className = 'phys-device-header open';
    header.innerHTML = `
        <span class="phys-arrow">▶</span>
        <div class="led" id="led-device-${dev.id}"></div>
        <span class="phys-device-name">${dev.device_name ?? 'Пристрій #' + dev.id}</span>
        <div class="phys-device-meta">
            <span>addr: ${dev.address}</span>
            <span>опит: ${dev.time_for_recall} мс</span>
            <span class="badge ${activeClass}">${activeLabel}</span>
        </div>
        <div class="phys-node-actions">
            <button class="btn-add-sm" onclick="openValueForm(null, ${dev.id})">+ value</button>
            <button class="btn-edit" onclick="openDeviceForm(${dev.id})">ред.</button>
            <button class="btn-del"  onclick="deleteDevice(${dev.id})">вид.</button>
        </div>`;

    const body = document.createElement('div');
    body.className = 'phys-device-body';

    values.forEach(v => {
        const row = document.createElement('div');
        row.className = 'phys-value-row';
        row.innerHTML = `
            <span class="phys-value-name">${v.value_name}</span>
            <span class="phys-value-tag">${v.value_tag}</span>
            <span class="phys-value-type">${getTypeName(v.decoding_type)}</span>
            <span class="phys-value-live" id="live-phys-${v.value_tag}">—</span>
            <div class="row-actions">
                <button class="btn-edit" onclick="openValueForm(${v.id})">ред.</button>
                <button class="btn-del"  onclick="deleteValue(${v.id})">вид.</button>
            </div>`;
        body.appendChild(row);
    });

    // Рядок додати value
    const addRow = document.createElement('div');
    addRow.className = 'phys-add-value-row';
    addRow.innerHTML = `<button class="btn-add-sm" onclick="openValueForm(null, ${dev.id})">+ додати value</button>`;
    body.appendChild(addRow);

    // Розгорнути/згорнути
    header.addEventListener('click', e => {
        if (e.target.closest('button')) return;
        const open = header.classList.toggle('open');
        body.classList.toggle('hidden', !open);
    });

    card.appendChild(header);
    card.appendChild(body);
    return card;
}

// Заглушки — залишені для сумісності (таблиці більше не рендеряться)
async function loadNodes()   { /* використовується loadPhysicalPanel */ }
async function loadDevices() { /* використовується loadPhysicalPanel */ }
async function loadValues()  { /* використовується loadPhysicalPanel */ }

// =====================================================
// ЛОГІЧНЕ
// =====================================================
function initLogicalTabs() {
    // Таби замінені деревом
}

async function loadLogicalPanel() {
    const [users, groups, subs, values] = await Promise.all([
        apiFetch('/users/get_all'),
        apiFetch('/user_group/all'),
        apiFetch('/sub_groups/get_all'),
        apiFetch('/values/get_all'),
    ]);
    state.values = values || [];

    // Підтягуємо прив'язки для всіх підгруп одним запитом
    let subgroupAssigns = [];
    if (subs && subs.length) {
        const ids = subs.map(s => s.id).join(',');
        const assigns = await apiFetch(`/sub_groups/ui/assigns/${ids}`);
        if (assigns && !assigns.__error) subgroupAssigns = assigns;
    }

    // Будуємо мапу: subgroup_id → [value_id, ...]
    state.subgroupValues = {};
    subgroupAssigns.forEach(a => {
        state.subgroupValues[a.id] = a.values_id;
    });

    renderUsersBlock(users || []);
    renderLogicTree(groups || [], subs || [], values || []);
    renderAssignSelects(users || [], groups || [], subs || [], values || []);
}

// --- Таблиця користувачів ---
function renderUsersBlock(users) {
    const container = document.getElementById('logic-users');
    container.innerHTML = '';

    if (!users.length) {
        container.innerHTML = '<div style="color:var(--text-muted);font-size:13px;padding:8px">Немає користувачів</div>';
        return;
    }

    users.forEach(u => {
        const row = document.createElement('div');
        row.className = 'phys-value-row';
        const roleLabel = ROLES[u.role_id] ?? `Роль #${u.role_id}`;
        const activeClass = u.is_active !== false ? 'badge-on' : 'badge-off';
        const activeLabel = u.is_active !== false ? 'активний' : 'вимкнено';
        row.innerHTML = `
            <span class="phys-value-name">${u.name}</span>
            <span class="phys-value-tag">${roleLabel}</span>
            <span class="phys-value-type"><span class="badge ${activeClass}">${activeLabel}</span></span>
            <div class="row-actions">
                <button class="btn-add-sm" onclick="openAssignGroupsModal(${u.id}, '${u.name}')">групи</button>
                <button class="btn-edit" onclick="openUserForm(${u.id})">ред.</button>
                <button class="btn-del"  onclick="deleteUser(${u.id})">вид.</button>
            </div>`;
        container.appendChild(row);
    });
}

// --- Дерево груп → підгруп → values ---
function renderLogicTree(groups, subs, values) {
    const tree = document.getElementById('logic-tree');
    tree.innerHTML = '';

    groups.forEach(g => {
        const groupSubs = subs.filter(s => s.group_id === g.id);
        tree.appendChild(buildGroupCard(g, groupSubs, values));
    });
}

function buildGroupCard(group, subs, values) {
    const card = document.createElement('div');
    card.className = 'phys-node';

    const header = document.createElement('div');
    header.className = 'phys-node-header open';
    header.innerHTML = `
        <span class="phys-arrow">▶</span>
        <span class="phys-node-ip">${group.group_name}</span>
        <span class="phys-node-meta">${group.description ?? ''}</span>
        <div class="phys-node-actions">
            <button class="btn-add-sm" onclick="openSubgroupForm(null, ${group.id})">+ підгрупа</button>
            <button class="btn-edit" onclick="openGroupForm(${group.id})">ред.</button>
            <button class="btn-del"  onclick="deleteGroup(${group.id})">вид.</button>
        </div>`;

    const body = document.createElement('div');
    body.className = 'phys-node-body';

    if (!subs.length) {
        const empty = document.createElement('div');
        empty.style.cssText = 'color:var(--text-muted);font-size:12px;padding:6px 0';
        empty.textContent = 'Немає підгруп';
        body.appendChild(empty);
    }

    subs.forEach(s => {
        const subValues = values; // buildSubgroupCard сам фільтрує по state.subgroupValues
        body.appendChild(buildSubgroupCard(s, subValues));
    });

    header.addEventListener('click', e => {
        if (e.target.closest('button')) return;
        const open = header.classList.toggle('open');
        body.classList.toggle('hidden', !open);
    });

    card.appendChild(header);
    card.appendChild(body);
    return card;
}

function buildSubgroupCard(sub, allValues) {
    const card = document.createElement('div');
    card.className = 'phys-device';

    // Фільтруємо тільки прив'язані values
    const assignedIds = new Set((state.subgroupValues?.[sub.id] ?? []).map(id => parseInt(id)));
    const values = allValues.filter(v => assignedIds.has(parseInt(v.id)));

    const countLabel = values.length
        ? `<span style="color:var(--accent);font-size:11px;margin-left:6px">${values.length} values</span>`
        : `<span style="color:var(--text-muted);font-size:11px;margin-left:6px">не прив'язано</span>`;

    const header = document.createElement('div');
    header.className = 'phys-device-header open';
    header.innerHTML = `
        <span class="phys-arrow">▶</span>
        <span class="phys-device-name">${sub.subgroup_name}</span>
        <span class="phys-node-meta">${sub.description ?? ''}</span>
        ${countLabel}
        <div class="phys-node-actions">
            <button class="btn-add-sm" onclick="openAssignValuesModal(${sub.id})">прив'язки</button>
            <button class="btn-edit" onclick="openSubgroupForm(${sub.id})">ред.</button>
            <button class="btn-del"  onclick="deleteSubgroup(${sub.id})">вид.</button>
        </div>`;

    const body = document.createElement('div');
    body.className = 'phys-device-body';

    if (!values.length) {
        const empty = document.createElement('div');
        empty.className = 'phys-value-row';
        empty.style.color = 'var(--text-muted)';
        empty.textContent = "Values не прив'язані";
        body.appendChild(empty);
    }

    values.forEach(v => {
        const row = document.createElement('div');
        row.className = 'phys-value-row';
        row.innerHTML = `
            <span class="phys-value-name">${v.value_name}</span>
            <span class="phys-value-tag">${v.value_tag}</span>
            <span class="phys-value-type">${getTypeName(v.decoding_type)}</span>
            <div class="row-actions">
                <button class="btn-del" onclick="removeValueFromSubgroup(${sub.id}, ${v.id})">від'єднати</button>
            </div>`;
        body.appendChild(row);
    });

    header.addEventListener('click', e => {
        if (e.target.closest('button')) return;
        const open = header.classList.toggle('open');
        body.classList.toggle('hidden', !open);
    });

    card.appendChild(header);
    card.appendChild(body);
    return card;
}

// --- Прив'язки (селекти) ---
function renderAssignSelects(users, groups, subs, values) {
    // Юзер → групи
    fillSelect('assign-user-select', users, u => ({ value: u.id, label: u.name }));

    const groupsList = document.getElementById('assign-groups-list');
    if (groupsList) {
        groupsList.innerHTML = '';
        groups.forEach(g => {
            const label = document.createElement('label');
            label.className = 'checkbox-label';
            label.innerHTML = `<input type="checkbox" value="${g.id}"> ${g.group_name}`;
            groupsList.appendChild(label);
        });
    }

    // Підгрупа → values
    fillSelect('assign-subgroup-select', subs, s => ({ value: s.id, label: s.subgroup_name }));

    const valuesList = document.getElementById('assign-values-list');
    if (valuesList) {
        valuesList.innerHTML = '';
        values.forEach(v => {
            const label = document.createElement('label');
            label.className = 'checkbox-label';
            label.innerHTML = `<input type="checkbox" value="${v.id}"> ${v.value_name} <span style="color:var(--text-muted)">(${v.value_tag})</span>`;
            valuesList.appendChild(label);
        });
    }
}

async function loadUsers() { /* використовується loadLogicalPanel */ }
async function loadGroupsTable() { /* використовується loadLogicalPanel */ }
async function loadSubgroupsTable() { /* використовується loadLogicalPanel */ }

// --- Прив'язки ---
async function loadAssignSelects() {
    const [users, groups, subs, values] = await Promise.all([
        apiFetch('/users/get_all'),
        apiFetch('/user_group/all'),
        apiFetch('/sub_groups/get_all'),
        apiFetch('/values/get_all'),
    ]);

    fillSelect('assign-user-select', users || [], u => ({ value: u.id, label: u.name }));
    fillSelect('assign-subgroup-select', subs || [], s => ({ value: s.id, label: s.subgroup_name }));

    // Групи — чекбокси
    const groupsList = document.getElementById('assign-groups-list');
    groupsList.innerHTML = '';
    (groups || []).forEach(g => {
        const label = document.createElement('label');
        label.className = 'checkbox-label';
        label.innerHTML = `<input type="checkbox" value="${g.id}"> ${g.group_name}`;
        groupsList.appendChild(label);
    });

    // Values — чекбокси
    const valuesList = document.getElementById('assign-values-list');
    valuesList.innerHTML = '';
    (values || []).forEach(v => {
        const label = document.createElement('label');
        label.className = 'checkbox-label';
        label.innerHTML = `<input type="checkbox" value="${v.id}"> ${v.value_name} <span style="color:var(--text-muted)">(${v.value_tag})</span>`;
        valuesList.appendChild(label);
    });
}

async function onAssignUserChange(e) {
    const userId = parseInt(e.target.value);
    if (!userId) return;
    const groups = await apiFetch(`/user_group/get_by_user_id/${userId}`);
    if (!groups) return;
    const assignedIds = new Set(groups.map(g => g.id));
    document.querySelectorAll('#assign-groups-list input[type=checkbox]').forEach(cb => {
        cb.checked = assignedIds.has(parseInt(cb.value));
    });
}

async function onAssignSubgroupChange(e) {
    // TODO: ендпоінт отримання прив'язаних values до підгрупи
}

// Модалка прив'язки підгрупа → values — дерево нода→пристрій→values
async function openAssignValuesModal(subgroupId) {
    const [nodes, devices, values] = await Promise.all([
        apiFetch('/nodes/get_all'),
        apiFetch('/devices/get_all'),
        apiFetch('/values/get_all'),
    ]);
    if (!values || values.__error) return;

    // Будуємо дерево нода → пристрій → values з чекбоксами
    let treeHtml = '';
    (nodes || []).forEach(node => {
        const nodeDevices = (devices || []).filter(d => d.parent_node_id === node.id);
        if (!nodeDevices.length) return;

        let devHtml = '';
        nodeDevices.forEach(dev => {
            const devValues = values.filter(v => v.parent_device_id === dev.id);
            if (!devValues.length) return;

            const valRows = devValues.map(v => `
                <label class="checkbox-label" style="padding-left:24px">
                    <input type="checkbox" value="${v.id}">
                    ${v.value_name}
                    <span style="color:var(--text-muted);font-size:10px">(${v.value_tag})</span>
                </label>`).join('');

            devHtml += `
                <div style="margin:4px 0 4px 12px">
                    <div style="font-size:12px;font-weight:600;color:var(--text-mid);padding:4px 0">
                        ▸ ${dev.device_name ?? 'Пристрій #' + dev.id}
                    </div>
                    ${valRows}
                </div>`;
        });

        if (!devHtml) return;
        treeHtml += `
            <div style="margin-bottom:8px;border:1px solid var(--border);border-radius:4px;overflow:hidden">
                <div style="background:var(--bg-panel);padding:6px 10px;font-size:12px;font-weight:700;color:var(--text)">
                    ${node.ip}${node.description ? ' · ' + node.description : ''}
                </div>
                <div style="padding:6px 8px">${devHtml}</div>
            </div>`;
    });

    if (!treeHtml) treeHtml = '<div style="color:var(--text-muted)">Немає values</div>';

    // Поточні прив'язки
    const assignedIds = new Set(state.subgroupValues?.[subgroupId] ?? []);

    // Відмічаємо вже прив'язані
    treeHtml = treeHtml.replace(
        /value="\$(\{\d+\})"/g,
        (match, vid) => `value="${vid}" ${assignedIds.has(parseInt(vid)) ? 'checked' : ''}`
    );

    openFormModal(
        'Values для підгрупи',
        'Зберегти',
        `<div class="form-group">
            <label class="form-label">Оберіть values (нода → пристрій → value):</label>
            <div style="max-height:380px;overflow-y:auto;margin-top:6px">${treeHtml}</div>
        </div>`,
        async (formData, formEl) => {
            const valueIds = [...formEl.querySelectorAll('input[type=checkbox]:checked')]
                .map(cb => parseInt(cb.value));
            const details = `Зберегти ${valueIds.length} values для підгрупи #${subgroupId}`;
            await confirmAndExecute(details, async () => {
                const r = await apiFetch('/assign/values', {
                    method: 'POST',
                    body: JSON.stringify({ subgroup_id: subgroupId, value_unit_ids: valueIds })
                });
                if (!r?.__error) await loadLogicalPanel();
                return r;
            });
        },
        // Після рендеру — відмічаємо вже прив'язані чекбокси
        (formEl) => {
            formEl.querySelectorAll('input[type=checkbox]').forEach(cb => {
                if (assignedIds.has(parseInt(cb.value))) cb.checked = true;
            });
        }
    );
}

// Модалка прив'язки юзер → групи
async function openAssignGroupsModal(userId, userName) {
    const [allGroups, userGroups] = await Promise.all([
        apiFetch('/user_group/all'),
        apiFetch(`/user_group/get_by_user_id/${userId}`),
    ]);
    if (!allGroups || allGroups.__error) return;

    const assignedIds = new Set((userGroups || []).map(g => g.id));

    const checkboxes = allGroups.map(g => `
        <label class="checkbox-label">
            <input type="checkbox" value="${g.id}" ${assignedIds.has(g.id) ? 'checked' : ''}>
            ${g.group_name}${g.description ? ` <span style="color:var(--text-muted)">· ${g.description}</span>` : ''}
        </label>`).join('');

    openFormModal(
        `Групи для: ${userName}`,
        'Зберегти',
        `<div class="form-group">
            <label class="form-label">Оберіть групи (поточні вже відмічені):</label>
            <div class="checkbox-list" style="max-height:300px">${checkboxes}</div>
        </div>
        <div style="margin-top:8px;display:flex;gap:8px">
            <button class="btn-danger-outline" onclick="clearUserGroups(${userId})">Очистити всі</button>
        </div>`,
        async (formData, formEl) => {
            const groupIds = [...formEl.querySelectorAll('input[type=checkbox]:checked')]
                .map(cb => parseInt(cb.value));
            const details = `Прив-язати до юзера ${userName}: ${groupIds.length} груп`;
            await confirmAndExecute(details, async () => {
                const r = await apiFetch('/assign/group', {
                    method: 'POST',
                    body: JSON.stringify({ user_id: userId, group_ids: groupIds })
                });
                if (!r?.__error) await loadLogicalPanel();
                return r;
            });
        }
    );
}

async function clearUserGroups(userId) {
    await confirmAndExecute(
        `Очистити всі прив-язки груп для юзера #${userId}`,
        async () => {
            const r = await apiFetch(`/assign/clean/users/${userId}`, { method: 'DELETE' });
            if (!r?.__error) await loadLogicalPanel();
            return r;
        }
    );
}

async function saveAssignGroups() {
    const userId = parseInt(document.getElementById('assign-user-select').value);
    if (!userId) return;
    const groupIds = [...document.querySelectorAll('#assign-groups-list input:checked')].map(cb => parseInt(cb.value));
    await confirmAndExecute(
        `Прив'язати до юзера #${userId} групи: [${groupIds.join(', ')}]`,
        () => apiFetch('/assign/group', {
            method: 'POST',
            body: JSON.stringify({ user_id: userId, group_ids: groupIds })
        })
    );
}

async function cleanAssignGroups() {
    const userId = parseInt(document.getElementById('assign-user-select').value);
    if (!userId) return;
    await confirmAndExecute(
        `Очистити всі прив-язки груп для юзера #${userId}`,
        () => apiFetch(`/assign/clean/users/${userId}`, { method: 'DELETE' })
    );
}

async function saveAssignValues() {
    const subgroupId = parseInt(document.getElementById('assign-subgroup-select').value);
    if (!subgroupId) return;
    const valueIds = [...document.querySelectorAll('#assign-values-list input:checked')].map(cb => parseInt(cb.value));
    await confirmAndExecute(
        `Прив'язати до підгрупи #${subgroupId} values: [${valueIds.join(', ')}]`,
        () => apiFetch('/assign/values', {
            method: 'POST',
            body: JSON.stringify({ subgroup_id: subgroupId, value_unit_ids: valueIds })
        })
    );
}

async function cleanAssignValues() {
    const subgroupId = parseInt(document.getElementById('assign-subgroup-select').value);
    if (!subgroupId) return;
    await confirmAndExecute(
        `Очистити всі прив'язки values для підгрупи #${subgroupId}`,
        () => apiFetch(`/assign/clean/subgroups/${subgroupId}`, { method: 'DELETE' })
    );
}

// =====================================================
// ФОРМИ (модалка)
// =====================================================

// --- Нода ---
function openNodeForm(id = null) {
    const node = id ? state.nodes.find(n => n.id === id) : null;
    const isEdit = !!node;

    openFormModal(
        isEdit ? 'Редагувати ноду' : 'Нова нода',
        isEdit ? 'Зберегти' : 'Створити',
        `
        <div class="form-group">
            <label class="form-label">IP адреса *</label>
            <input class="form-input" type="text" name="ip" value="${node?.ip ?? ''}" placeholder="192.168.1.10" ${isEdit ? '' : ''}>
        </div>
        <div class="form-group">
            <label class="form-label">Порт</label>
            <input class="form-input" type="number" name="port" value="${node?.port ?? 502}" min="1" max="65535">
        </div>
        <div class="form-group">
            <label class="form-label">Опис</label>
            <textarea class="form-input form-textarea" name="description" placeholder="Необов'язково...">${node?.description ?? ''}</textarea>
        </div>`,
        async (formData) => {
            const payload = {
                id:          id,
                ip:          formData.ip || null,
                port:        formData.port ? parseInt(formData.port) : 502,
                description: formData.description || null,
            };
            const details = `IP: ${payload.ip}\nПорт: ${payload.port}\nОпис: ${payload.description ?? '—'}`;
            if (isEdit) {
                await confirmAndExecute(details, () => apiFetch(`/nodes/update/${id}`, { method: 'PUT', body: JSON.stringify(payload) }));
            } else {
                await confirmAndExecute(details, () => apiFetch('/nodes/create', { method: 'POST', body: JSON.stringify(payload) }));
            }
            await loadPhysicalPanel();
        }
    );
}

async function deleteNode(id) {
    const node = state.nodes.find(n => n.id === id);
    await confirmAndExecute(
        `Видалити ноду #${id} (${node?.ip ?? ''})`,
        async () => {
            await apiFetch(`/nodes/${id}`, { method: 'DELETE' });
            await loadPhysicalPanel();
        }
    );
}

// --- Пристрій ---
function openDeviceForm(id = null, parentNodeId = null) {
    const dev = id ? state.devices.find(d => d.id === id) : null;
    const isEdit = !!dev;
    const preselectedNode = dev?.parent_node_id ?? parentNodeId;
    const nodeOptions = state.nodes.map(n =>
        `<option value="${n.id}" ${preselectedNode === n.id ? 'selected' : ''}>${n.ip}</option>`
    ).join('');

    openFormModal(
        isEdit ? 'Редагувати пристрій' : 'Новий пристрій',
        isEdit ? 'Зберегти' : 'Створити',
        `
        <div class="form-group">
            <label class="form-label">Назва *</label>
            <input class="form-input" type="text" name="device_name" value="${dev?.device_name ?? ''}">
        </div>
        <div class="form-group">
            <label class="form-label">Нода *</label>
            <select class="form-select" name="parent_node_id">${nodeOptions}</select>
        </div>
        <div class="form-group">
            <label class="form-label">Адреса Modbus *</label>
            <input class="form-input" type="number" name="address" value="${dev?.address ?? 1}" min="0" max="255">
        </div>
        <div class="form-group">
            <label class="form-label">Час опитування (мс) *</label>
            <input class="form-input" type="number" name="time_for_recall" value="${dev?.time_for_recall ?? 1000}" min="1">
        </div>
        <div class="form-group">
            <label class="form-label">Таймаут (мс) *</label>
            <input class="form-input" type="number" name="timeout" value="${dev?.timeout ?? 500}" min="1">
        </div>
        <div class="form-group">
            <label class="form-label">Повторів *</label>
            <input class="form-input" type="number" name="retry_count" value="${dev?.retry_count ?? 3}" min="1">
        </div>
        <div class="form-group">
            <div class="form-checkbox-row">
                <input type="checkbox" name="is_active" id="dev-active" ${dev?.is_active !== false ? 'checked' : ''}>
                <label for="dev-active">Активний</label>
            </div>
        </div>
        <div class="form-group">
            <div class="form-checkbox-row">
                <input type="checkbox" name="read_by_group" id="dev-group" ${dev?.read_by_group ? 'checked' : ''}>
                <label for="dev-group">Групове читання</label>
            </div>
        </div>
        <div class="form-group">
            <label class="form-label">Опис</label>
            <textarea class="form-input form-textarea" name="description" placeholder="Необов'язково...">${dev?.description ?? ''}</textarea>
        </div>`,
        async (formData) => {
            const payload = {
                id:             id ?? undefined,
                device_name:    formData.device_name,
                parent_node_id: parseInt(formData.parent_node_id),
                address:        parseInt(formData.address),
                time_for_recall:parseInt(formData.time_for_recall),
                timeout:        parseInt(formData.timeout),
                retry_count:    parseInt(formData.retry_count),
                is_active:      !!formData.is_active,
                read_by_group:  !!formData.read_by_group,
                description:    formData.description || null,
            };
            const details = `Назва: ${payload.device_name}\nАдреса: ${payload.address}\nНода: ${payload.parent_node_id}`;
            if (isEdit) {
                await confirmAndExecute(details, async () => {
                    const r = await apiFetch(`/devices/update/${id}`, { method: 'PUT', body: JSON.stringify(payload) });
                    if (!r?.__error) await refreshDeviceCard(id);
                    return r;
                });
            } else {
                await confirmAndExecute(details, async () => {
                    const r = await apiFetch('/devices/create', { method: 'POST', body: JSON.stringify(payload) });
                    if (!r?.__error) await refreshNodeBranch(payload.parent_node_id);
                    return r;
                });
            }
        }
    );
}

async function deleteDevice(id) {
    const dev = state.devices.find(d => d.id === id);
    await confirmAndExecute(
        `Видалити пристрій #${id} (${dev?.device_name ?? ''})`,
        async () => {
            const r = await apiFetch(`/devices/delete/${id}`, { method: 'DELETE' });
            if (!r?.__error) removeDeviceCard(id);
            return r;
        }
    );
}

// --- Value unit ---
function openValueForm(id = null, parentDeviceId = null) {
    const val = id ? state.values.find(v => v.id === id) : null;
    const isEdit = !!val;
    const preselectedDevice = val?.parent_device_id ?? parentDeviceId;
    const deviceOptions = state.devices.map(d =>
        `<option value="${d.id}" ${preselectedDevice === d.id ? 'selected' : ''}>${d.device_name ?? `#${d.id}`}</option>`
    ).join('');
    const typeOptions = Object.entries(VALUE_SCHEMAS).map(([tid, s]) =>
        `<option value="${tid}" ${val?.decoding_type == tid ? 'selected' : ''}>${s.name}</option>`
    ).join('');

    const currentSettings = val?.settings ?? {};
    const currentTypeId = val?.decoding_type ?? 1;

    openFormModal(
        isEdit ? 'Редагувати value' : 'Нове value',
        isEdit ? 'Зберегти' : 'Створити',
        `
        <div class="form-group">
            <label class="form-label">Назва *</label>
            <input class="form-input" type="text" name="value_name" value="${val?.value_name ?? ''}">
        </div>
        <div class="form-group">
            <label class="form-label">Тег *</label>
            <input class="form-input" type="text" name="value_tag" value="${val?.value_tag ?? ''}">
        </div>
        <div class="form-group">
            <label class="form-label">Пристрій *</label>
            <select class="form-select" name="parent_device_id">${deviceOptions}</select>
        </div>
        <div class="form-group">
            <label class="form-label">Тип декодування *</label>
            <select class="form-select" name="decoding_type" id="val-type-select">${typeOptions}</select>
        </div>
        <div class="form-group">
            <div class="form-checkbox-row">
                <input type="checkbox" name="is_logging" id="val-logging" ${val?.is_logging ? 'checked' : ''}>
                <label for="val-logging">Логування</label>
            </div>
        </div>
        <div class="form-group">
            <label class="form-label">Опис</label>
            <textarea class="form-input form-textarea" name="description" placeholder="Необов'язково...">${val?.description ?? ''}</textarea>
        </div>
        <div class="form-section-title">Параметри декодування</div>
        <div id="settings-fields">${buildSettingsFields(currentTypeId, currentSettings)}</div>`,
        async (formData, formEl) => {
            const typeId = parseInt(formEl.querySelector('[name=decoding_type]').value);
            const settings = readSettingsFromForm(formEl);
            // tag береться з value_tag і дублюється в settings
            settings.tag = formData.value_tag;

            const payload = {
                id:               id ?? undefined,
                value_name:       formData.value_name,
                value_tag:        formData.value_tag,
                parent_device_id: parseInt(formData.parent_device_id),
                decoding_type:    typeId,
                is_logging:       !!formData.is_logging,
                description:      formData.description || null,
                settings,
            };
            const details = `Назва: ${payload.value_name}\nТег: ${payload.value_tag}\nТип: ${getTypeName(typeId)}`;
            const deviceId = payload.parent_device_id;
            if (isEdit) {
                await confirmAndExecute(details, async () => {
                    const r = await apiFetch(`/values/update/${id}`, { method: 'PUT', body: JSON.stringify(payload) });
                    if (!r?.__error) await refreshDeviceCard(deviceId);
                    return r;
                });
            } else {
                await confirmAndExecute(details, async () => {
                    const r = await apiFetch('/values/create', { method: 'POST', body: JSON.stringify(payload) });
                    if (!r?.__error) await refreshDeviceCard(deviceId);
                    return r;
                });
            }
        },
        // після рендеру форми — підписуємось на зміну типу
        (formEl) => {
            formEl.querySelector('#val-type-select').addEventListener('change', (e) => {
                const newTypeId = parseInt(e.target.value);
                formEl.querySelector('#settings-fields').innerHTML = buildSettingsFields(newTypeId);
            });
        }
    );
}

async function deleteValue(id) {
    const val = state.values.find(v => v.id === id);
    await confirmAndExecute(
        `Видалити value #${id} (${val?.value_name ?? ''}, тег: ${val?.value_tag ?? ''})`,
        async () => {
            const r = await apiFetch(`/values/delete/${id}`, { method: 'DELETE' });
            if (!r?.__error && val) await refreshDeviceCard(val.parent_device_id);
            return r;
        }
    );
}

// --- Користувач ---
async function openUserForm(id = null) {
    // Якщо редагування — підтягуємо дані юзера
    let user = null;
    if (id) {
        const res = await apiFetch(`/users/get_by_id/${id}`);
        if (res && !res.__error) user = res;
    }

    const roleOptions = Object.entries(ROLES).map(([rid, rname]) =>
        `<option value="${rid}" ${(user?.role_id ?? 3) == rid ? 'selected' : ''}>${rname}</option>`
    ).join('');

    const isEdit = !!user;
    const userName = user?.name ?? user?.username ?? '';

    openFormModal(
        isEdit ? `Редагувати: ${userName}` : 'Новий користувач',
        isEdit ? 'Зберегти' : 'Створити',
        `
        <div class="form-group">
            <label class="form-label">Логін *</label>
            <input class="form-input" type="text" name="username" value="${userName}">
        </div>
        <div class="form-group">
            <label class="form-label">Пароль${isEdit ? ' (порожньо — не змінювати)' : ' *'}</label>
            <input class="form-input" type="password" name="password_hash">
        </div>
        <div class="form-group">
            <label class="form-label">Роль *</label>
            <select class="form-select" name="role_id">${roleOptions}</select>
        </div>
        ${isEdit ? `
        <div class="form-group">
            <div class="form-checkbox-row">
                <input type="checkbox" name="is_active" id="user-active" ${user?.is_active !== false ? 'checked' : ''}>
                <label for="user-active">Активний</label>
            </div>
        </div>` : ''}`,
        async (formData) => {
            const payload = {
                id:        id ?? undefined,
                username:  formData.username,
                role_id:   parseInt(formData.role_id),
                is_active: formData.is_active !== undefined ? !!formData.is_active : true,
            };
            // Пароль додаємо тільки якщо він заповнений
            if (formData.password_hash && formData.password_hash.trim() !== '') {
                payload.password_hash = formData.password_hash;
            }
            const roleLabel = ROLES[payload.role_id] ?? payload.role_id;
            const details = `Логін: ${payload.username}\nРоль: ${roleLabel}\nАктивний: ${payload.is_active ? 'так' : 'ні'}`;
            if (isEdit) {
                await confirmAndExecute(details, async () => {
                    const r = await apiFetch(`/users/update/${id}`, { method: 'PUT', body: JSON.stringify(payload) });
                    if (!r?.__error) await loadLogicalPanel();
                    return r;
                });
            } else {
                await confirmAndExecute(details, async () => {
                    const r = await apiFetch('/users/create', { method: 'POST', body: JSON.stringify(payload) });
                    if (!r?.__error) await loadLogicalPanel();
                    return r;
                });
            }
        }
    );
}

async function deleteUser(id) {
    await confirmAndExecute(
        `Видалити користувача #${id}`,
        async () => {
            const r = await apiFetch(`/users/delete/${id}`, { method: 'DELETE' });
            if (!r?.__error) await loadLogicalPanel();
            return r;
        }
    );
}

// --- Група ---
function openGroupForm(id = null) {
    openFormModal(
        id ? 'Редагувати групу' : 'Нова група',
        id ? 'Зберегти' : 'Створити',
        `
        <div class="form-group">
            <label class="form-label">Назва *</label>
            <input class="form-input" type="text" name="group_name">
        </div>
        <div class="form-group">
            <label class="form-label">Опис</label>
            <textarea class="form-input form-textarea" name="description" placeholder="Необов'язково..."></textarea>
        </div>`,
        async (formData) => {
            const payload = { group_name: formData.group_name, description: formData.description || null };
            const details = `Назва: ${payload.group_name}`;
            if (id) {
                await confirmAndExecute(details, () => apiFetch(`/user_group/update/${id}`, { method: 'PUT', body: JSON.stringify({ id, ...payload }) }));
            } else {
                await confirmAndExecute(details, () => apiFetch('/user_group/create', { method: 'POST', body: JSON.stringify(payload) }));
            }
            await Promise.all([loadLogicalPanel(), loadGroups()]);
        }
    );
}

async function deleteGroup(id) {
    await confirmAndExecute(
        `Видалити групу #${id}`,
        async () => {
            await apiFetch(`/user_group/delete/${id}`, { method: 'DELETE' });
            await Promise.all([loadLogicalPanel(), loadGroups()]);
        }
    );
}

// --- Підгрупа ---
function openSubgroupForm(id = null, parentGroupId = null) {
    apiFetch('/user_group/all').then(groups => {
        const groupOptions = (groups || []).map(g =>
            `<option value="${g.id}" ${g.id === parentGroupId ? 'selected' : ''}>${g.group_name}</option>`
        ).join('');

        openFormModal(
            id ? 'Редагувати підгрупу' : 'Нова підгрупа',
            id ? 'Зберегти' : 'Створити',
            `
            <div class="form-group">
                <label class="form-label">Група *</label>
                <select class="form-select" name="group_id">${groupOptions}</select>
            </div>
            <div class="form-group">
                <label class="form-label">Назва *</label>
                <input class="form-input" type="text" name="subgroup_name">
            </div>
            <div class="form-group">
                <label class="form-label">Опис</label>
                <textarea class="form-input form-textarea" name="description" placeholder="Необов'язково..."></textarea>
            </div>`,
            async (formData) => {
                const payload = {
                    group_id:      parseInt(formData.group_id),
                    subgroup_name: formData.subgroup_name,
                    description:   formData.description || null,
                };
                const details = `Назва: ${payload.subgroup_name}`;
                if (id) {
                    await confirmAndExecute(details, () => apiFetch(`/sub_groups/update/${id}`, { method: 'PUT', body: JSON.stringify({ id, ...payload }) }));
                } else {
                    await confirmAndExecute(details, () => apiFetch('/sub_groups/create', { method: 'POST', body: JSON.stringify(payload) }));
                }
                await loadLogicalPanel();
            }
        );
    });
}

async function deleteSubgroup(id) {
    await confirmAndExecute(
        `Видалити підгрупу #${id}`,
        async () => {
            await apiFetch(`/sub_groups/delete/${id}`, { method: 'DELETE' });
            await loadLogicalPanel();
        }
    );
}

// =====================================================
// МОДАЛКА: ФОРМА (універсальна)
// =====================================================
function openFormModal(title, submitLabel, bodyHtml, onSubmit, onAfterRender = null) {
    const overlay = document.getElementById('modal-form-overlay');
    const formBody = document.getElementById('modal-form-body');

    document.getElementById('modal-form-title').textContent = title;
    document.getElementById('modal-form-submit').textContent = submitLabel;
    formBody.innerHTML = bodyHtml;
    overlay.classList.remove('hidden');

    if (onAfterRender) onAfterRender(formBody);

    // Автоматичне розширення textarea
    formBody.querySelectorAll('.form-textarea').forEach(ta => {
        const resize = () => { ta.style.height = 'auto'; ta.style.height = ta.scrollHeight + 'px'; };
        ta.addEventListener('input', resize);
        resize();
    });

    // Знімаємо старий обробник і вішаємо новий
    const submitBtn = document.getElementById('modal-form-submit');
    const newBtn = submitBtn.cloneNode(true);
    submitBtn.parentNode.replaceChild(newBtn, submitBtn);

    newBtn.addEventListener('click', async () => {
        const formData = readFormData(formBody);
        closeFormModal();
        await onSubmit(formData, formBody);
    });
}

function closeFormModal() {
    document.getElementById('modal-form-overlay').classList.add('hidden');
}

function initModalClose() {
    document.getElementById('modal-form-close').addEventListener('click', closeFormModal);
    document.getElementById('modal-form-cancel').addEventListener('click', closeFormModal);
    document.getElementById('modal-form-overlay').addEventListener('click', e => {
        if (e.target === e.currentTarget) closeFormModal();
    });
}

function readFormData(container) {
    const data = {};
    container.querySelectorAll('[name]').forEach(el => {
        if (el.name.startsWith('settings.')) return;
        if (el.type === 'checkbox') {
            data[el.name] = el.checked;
        } else {
            data[el.name] = el.value;
        }
    });
    return data;
}

// =====================================================
// МОДАЛКА: ПІДТВЕРДЖЕННЯ
// =====================================================
function initConfirmCheckbox() {
    const cb  = document.getElementById('confirm-checkbox');
    const btn = document.getElementById('confirm-ok');
    cb.addEventListener('change', () => { btn.disabled = !cb.checked; });
}

function confirmAndExecute(details, action) {
    return new Promise(resolve => {
        const overlay = document.getElementById('modal-confirm-overlay');
        document.getElementById('confirm-details').textContent = details;
        document.getElementById('confirm-checkbox').checked = false;
        document.getElementById('confirm-ok').disabled = true;
        overlay.classList.remove('hidden');

        const closeConfirm = () => overlay.classList.add('hidden');

        const okBtn = document.getElementById('confirm-ok');
        const cancelBtn = document.getElementById('confirm-cancel');

        const onOk = async () => {
            cleanup();
            closeConfirm();
            const result = await action();
            if (result && result.__error) {
                showResult(false, result.message);
            } else {
                showResult(true);
            }
            resolve();
        };
        const onCancel = () => { cleanup(); closeConfirm(); resolve(); };

        function cleanup() {
            okBtn.removeEventListener('click', onOk);
            cancelBtn.removeEventListener('click', onCancel);
        }

        okBtn.addEventListener('click', onOk);
        cancelBtn.addEventListener('click', onCancel);
    });
}

// =====================================================
// ВЕБСОКЕТ
// =====================================================
// ===== ГРАФІК =====
async function loadChart(values, title) {
    if (!values || !values.length) return;

    // Запам'ятовуємо ID групи, ДЛЯ якої ми починаємо вантажити графік
    const savedGroupId = state.currentGroupId;

    const hours = getChartHours();
    const now   = Math.floor(Date.now() / 1000);
    const from  = now - hours * 3600;
    const ids   = values.map(v => v.id).join(',');

    const titleEl = document.getElementById('chart-title');
    if (titleEl) titleEl.textContent = title;

    // Зберігаємо контекст для масштабування годин
    <!--
    if (viewChart) {
        viewChart._currentValues = values;
        viewChart._currentTitle = title;
    }
    -->
    destroyChartCleanly();

    // ЛЕТИМО В БАЗУ ДАНИХ (це займає час)
    const measures = await apiFetch(`/measure/?value_ids=${ids}&start_time=${from}&end_time=${now}`);

    // СУВОРЕ ПРАВИЛО БЕЗПЕКИ:
    // Якщо поки вантажилась історія, юзер уже змінив групу або занулив її — ГАСИМО ЦЕЙ ЗАПИТ!
    if (savedGroupId !== state.currentGroupId || !state.currentGroupId) {
        return;
    }

    if (!measures || measures.__error) return;

    // Малюємо графік тільки якщо група все ще та сама
    renderChart(measures, values);
}

function renderChart(measures, values) {
    const ctx = document.getElementById('main-chart');
    if (!ctx) return;
    if (viewChart) { viewChart.destroy(); viewChart = null; }
    const colors = ['#2A6496','#C0392B','#2E8B3A','#C07800','#8B2FC9','#1A8C8C','#E67E22','#2980B9'];

    const datasets = measures.map((m, i) => {
        const v = values.find(vv => vv.id === m.id);
        const color = colors[i % colors.length];
        return {
            label: v?.value_name ?? `#${m.id}`,

             data: m.values.map(p => {
                let currentVal = p.val;
                // Якщо значення критично мале (f64::MIN), міняємо на null
                if (currentVal === null || currentVal === undefined || currentVal < -1e+300) {
                    currentVal = null;
                }
                return {
                    x: p.timestamp * 1000,
                    y: currentVal
                };
            }),

            borderColor: color,
            backgroundColor: color + '22',
            borderWidth: 2,
            pointRadius: 0,
            pointHoverRadius: 4,
            tension: 0.2,
            spanGaps: false // Змушує Chart.js робити красивий розрив лінії, а не з'єднувати точки через порожнечу
        };
    });

    viewChart = new Chart(ctx, {
        type: 'line',
        data: { datasets },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            interaction: { mode: 'nearest', intersect: false, axis: 'x' },
            // interaction: { mode: 'index', intersect: false, axis: 'x' },
            plugins: {
                legend: { position: 'top', labels: { font: { family: 'JetBrains Mono, monospace', size: 11 } } },
                tooltip: {
                    callbacks: {
                        title: ctx => new Date(ctx[0].parsed.x).toLocaleString('uk-UA'),
                        // --- ЗАХИСТ ТУЛТІПА ВІД NULL ---
                        label: ctx => {
                            const val = ctx.parsed.y;
                            if (val === null || val === undefined) {
                                return `${ctx.dataset.label}: Немає даних (Немає зв'язку)`;
                            }
                            return `${ctx.dataset.label}: ${val.toFixed(2)}`;
                        }
                    }
                }
            },
            scales: {
                x: { type: 'time', time: { unit: 'hour', displayFormats: { hour: 'HH:mm', day: 'dd.MM' } }, ticks: { maxRotation: 0, font: { size: 10 } } },
                y: { ticks: { font: { size: 10 } } }
            }
        }
    });

    viewChart._valueTags = {};
    viewChart._currentValues = values;
    viewChart._currentTitle  = document.getElementById('chart-title').textContent;
    measures.forEach((m, i) => {
        const v = values.find(vv => vv.id === m.id);
        if (v) viewChart._valueTags[v.value_tag] = i;
    });
}

function appendChartPoint(tag, value) {
    // Якщо графіка немає, або у нього немає даних, або тег не належить ЦЬОМУ графіку — негайно виходимо!
    if (!viewChart || !viewChart.data || !viewChart.data.datasets || viewChart._valueTags === undefined) return;
    if (value === null || value === undefined || value < -1e+300) {
        value = null; // перетворюємо на зрозумілий для Chart.js пропуск даних
    }

    const dsIdx = viewChart._valueTags[tag];
    if (dsIdx === undefined || !viewChart.data.datasets[dsIdx]) return;

    try {
        const parsedValue = parseFloat(value);
        if (isNaN(parsedValue)) return; // Захист від сміття замість цифр

        viewChart.data.datasets[dsIdx].data.push({
            x: Date.now(),
            y: parsedValue
        });

        // Обмежуємо масив точок, щоб графік не розпухав до нескінченності
        if (viewChart.data.datasets[dsIdx].data.length > 6000) {
            viewChart.data.datasets[dsIdx].data.shift();
        }

        // Оновлюємо графік БЕЗ анімації, щоб не навантажувати процесор
        viewChart.update('none');
    } catch (err) {
    }
}

function destroyChartCleanly() {
    if (viewChart) {
        try {
            viewChart.destroy();
        } catch (e) {
        }
        viewChart = null;
    }

    // Очищаємо сам DOM-елемент канвасу, щоб скинути контекст рендерингу браузера
    const oldCanvas = document.getElementById('main-chart');
    if (oldCanvas) {
        const parent = oldCanvas.parentElement;
        if (parent) {
            // Створюємо абсолютно новий чистий елемент canvas
            const newCanvas = document.createElement('canvas');
            newCanvas.id = 'main-chart';

            // Замінюємо старий канвас на новий
            parent.replaceChild(newCanvas, oldCanvas);
        }
    }
}
function connectViewWebSocket(subGroups) {
    // 1. Якщо старий вебсокет існує — жорстко закриваємо його перед новим підключенням
    if (state.ws) {
        state.ws.close();
        state.ws = null;
    }

    // 2. Збираємо всі теги, які є в підгрупах цієї групи
    const allValues = subGroups.flatMap(sg => sg.values);
    const tags = allValues.map(v => v.value_tag);

    // 3. ДІСТАЄМО НОДИ ТА ПРИСТРОЇ з нових полів структури Rust (UiUserGroupRead), яку ми зберегли в viewData
    const nodeIds   = (viewData && viewData.nodes)   ? viewData.nodes   : [];
    const deviceIds = (viewData && viewData.devices) ? viewData.devices : [];

    // Якщо взагалі нічого немає для підписки — виходимо
    if (!tags.length && !nodeIds.length && !deviceIds.length) return;

    // 4. Визначаємо протокол (ws або wss)
    const wsProtocol = location.protocol === 'https:' ? 'wss:' : 'ws:';

    // 5. Дістаємо токен авторизації
    const token = typeof auth !== 'undefined' && auth.getAccess ? auth.getAccess() : '';

    // 6. Формуємо URL з токеном для нашого екранованого ws_handler
    const wsUrl = `${wsProtocol}//${location.host}/live_data${token ? '?token=' + token : ''}`;

    // 7. Створюємо об'єкт вебсокета
    state.ws = new WebSocket(wsUrl);

    // 8. Чекаємо події onopen (використовуємо function(e) і "this" для уникнення race condition)
    state.ws.onopen = function(e) {

        // Відправляємо заповнені масиви на Rust-бекенд
        this.send(JSON.stringify({
            values:  tags,
            nodes:   nodeIds,   // Тепер тут реальні ID нод, наприклад [1, 2, 5]
            devices: deviceIds, // Тепер тут реальні ID пристроїв, наприклад [10, 11]
        }));
    };

    // Обробка живих повідомлень від Rust
    state.ws.onmessage = (e) => {
        try {
            const data = JSON.parse(e.data);

            if (data.values)  handleValueEvents(data.values);
            if (data.nodes)   handleNodeEvents(data.nodes);
            if (data.devices) handleDeviceEvents(data.devices);
        } catch (err) {
        }
    };

    // Обробка закриття сокета
    state.ws.onclose = async (e) => {
        state.ws = null;

        if (e.code === 1000 && viewData) {
            const ok = await tryRefresh();
            if (ok) {
                connectViewWebSocket(viewData.sub_groups);
            } else {
                goToLogin();
            }
        }
    };
}
// ОБРОБКА СТАНУ НОД (Всеїдна версія + текстовий алерт)
function handleNodeEvents(nodes) {
    if (!Array.isArray(nodes)) return;

    nodes.forEach(async node => {
        if (!node || node.id === undefined) return;

        const stateStr = String(node.state).toLowerCase().trim();
        const isOnline = stateStr === 'connected' || stateStr === 'true' || stateStr === 'online' || node.state === 1;

        // 1. Оновлюємо велике дерево (якщо відрендерено)
        const led = document.getElementById(`led-node-${node.id}`);
        if (led) {
            led.className = isOnline ? 'led online' : 'led failed';
            const header = led.closest('.phys-node-header');
            if (header) {
                const ipSpan = header.querySelector('.phys-node-ip');
                if (ipSpan) {
                    ipSpan.style.color = isOnline ? '' : '#c0392b';
                    ipSpan.style.fontWeight = isOnline ? '' : 'bold';
                }
            }
        }

        // 2. ОНОВЛЕННЯ АЛЕРТ-ПАНЕЛІ ЗВЕРХУ (Тільки IP при унконнекті)
        const strip = document.getElementById('alert-strip');
        if (!strip) return;

        const alertId = `alert-node-${node.id}`;
        let alertEl = document.getElementById(alertId);

        if (isOnline) {
            // Якщо підключено — видаляємо IP з панелі аварій
            if (alertEl) alertEl.remove();
        } else {
            // Якщо зв'язок втрачено — виводимо IP
            let nodeIp = node.ip;

            // Якщо в сокеті немає IP, дістаємо через твій роут by_id
            if (!nodeIp) {
                const cached = state.nodes.find(n => n.id === node.id);
                if (cached) {
                    nodeIp = cached.ip;
                } else {
                    const fresh = await apiFetch(`/nodes/${node.id}`);
                    if (fresh && !fresh.__error) nodeIp = fresh.ip;
                }
            }
            nodeIp = nodeIp || `Node IP #${node.id}`;

            // Додаємо IP на панель аварій червоним
            if (!alertEl) {
                alertEl = document.createElement('div');
                alertEl.id = alertId;
                alertEl.className = 'alert-badge node-fail';
                strip.appendChild(alertEl);
            }

            alertEl.textContent = nodeIp;
            alertEl.style.color = '#c0392b'; // Суворо червоний при унконнекті
            alertEl.style.fontWeight = 'bold';
            alertEl.style.marginRight = '8px';
        }
    });
}

// ОБРОБКА СТАНУ ПРИСТРОЇВ
function handleDeviceEvents(devices) {
    if (!Array.isArray(devices)) return;

    devices.forEach(async dev => {
        if (!dev || dev.id === undefined) return;
        const stateStr = String(dev.state).trim(); // "Failed", "SomePart", "Full"

        // 1. Оновлюємо LED та колір у великому фізичному дереві (якщо воно відкрите)
        const led = document.getElementById(`led-device-${dev.id}`);
        if (led) {
            led.className = (stateStr === 'Full' || stateStr === 'Active' || stateStr === 'true') ? 'led online' : (stateStr === 'SomePart' ? 'led some-part' : 'led failed');
            const devContainer = led.closest('.phys-device');
            if (devContainer) {
                const nameEl = devContainer.querySelector('.phys-device-name');
                if (nameEl) {
                    nameEl.style.color = stateStr === 'Failed' ? '#c0392b' : (stateStr === 'SomePart' ? '#f39c12' : '');
                    nameEl.style.fontWeight = stateStr === 'Full' ? '' : 'bold';
                }
            }
        }

        // 2. ОНОВЛЕННЯ АЛЕРТ-ПАНЕЛІ ЗВЕРХУ
        const strip = document.getElementById('alert-strip');
        if (!strip) return;

        const alertId = `alert-dev-${dev.id}`;
        let alertEl = document.getElementById(alertId);

        if (stateStr === 'Full') {
            // Якщо все добре — прибираємо пристрій з панелі аварій
            if (alertEl) alertEl.remove();
        } else {
            // Якщо Failed або SomePart — виводимо на панель
            let devName = dev.device_name;

            // Якщо бекенд не прислав ім'я в сокеті, беремо з кешу або смикаємо твій роут за ID
            if (!devName) {
                const cached = state.devices.find(d => d.id === dev.id);
                if (cached) {
                    devName = cached.device_name;
                } else {
                    const fresh = await apiFetch(`/devices/get_device/${dev.id}`);
                    if (fresh && !fresh.__error) devName = fresh.device_name;
                }
            }
            devName = devName || `Пристрій #${dev.id}`;

            // Створюємо або оновлюємо плашку на панелі аварій
            if (!alertEl) {
                alertEl = document.createElement('div');
                alertEl.id = alertId;
                alertEl.className = 'alert-badge'; // твій CSS клас для плашок
                strip.appendChild(alertEl);
            }

            // Фарбуємо ім'я пристрою залежно від важкості
            alertEl.textContent = devName;
            alertEl.style.color = stateStr === 'Failed' ? '#c0392b' : '#f39c12';
            alertEl.style.fontWeight = 'bold';
            alertEl.style.marginRight = '8px'; // щоб не злипались
        }
    });
}
function removeTextAlertByDeviceId(deviceId) {
    const strip = document.getElementById('alert-strip');
    if (!strip) return;

    // Шукаємо всі алерти, які містять ID цього пристрою, і видаляємо їх
    Array.from(strip.children).forEach(child => {
        if (child.textContent.includes(`[ID: ${deviceId}]`)) {
            child.remove();
        }
    });
}

// Модифікована функція алертів із підтримкою data-атрибутів для чистки
function addTextAlert(message, type = 'danger', nodeId = null) {
    const strip = document.getElementById('alert-strip');
    if (!strip) return;

    const exists = Array.from(strip.children).some(child => child.textContent.includes(message));
    if (exists) return;

    const alertEl = document.createElement('div');
    alertEl.className = `alert-item ${type}`;
    if (nodeId !== null) {
        alertEl.dataset.nodeId = nodeId; // прикріплюємо ID ноди до плашки
    }
    alertEl.innerHTML = `<span>${message}</span><button onclick="this.parentElement.remove()">×</button>`;

    strip.insertBefore(alertEl, strip.firstChild);
}
function removeTextAlertByNodeId(nodeId) {
    const strip = document.getElementById('alert-strip');
    if (!strip) return;

    Array.from(strip.children).forEach(child => {
        if (child.dataset.nodeId == nodeId) {
            child.remove();
        }
    });
}

/*
function handleValueEvents(values) {
    values.forEach(({ tag, value }) => {
        const el = document.getElementById(`live-${tag}`);
        if (el) el.textContent = value;
    });
}

 */
// 1. ОБРОБКА ДАТЧИКІВ (Тільки міняємо текст і кидаємо точку в графік)
function handleValueEvents(values) {
    if (!Array.isArray(values)) return;

    values.forEach(item => {
        const tag = item.tag || item.value_tag;
        const val = item.value !== undefined ? item.value : item.last_value;
        if (!tag) return;

        // Міняємо текст на картці перегляду
        const el = document.getElementById(`live-${tag}`);
        if (el) el.textContent = val ?? '—';

        // Міняємо текст у фізичному дереві
        const physEl = document.getElementById(`live-phys-${tag}`);
        if (physEl) physEl.textContent = val ?? '—';

        // Кидаємо точку на графік
        if (typeof appendChartPoint === 'function') {
            appendChartPoint(tag, val);
        }
    });
}

// 2. ОБРОБКА СТАНУ НОД (Тільки міняємо колір LED, НІЯКИХ рендерів усього дерева!)


// 3. ОБРОБКА СТАНУ ПРИСТРОЇВ (Тільки міняємо колір LED!)
// 1. ОБРОБКА ДАТЧИКІВ (Тільки міняємо текст і кидаємо точку в графік)
function handleValueEvents(values) {
    if (!Array.isArray(values)) return;

    values.forEach(item => {
        const tag = item.tag || item.value_tag;
        const val = (item.value !== undefined && item.value >= -1e+300) ? item.value : item.last_value;
        if (!tag) return;

        // Міняємо текст на картці перегляду
        const el = document.getElementById(`live-${tag}`);
        if (el) el.textContent = val ?? '—';

        // Міняємо текст у фізичному дереві
        const physEl = document.getElementById(`live-phys-${tag}`);
        if (physEl) physEl.textContent = val ?? '—';

        // Кидаємо точку на графік
        if (typeof appendChartPoint === 'function') {
            appendChartPoint(tag, val);
        }
    });
}

function updateAlertPill(key, label, stateStr, stateMap) {
    const strip = document.getElementById('alert-strip');
    let pill = document.getElementById(`pill-${key}`);
    const cssClass = stateMap[stateStr];

    if (!cssClass) {
        // Норма — прибираємо плашку якщо була
        if (pill) pill.remove();
        return;
    }

    if (!pill) {
        pill = document.createElement('div');
        pill.id = `pill-${key}`;
        pill.className = 'alert-pill';
        strip.appendChild(pill);
    }
    pill.className = `alert-pill ${cssClass}`;
    pill.innerHTML = `<div class="led ${cssClass}"></div><span>${label}: ${stateStr}</span>`;
}

// =====================================================
// ФУТЕР
// =====================================================
function autoSetReportDates() {
    const now = new Date();
    const eightAM = new Date(now);
    eightAM.setHours(8, 0, 0, 0);

    // Якщо зараз ще до 8 ранку — беремо 8 ранку вчора
    if (now < eightAM) {
        eightAM.setDate(eightAM.getDate() - 1);
    }

    // Початок — за добу до кінця
    const startDate = new Date(eightAM);
    startDate.setDate(startDate.getDate() - 1);

    // Форматуємо для input[type=datetime-local]: "YYYY-MM-DDTHH:MM"
    const fmt = d => {
        const pad = n => String(n).padStart(2, '0');
        return `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
    };

    const startEl = document.getElementById('start-time');
    const endEl   = document.getElementById('end-time');
    if (startEl && !startEl.value) startEl.value = fmt(startDate);
    if (endEl   && !endEl.value)   endEl.value   = fmt(eightAM);
}
/*
function initFooter() {
    // Автовиставлення дат
    autoSetReportDates();

    document.getElementById('btn-report').addEventListener('click', () => {
        const start = document.getElementById('start-time').value;
        const end   = document.getElementById('end-time').value;
        if (!start || !end) { alert('Вкажіть початок і кінець звіту'); return; }

        const ids = [...document.querySelectorAll('#subgroup-list input[type=checkbox]:checked')]
            .map(cb => cb.dataset.valueId)
            .filter(Boolean);

        if (!ids.length) { alert('Оберіть хоча б одне значення'); return; }

        const startTs = Math.floor(new Date(start).getTime() / 1000);
        const endTs   = Math.floor(new Date(end).getTime()   / 1000);
        const url = `/measure/?value_ids=${ids.join(',')}&start_time=${startTs}&end_time=${endTs}`;
        window.open(url, '_blank');
    });
}
 */
function initFooter() {
    // Автовиставлення дат (твій оригінальний код)
    autoSetReportDates();

    document.getElementById('btn-report').addEventListener('click', async () => {
        const start = document.getElementById('start-time').value;
        const end   = document.getElementById('end-time').value;

        if (!start || !end) {
            alert('Вкажіть початок і кінець звіту');
            return;
        }

        const checkedCbs = [...document.querySelectorAll('#subgroup-list input[type=checkbox]:checked')];
        const ids = checkedCbs.map(cb => cb.dataset.valueId).filter(Boolean);

        if (!ids.length) {
            alert('Оберіть хоча б одне значення');
            return;
        }

        const startTs = Math.floor(new Date(start).getTime() / 1000);
        const endTs   = Math.floor(new Date(end).getTime() / 1000);

        const selectedValues = checkedCbs.map(cb => ({
            id: parseInt(cb.dataset.valueId),
            value_name: cb.dataset.valueName || `ID #${cb.dataset.valueId}`
        }));

        try {
            // ПРАВИЛЬНИЙ ВИКЛИК apiFetch
            const measures = await apiFetch(
                `/measure/?value_ids=${ids.join(',')}&start_time=${startTs}&end_time=${endTs}`
            );

            // Обробка помилки від apiFetch
            if (!measures || measures.__error) {
                const errorMsg = measures?.message || 'Невідома помилка від сервера';
                throw new Error(errorMsg);
            }

            if (!Array.isArray(measures) || measures.length === 0) {
                alert('Дані за вказаними параметрами не знайдено');
                return;
            }

            const reportWindow = window.open("", "_blank");
            if (reportWindow) {
                generateFrontReport(reportWindow, measures, selectedValues, start, end);
            } else {
                alert('Браузер заблокував спливаюче вікно!');
            }

        } catch (err) {
            alert(`Не вдалося сформувати звіт:\n${err.message}`);
        }
    });
}
// Оновлює рядок вибраних values у футері
function updateFooterSelected() {
    const checked = [...document.querySelectorAll('#subgroup-list input[type=checkbox]:checked')];
    const container = document.getElementById('footer-selected');
    const tagsDiv   = document.getElementById('selected-tags');

    if (!checked.length) {
        container.classList.add('hidden');
        return;
    }

    container.classList.remove('hidden');
    tagsDiv.innerHTML = '';

    checked.forEach(cb => {
        const name = cb.dataset.valueName || cb.dataset.valueTag || cb.dataset.valueId;
        const tag = document.createElement('div');
        tag.className = 'selected-tag';
        tag.innerHTML = `<span>${name}</span><button class="selected-tag-remove" title="Зняти">\u2715</button>`;
        tag.querySelector('button').addEventListener('click', () => {
            cb.checked = false;
            updateFooterSelected();
        });
        tagsDiv.appendChild(tag);
    });
}

// =====================================================
// УТИЛІТИ
// =====================================================
function fillSelect(id, items, mapper) {
    const sel = document.getElementById(id);
    if (!sel) return;
    sel.innerHTML = '<option value="">— оберіть —</option>';
    items.forEach(item => {
        const { value, label } = mapper(item);
        const opt = document.createElement('option');
        opt.value = value;
        opt.textContent = label;
        sel.appendChild(opt);
    });
}

// =====================================================
// AUTH — зберігання токенів
// =====================================================
const auth = {
    getAccess()  { return sessionStorage.getItem('access_token'); },
    getRefresh() { return localStorage.getItem('refresh_token'); },
    setTokens(access, refresh) {
        sessionStorage.setItem('access_token', access);
        if (refresh) localStorage.setItem('refresh_token', refresh);
    },
    clear() {
        sessionStorage.removeItem('access_token');
        localStorage.removeItem('refresh_token');
    },
};

let _refreshing = false;
let _refreshPromise = null;

async function tryRefresh() {
    // Якщо вже йде refresh — чекаємо той самий результат
    if (_refreshing) return _refreshPromise;

    const refreshToken = auth.getRefresh();
    if (!refreshToken) return false;

    _refreshing = true;
    _refreshPromise = (async () => {
        try {
            const res = await fetch(`${API}/auth/refresh_token`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ refresh_token: refreshToken }),
            });
            if (!res.ok) { auth.clear(); return false; }
            const data = await res.json();
            auth.setTokens(data.access_token, data.refresh_token);
            return true;
        } catch {
            auth.clear();
            return false;
        } finally {
            _refreshing = false;
            _refreshPromise = null;
        }
    })();

    return _refreshPromise;
}

function goToLogin() {
    auth.clear();
    document.getElementById('login-overlay').classList.remove('hidden');
}

async function apiFetch(path, options = {}, _retry = false) {
    const headers = { 'Content-Type': 'application/json', ...options.headers };
    const token = auth.getAccess();
    if (token) headers['Authorization'] = `Bearer ${token}`;

    try {
        const res = await fetch(`${API}${path}`, { ...options, headers });
        const text = await res.text();

        if (res.status === 401) {
            if (_retry) { goToLogin(); return { __error: true, status: 401, message: 'Сесія закінчилась' }; }
            const refreshed = await tryRefresh();
            if (refreshed) return apiFetch(path, options, true);
            goToLogin();
            return { __error: true, status: 401, message: 'Сесія закінчилась' };
        }

        if (res.status === 403) {
            return { __error: true, status: 403, message: 'Недостатньо прав доступу' };
        }

        if (!res.ok) {
            return { __error: true, status: res.status, message: text };
        }

        if (!text || text.trim() === '') return { __ok: true };
        try { return JSON.parse(text); }
        catch { return { __ok: true, text }; }
    } catch (err) {
        return { __error: true, status: 0, message: err.message };
    }
}

// Показує модалку результату
function showResult(ok, message = '') {
    const overlay = document.getElementById('modal-confirm-overlay');
    const details = document.getElementById('confirm-details');
    const checkbox = document.getElementById('confirm-checkbox');
    const okBtn = document.getElementById('confirm-ok');
    const cancelBtn = document.getElementById('confirm-cancel');

    if (ok) {
        details.innerHTML = '<span style="color:var(--success);font-size:14px">✓ Виконано успішно</span>';
    } else {
        details.innerHTML = `<span style="color:var(--danger);font-size:13px">✗ Помилка</span><br><pre style="margin-top:8px;font-size:11px;white-space:pre-wrap;color:var(--text-mid)">${message}</pre>`;
    }

    // Ховаємо чекбокс і кнопку скасувати — це тільки інфо
    checkbox.parentElement.classList.add('hidden');
    cancelBtn.classList.add('hidden');
    okBtn.disabled = false;
    okBtn.textContent = 'OK';
    overlay.classList.remove('hidden');

    const onOk = () => {
        okBtn.removeEventListener('click', onOk);
        overlay.classList.add('hidden');
        checkbox.parentElement.classList.remove('hidden');
        cancelBtn.classList.remove('hidden');
        okBtn.textContent = 'Виконати';
    };
    okBtn.addEventListener('click', onOk);
}

function generateFrontReport(win, measures, values, startStr, endStr) {
    const doc = win.document;

    const fmtDate = isoStr => new Date(isoStr).toLocaleString('uk-UA');
    const titleTime = `${fmtDate(startStr)} — ${fmtDate(endStr)}`;

    const colors = ['#2A6496','#C0392B','#2E8B3A','#C07800','#8B2FC9','#1A8C8C','#E67E22','#2980B9'];

    const datasets = measures.map((m, i) => {
        const v = values.find(vv => vv.id === m.id);
        const color = colors[i % colors.length];
        return {
            label: v?.value_name ?? `#${m.id}`,
            data: (m.values || []).map(p => ({
                x: p.timestamp * 1000,
                y: (p.val === null || p.val === undefined || p.val < -1e+300) ? null : p.val
            })),
            borderColor: color,
            backgroundColor: color + '22',
            borderWidth: 2,
            tension: 0.2,
            pointRadius: 0,
            spanGaps: false
        };
    });

    doc.open();
    doc.write(`
<!DOCTYPE html>
<html lang="uk">
<head>
    <meta charset="UTF-8">
    <title>Звіт вимірів</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0"></script>
    <script src="https://cdn.jsdelivr.net/npm/chartjs-adapter-date-fns@3.0.0"></script>
    <style>
        body { 
            font-family: 'JetBrains Mono', monospace; 
            background: #f0f0f0; 
            margin: 20px; 
            display: flex; 
            justify-content: center;
        }
        
        .a4-page {
            width: 297mm;
            height: 210mm;
            background: white;
            box-shadow: 0 0 20px rgba(0,0,0,0.25);
            border: 1px solid #aaa;
            overflow: hidden;
        }

        .content {
            padding: 12mm 15mm;
            height: 100%;
            display: flex;
            flex-direction: column;
        }

        .header {
            margin-bottom: 6mm;
            padding-bottom: 6mm;
            border-bottom: 1px solid #ddd;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }

        .legend-container {
            margin-bottom: 8mm;
        }

        .legend-grid { display: flex; flex-wrap: wrap; gap: 8px; }
        .legend-badge { 
            display: flex; align-items: center; gap: 6px; 
            background: #f8f8f8; border: 1px solid #ddd; 
            padding: 4px 10px; border-radius: 4px; font-size: 11.5px;
        }
        .legend-color { width: 14px; height: 14px; border-radius: 3px; border: 2px solid #333; }

        /* ФІКСОВАНИЙ РОЗМІР ГРАФІКА */
        .chart-container {
            width: 100%;
            height: 500px !important;     /* ← ЗМІНЮЙ ЦЕ ЗНАЧЕННЯ */
            position: relative;
        }

        .btn-print {
            padding: 8px 18px;
            background: #2A6496;
            color: white;
            border: none;
            border-radius: 4px;
            cursor: pointer;
        }

        @media print {
            body { margin:0; background:white; }
            .a4-page { box-shadow:none; border:none; width:100%; height:100%; }
            .btn-print { display:none !important; }
            .chart-container { height: 500px !important; }
        }
    </style>
</head>
<body>
    <div class="a4-page">
        <div class="content">
            <div class="header">
                <div>
                    <h1 style="margin:0 0 5px 0; font-size:24px;">Звіт вимірів</h1>
                    <p style="margin:0; color:#555;">${titleTime}</p>
                </div>
                <button onclick="window.print()" class="btn-print">🖨️ Друк / PDF</button>
            </div>

            <div class="legend-container">
                <div class="legend-grid" id="legend-grid"></div>
            </div>

            <div class="chart-container">
                <canvas id="reportChart"></canvas>
            </div>
        </div>
    </div>

    <script>
        const datasets = ${JSON.stringify(datasets)};
        const ctx = document.getElementById('reportChart').getContext('2d');

        setTimeout(() => {
            new Chart(ctx, {
                type: 'line',
                data: { datasets },
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    animation: false,
                    scales: {
                        x: {
                            type: 'time',
                            time: { unit: 'minute', stepSize: 10, displayFormats: { minute: 'HH:mm' } },
                            grid: { color: '#eee' },
                            ticks: { font: { size: 10 } }
                        },
                        y: {
                            grid: { color: '#eee' },
                            ticks: { font: { size: 10 }, stepSize: 5 }
                        }
                    }
                }
            });

            const grid = document.getElementById('legend-grid');
            datasets.forEach(d => {
                grid.innerHTML += \`
                    <div class="legend-badge">
                        <div class="legend-color" style="background:\${d.borderColor}"></div>
                        <strong>\${d.label}</strong>
                    </div>
                \`;
            });
        }, 400);
    </script>
</body>
</html>`);
    doc.close();
}