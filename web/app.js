/* HARPOON — zero-build RTS fleet command client.
 * The server is authoritative. This client only ever renders the per-side
 * Observation it receives; contact positions remain estimates.
 */
(() => {
  'use strict';

  // ------------------------------------------------------------------
  // Constants and tiny helpers
  // ------------------------------------------------------------------
  const MONO = '"SFMono-Regular", "Cascadia Mono", "Roboto Mono", Menlo, Consolas, monospace';
  const MAX_EVENTS = 400;
  const MAX_WAYPOINTS = 24;
  const POLL_MS = 500;
  const RECONNECT_MS = 3000;
  const SIDE_STYLE = {
    Blue: { hex: '#4bcaff', rgb: '75,202,255' },
    Red: { hex: '#ff7162', rgb: '255,113,98' },
  };

  const CATEGORY_ALIASES = {
    ship: 'surface',
    surface: 'surface',
    aircraft: 'air',
    air: 'air',
    sub: 'subsurface',
    subsurface: 'subsurface',
    submarine: 'subsurface',
    missile: 'missile',
    torpedo: 'torpedo',
    base: 'land',
    land: 'land',
  };

  const CATEGORY_DETAILS = {
    surface: { key: 'surface', label: 'Surface', short: 'SRF', target: 'surface', shape: 'rect', color: '255,151,120' },
    air: { key: 'air', label: 'Air', short: 'AIR', target: 'air', shape: 'triangle', color: '242,182,96' },
    subsurface: { key: 'subsurface', label: 'Subsurface', short: 'SUB', target: 'sub', shape: 'diamond', color: '199,148,255' },
    missile: { key: 'missile', label: 'Missile', short: 'MSL', target: 'missile', shape: 'circle', color: '255,102,108' },
    torpedo: { key: 'torpedo', label: 'Torpedo', short: 'TOR', target: 'torpedo', shape: 'circle', color: '255,147,164' },
    land: { key: 'land', label: 'Land', short: 'LND', target: 'land', shape: 'square', color: '113,212,160' },
    unknown: { key: 'unknown', label: 'Unknown', short: 'UNK', target: null, shape: 'diamond', color: '172,201,213' },
  };

  const $ = (id) => document.getElementById(id);

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function numberOr(value, fallback = 0) {
    const n = Number(value);
    return Number.isFinite(n) ? n : fallback;
  }

  function finiteOrNull(value) {
    if (value === null || value === undefined || value === '') return null;
    const n = Number(value);
    return Number.isFinite(n) ? n : null;
  }

  function escapeHtml(value) {
    return String(value ?? '').replace(/[&<>"']/g, (ch) => {
      switch (ch) {
        case '&': return '&amp;';
        case '<': return '&lt;';
        case '>': return '&gt;';
        case '"': return '&quot;';
        default: return '&#39;';
      }
    });
  }

  function categoryInfo(raw) {
    const key = String(raw ?? '').trim().toLowerCase();
    return CATEGORY_DETAILS[CATEGORY_ALIASES[key]] || CATEGORY_DETAILS.unknown;
  }

  function fmt1(value) {
    const n = finiteOrNull(value);
    return n === null ? '—' : n.toFixed(1);
  }

  function fmtInt(value) {
    const n = finiteOrNull(value);
    return n === null ? '—' : String(Math.round(n));
  }

  function fmtRange(value) {
    const n = finiteOrNull(value);
    if (n === null) return '—';
    return n >= 100 ? n.toFixed(0) : n.toFixed(1);
  }

  function fmtCourse(value) {
    const n = finiteOrNull(value);
    if (n === null) return '—';
    return `${String(Math.round(((n % 360) + 360) % 360)).padStart(3, '0')}°`;
  }

  function fmtClock(seconds) {
    const total = Math.max(0, Math.floor(numberOr(seconds, 0)));
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    return `${String(h).padStart(3, '0')}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }

  function fmtEventTick(tick) {
    const total = Math.max(0, Math.floor(numberOr(tick, 0)));
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    return `T+${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }

  function fmtAge(seconds) {
    const n = finiteOrNull(seconds);
    if (n === null) return '—';
    if (n < 5) return 'JUST NOW';
    if (n < 60) return `${Math.floor(n)}s AGO`;
    if (n < 3600) return `${Math.floor(n / 60)}m ${Math.floor(n % 60)}s AGO`;
    return `${Math.floor(n / 3600)}h AGO`;
  }

  function fmtEta(seconds) {
    const n = finiteOrNull(seconds);
    if (n === null) return '—';
    if (n < 60) return `${Math.max(0, Math.round(n))}s`;
    return `${Math.floor(n / 60)}m ${Math.round(n % 60)}s`;
  }

  function titleCaseWords(text) {
    return String(text ?? '')
      .replace(/_/g, ' ')
      .replace(/\b\w/g, (ch) => ch.toUpperCase());
  }

  function truncate(text, max) {
    const s = String(text ?? '');
    return s.length > max ? `${s.slice(0, Math.max(1, max - 1))}…` : s;
  }

  function sideStyle(side) {
    return SIDE_STYLE[side] || SIDE_STYLE.Blue;
  }

  function isObject(value) {
    return value !== null && typeof value === 'object';
  }

  function readStorage(key) {
    try {
      return window.localStorage.getItem(key);
    } catch {
      return null;
    }
  }

  function writeStorage(key, value) {
    try {
      window.localStorage.setItem(key, value);
    } catch {
      /* storage is optional */
    }
  }

  class ApiError extends Error {
    constructor(message, status, body) {
      super(message);
      this.name = 'ApiError';
      this.status = status;
      this.body = body;
    }
  }

  function errorText(err) {
    if (err instanceof ApiError) {
      if (err.status === 409) return 'No game is running on the server.';
      return err.message || `HTTP ${err.status}`;
    }
    if (err && err.name === 'AbortError') return 'Request timed out.';
    return err && err.message ? err.message : 'Network error.';
  }

  // ------------------------------------------------------------------
  // DOM references
  // ------------------------------------------------------------------
  const dom = {
    sideToggle: $('side-toggle'),
    scenarioSelect: $('scenario-select'),
    btnStart: $('btn-start'),
    speedGroup: $('speed-group'),
    tickValue: $('tick-value'),
    clockValue: $('clock-value'),
    overChip: $('over-chip'),
    linkChip: $('link-chip'),
    linkText: $('link-text'),

    mapShell: $('map-shell'),
    mapCanvas: $('map-canvas'),
    mapScenario: $('map-scenario'),
    mapDesc: $('map-desc'),
    weatherChips: $('weather-chips'),
    outcomePanel: $('outcome-panel'),
    outcomeTitle: $('outcome-title'),
    outcomeReason: $('outcome-reason'),
    outcomeVp: $('outcome-vp'),
    mapFit: $('map-fit'),
    mapZoomIn: $('map-zoom-in'),
    mapZoomOut: $('map-zoom-out'),
    scaleBar: $('scale-bar'),
    scaleText: $('scale-text'),
    cursorReadout: $('cursor-readout'),
    toastStack: $('toast-stack'),

    unitPanel: $('unit-panel'),
    unitCount: $('unit-count'),
    unitEmpty: $('unit-empty'),
    unitContent: $('unit-content'),
    unitKindBadge: $('unit-kind-badge'),
    unitName: $('unit-name'),
    unitClass: $('unit-class'),
    unitId: $('unit-id'),
    unitSideBadge: $('unit-side-badge'),
    unitHpBar: $('unit-hp-bar'),
    unitHpText: $('unit-hp-text'),
    unitSpeed: $('unit-speed'),
    unitSpeedOrder: $('unit-speed-order'),
    unitCourse: $('unit-course'),
    unitCourseOrder: $('unit-course-order'),
    unitDepthLabel: $('unit-depth-label'),
    unitDepth: $('unit-depth'),
    unitDepthOrder: $('unit-depth-order'),
    unitChips: $('unit-chips'),

    targetPanel: $('target-panel'),
    targetQualityChip: $('target-quality-chip'),
    targetEmpty: $('target-empty'),
    targetContent: $('target-content'),
    targetSymbol: $('target-symbol'),
    targetName: $('target-name'),
    targetSub: $('target-sub'),
    targetLockTag: $('target-lock-tag'),
    targetQualityBar: $('target-quality-bar'),
    targetQualityText: $('target-quality-text'),
    targetRange: $('target-range'),
    targetCourse: $('target-course'),
    targetSpeed: $('target-speed'),
    targetLastseen: $('target-lastseen'),
    targetTags: $('target-tags'),
    btnAssign: $('btn-assign'),
    btnClearTarget: $('btn-clear-target'),
    fireWeapon: $('fire-weapon'),
    btnFire: $('btn-fire'),
    fireNote: $('fire-note'),

    ordersPanel: $('orders-panel'),
    weaponsStateChip: $('weapons-state-chip'),
    btnWeaponsFree: $('btn-weapons-free'),
    weaponsFreeState: $('weapons-free-state'),
    btnChaff: $('btn-chaff'),
    chaffCount: $('chaff-count'),
    btnStop: $('btn-stop'),
    btnRecover: $('btn-recover'),
    recoverState: $('recover-state'),
    assignedTarget: $('assigned-target'),

    weaponsPanel: $('weapons-panel'),
    weaponsCount: $('weapons-count'),
    weaponsList: $('weapons-list'),
    sensorsPanel: $('sensors-panel'),
    sensorsCount: $('sensors-count'),
    sensorsList: $('sensors-list'),
    missionPanel: $('mission-panel'),
    missionValue: $('mission-value'),
    missionDetail: $('mission-detail'),
    deckPanel: $('deck-panel'),
    deckList: $('deck-list'),
    systemsPanel: $('systems-panel'),
    systemsList: $('systems-list'),

    logSideBadge: $('log-side-badge'),
    logMeta: $('log-meta'),
    eventLog: $('event-log'),
    logEmpty: $('log-empty'),
    eventList: $('event-list'),
  };

  // ------------------------------------------------------------------
  // Application state
  // ------------------------------------------------------------------
  const state = {
    side: readStorage('harpoon.side') === 'Red' ? 'Red' : 'Blue',
    scenarios: [],
    scenarioId: null,
    observation: null,
    db: null,
    platformById: new Map(),
    platformByName: new Map(),
    weaponById: new Map(),
    sensorById: new Map(),
    selectedUnitId: null,
    selectedContactId: null,
    fireWeaponId: null,
    fireOptionsKey: null,
    speed: 60,
    eventMap: new Map(),
    routes: new Map(),
    gotoQueues: new Map(),
    startingScenario: false,
    gameGeneration: 0,
    transport: '',
  };

  const map = {
    ctx: null,
    w: 0,
    h: 0,
    dpr: 1,
    view: { cx: 24, cy: 24, scale: 6 },
    viewInitialized: false,
    fitPending: true,
    userAdjusted: false,
    seenContacts: new Set(),
    smooth: new Map(),
    hover: null,
    cursorWorld: null,
    pointerInside: false,
    drag: null,
    lastFrame: 0,
    lastScaleKey: '',
  };

  let socket = null;
  let pollTimer = null;
  let reconnectTimer = null;
  let transportEpoch = 0;
  let pollBusy = false;
  let refreshBusy = false;

  // ------------------------------------------------------------------
  // API plumbing
  // ------------------------------------------------------------------
  async function requestJson(url, options = {}) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 7000);
    let response;
    try {
      response = await fetch(url, {
        cache: 'no-store',
        ...options,
        signal: options.signal || controller.signal,
      });
    } finally {
      clearTimeout(timer);
    }

    const text = await response.text();
    let data = null;
    if (text) {
      try {
        data = JSON.parse(text);
      } catch {
        data = text;
      }
    }

    if (!response.ok) {
      let message = '';
      if (isObject(data)) message = data.error || data.message || JSON.stringify(data);
      else if (typeof data === 'string') message = data;
      if (!message) message = `HTTP ${response.status}`;
      throw new ApiError(message, response.status, data);
    }
    return data;
  }

  function postJson(url, payload) {
    return requestJson(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  }

  function fetchObservation(side = state.side) {
    return requestJson(`/api/state?side=${encodeURIComponent(side)}&since=0&max_events=200`);
  }

  async function fetchTime() {
    const result = await requestJson('/api/time');
    if (isObject(result) && Number.isFinite(Number(result.speed))) {
      state.speed = Number(result.speed);
      renderSpeedControls();
    }
    return result;
  }

  async function syncTime() {
    try {
      await fetchTime();
    } catch {
      /* the observation feed is the primary status source */
    }
  }

  async function loadScenarios() {
    try {
      const list = await requestJson('/api/scenarios');
      const scenarios = (Array.isArray(list) ? list : []).filter((sc) => sc && sc.id);
      scenarios.sort((a, b) => String(a.name || a.id).localeCompare(String(b.name || b.id)));
      state.scenarios = scenarios;

      dom.scenarioSelect.replaceChildren();
      if (!scenarios.length) {
        const option = document.createElement('option');
        option.textContent = 'No scenarios';
        option.value = '';
        dom.scenarioSelect.appendChild(option);
        dom.scenarioSelect.disabled = true;
        dom.btnStart.disabled = true;
        showToast('No scenarios are available in the server scenario directory.', 'error', 5000);
        return;
      }

      const stored = readStorage('harpoon.scenario');
      const preferred = scenarios.find((sc) => sc.id === state.scenarioId)
        || scenarios.find((sc) => sc.id === stored)
        || scenarios[0];
      state.scenarioId = preferred ? preferred.id : null;

      for (const scenario of scenarios) {
        const option = document.createElement('option');
        option.value = scenario.id;
        option.textContent = scenario.name || scenario.id;
        option.title = scenario.description || scenario.id;
        dom.scenarioSelect.appendChild(option);
      }

      if (state.scenarioId) dom.scenarioSelect.value = state.scenarioId;
      dom.scenarioSelect.disabled = false;
      dom.btnStart.disabled = false;
      updateMapBrief(state.observation);
    } catch (err) {
      state.scenarios = [];
      dom.scenarioSelect.replaceChildren();
      const option = document.createElement('option');
      option.textContent = 'Scenario feed unavailable';
      option.value = '';
      dom.scenarioSelect.appendChild(option);
      dom.scenarioSelect.disabled = true;
      dom.btnStart.disabled = true;
      showToast(`Unable to load scenarios: ${errorText(err)}`, 'error', 5000);
    }
  }

  async function loadDatabase() {
    try {
      const db = await requestJson('/api/db');
      if (!isObject(db)) return;
      state.db = db;
      state.platformById = new Map();
      state.platformByName = new Map();
      state.weaponById = new Map();
      state.sensorById = new Map();

      for (const platform of Object.values(db.platforms || {})) {
        if (!platform || !platform.id) continue;
        state.platformById.set(String(platform.id), platform);
        state.platformByName.set(String(platform.name || platform.id).toLowerCase(), platform);
        state.platformByName.set(String(platform.id).toLowerCase(), platform);
      }
      for (const weapon of Object.values(db.weapons || {})) {
        if (weapon && weapon.id) state.weaponById.set(String(weapon.id), weapon);
      }
      for (const sensor of Object.values(db.sensors || {})) {
        if (sensor && sensor.id) state.sensorById.set(String(sensor.id), sensor);
      }
      state.fireOptionsKey = null;
      renderSidebar();
    } catch {
      /* labels and fire compatibility gracefully fall back to observation data */
    }
  }

  // ------------------------------------------------------------------
  // Feed: WebSocket first, 500 ms HTTP polling as fallback
  // ------------------------------------------------------------------
  function setLink(mode, label, title) {
    state.transport = mode;
    dom.linkChip.dataset.mode = mode;
    dom.linkText.textContent = label;
    dom.linkChip.title = title || label;
  }

  function detachSocket(ws) {
    if (!ws) return;
    ws.onopen = null;
    ws.onmessage = null;
    ws.onerror = null;
    ws.onclose = null;
    try {
      ws.close();
    } catch {
      /* already closed */
    }
  }

  function restartTransport() {
    transportEpoch += 1;
    const epoch = transportEpoch;

    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
    if (reconnectTimer !== null) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    if (socket) {
      detachSocket(socket);
      socket = null;
    }
    pollBusy = false;
    refreshBusy = false;

    setLink('connecting', 'LINKING', 'Connecting to the WebSocket feed…');
    startPolling(epoch);
    openSocket(epoch);
  }

  function startPolling(epoch) {
    if (pollTimer !== null || epoch !== transportEpoch) return;
    void pollOnce(epoch);
    pollTimer = window.setInterval(() => {
      void pollOnce(epoch);
    }, POLL_MS);
  }

  function stopPolling() {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  async function pollOnce(epoch) {
    if (epoch !== transportEpoch || pollBusy) return;
    pollBusy = true;
    const side = state.side;
    try {
      const obs = await fetchObservation(side);
      if (epoch !== transportEpoch || side !== state.side) return;
      applyObservation(obs);
      if (socket && socket.readyState === WebSocket.OPEN) {
        setLink('ws', 'WS LIVE', 'WebSocket feed connected');
      } else {
        setLink('poll', 'POLL 2HZ', 'WebSocket unavailable — polling /api/state every 500 ms');
      }
    } catch (err) {
      if (epoch !== transportEpoch || side !== state.side) return;
      if (err instanceof ApiError && err.status === 409) {
        setLink('poll', 'NO GAME', 'No game running — choose a scenario');
      } else if (!(socket && socket.readyState === WebSocket.OPEN)) {
        setLink('offline', 'LINK LOST', `Feed unavailable: ${errorText(err)}`);
      }
    } finally {
      pollBusy = false;
    }
  }

  function scheduleReconnect(epoch) {
    if (epoch !== transportEpoch || reconnectTimer !== null) return;
    reconnectTimer = window.setTimeout(() => {
      reconnectTimer = null;
      if (epoch === transportEpoch) openSocket(epoch);
    }, RECONNECT_MS);
  }

  function openSocket(epoch) {
    if (epoch !== transportEpoch) return;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    let ws;
    try {
      ws = new WebSocket(`${protocol}//${window.location.host}/ws?side=${encodeURIComponent(state.side)}`);
    } catch {
      scheduleReconnect(epoch);
      return;
    }
    socket = ws;

    ws.onopen = () => {
      if (epoch !== transportEpoch || socket !== ws) {
        detachSocket(ws);
        return;
      }
      stopPolling();
      setLink('ws', 'WS LIVE', 'WebSocket feed connected');
      void refreshState(epoch);
      void syncTime();
    };

    ws.onmessage = (event) => {
      if (epoch !== transportEpoch || socket !== ws) return;
      if (typeof event.data !== 'string') return;
      try {
        const obs = JSON.parse(event.data);
        applyObservation(obs);
      } catch {
        /* ignore malformed frame */
      }
    };

    ws.onerror = () => {
      /* close usually follows; polling is already active while disconnected */
    };

    ws.onclose = () => {
      if (epoch !== transportEpoch || socket !== ws) return;
      socket = null;
      startPolling(epoch);
      setLink('poll', 'POLL 2HZ', 'WebSocket lost — polling /api/state every 500 ms');
      scheduleReconnect(epoch);
    };
  }

  async function refreshState(epoch = transportEpoch) {
    if (refreshBusy) return;
    refreshBusy = true;
    const side = state.side;
    try {
      const obs = await fetchObservation(side);
      if (epoch !== transportEpoch || side !== state.side) return;
      applyObservation(obs);
    } catch {
      /* polling / reconnect will retry */
    } finally {
      refreshBusy = false;
    }
  }

  // ------------------------------------------------------------------
  // Observation lifecycle
  // ------------------------------------------------------------------
  async function bootstrap() {
    await Promise.all([loadScenarios(), loadDatabase()]);

    let initialObservation = null;
    let needsGame = false;
    try {
      initialObservation = await fetchObservation(state.side);
    } catch (err) {
      if (err instanceof ApiError && err.status === 409) {
        needsGame = true;
      } else {
        showToast(`Unable to reach the referee: ${errorText(err)}`, 'error', 5000);
      }
    }

    if (initialObservation) {
      applyObservation(initialObservation);
      await syncTime();
    } else if (needsGame && state.scenarios.length) {
      const preferred = state.scenarioId || state.scenarios[0].id;
      await startScenario(preferred, { initial: true });
      await syncTime();
    }

    restartTransport();
  }

  async function startScenario(id, options = {}) {
    const { initial = false } = options;
    if (!id || state.startingScenario) return false;
    if (!state.scenarios.some((sc) => sc.id === id)) {
      showToast('That scenario is not present on the server.', 'error');
      return false;
    }

    const previousScenario = state.observation
      ? state.observation.scenario_id
      : state.scenarioId;
    state.startingScenario = true;
    updateScenarioControls();
    let started = false;

    try {
      await postJson('/api/game', { scenario: id, seed: 123, speed: 60 });

      state.scenarioId = id;
      state.speed = 60;
      renderSpeedControls();

      resetRuntime();
      dom.scenarioSelect.value = id;
      writeStorage('harpoon.scenario', id);

      if (!initial) restartTransport();

      const scenario = state.scenarios.find((sc) => sc.id === id);
      showToast(`SCENARIO DEPLOYED · ${scenario ? scenario.name : id}`, 'ok');

      const [obs] = await Promise.all([
        fetchObservation(state.side).catch(() => null),
        syncTime(),
      ]);
      if (obs && obs.your_side === state.side) applyObservation(obs);
      started = true;
    } catch (err) {
      showToast(`Unable to start scenario: ${errorText(err)}`, 'error', 5000);
      if (previousScenario && state.scenarios.some((sc) => sc.id === previousScenario)) {
        dom.scenarioSelect.value = previousScenario;
        state.scenarioId = previousScenario;
      }
    } finally {
      state.startingScenario = false;
      updateScenarioControls();
      updateMapBrief(state.observation);
    }

    return started;
  }

  function resetRuntime() {
    state.observation = null;
    state.selectedUnitId = null;
    state.selectedContactId = null;
    state.fireWeaponId = null;
    state.fireOptionsKey = null;
    state.routes.clear();
    state.gotoQueues.clear();
    state.gameGeneration += 1;

    clearEventLog();
    updateHeader(null);
    updateOutcome(null);
    renderSidebar();
    updateMapBrief(null);

    map.smooth.clear();
    map.hover = null;
    map.cursorWorld = null;
    map.seenContacts.clear();
    map.fitPending = true;
    map.userAdjusted = false;
    map.lastScaleKey = '';
  }

  function applyObservation(obs) {
    if (!isObject(obs)) return;
    if (obs.your_side && obs.your_side !== state.side) return;

    const previous = state.observation;
    if (
      previous
      && previous.scenario_id === obs.scenario_id
      && Number.isFinite(Number(previous.tick))
      && Number.isFinite(Number(obs.tick))
      && Number(obs.tick) < Number(previous.tick)
    ) {
      return;
    }

    state.observation = obs;
    if (obs.scenario_id) {
      state.scenarioId = obs.scenario_id;
      if (dom.scenarioSelect.value !== obs.scenario_id) {
        const optionExists = Array.from(dom.scenarioSelect.options).some((option) => option.value === obs.scenario_id);
        if (optionExists) dom.scenarioSelect.value = obs.scenario_id;
      }
    }

    mergeEvents(Array.isArray(obs.events) ? obs.events : []);
    reconcileSelection(obs);
    updateHeader(obs);
    updateMapBrief(obs);
    updateOutcome(obs);
    pruneRoutes(obs);
    renderSidebar();
    maybeAutoFit();
  }

  function reconcileSelection(obs) {
    const units = Array.isArray(obs.units) ? obs.units : [];
    const contacts = Array.isArray(obs.contacts) ? obs.contacts : [];

    if (state.selectedUnitId !== null && !units.some((u) => Number(u.id) === state.selectedUnitId)) {
      state.selectedUnitId = null;
      state.fireOptionsKey = null;
    }
    if (state.selectedContactId !== null && !contacts.some((c) => Number(c.id) === state.selectedContactId)) {
      state.selectedContactId = null;
      state.fireOptionsKey = null;
    }
  }

  function selectedUnit() {
    const units = state.observation && Array.isArray(state.observation.units)
      ? state.observation.units
      : [];
    return units.find((u) => Number(u.id) === state.selectedUnitId) || null;
  }

  function selectedContact() {
    const contacts = state.observation && Array.isArray(state.observation.contacts)
      ? state.observation.contacts
      : [];
    return contacts.find((c) => Number(c.id) === state.selectedContactId) || null;
  }

  function findContactById(id) {
    if (id === null || id === undefined) return null;
    const contacts = state.observation && Array.isArray(state.observation.contacts)
      ? state.observation.contacts
      : [];
    return contacts.find((c) => Number(c.id) === Number(id)) || null;
  }

  function platformForUnit(unit) {
    if (!unit) return null;
    return state.platformById.get(String(unit.class || '')) || null;
  }

  function platformForContact(contact) {
    if (!contact || !contact.class_name) return null;
    return state.platformByName.get(String(contact.class_name).toLowerCase()) || null;
  }

  function weaponDefinition(id) {
    return state.weaponById.get(String(id)) || null;
  }

  function weaponName(id) {
    const def = weaponDefinition(id);
    return def && def.name ? def.name : titleCaseWords(id || 'Weapon');
  }

  function sensorName(id) {
    const def = state.sensorById.get(String(id));
    return def && def.name ? def.name : titleCaseWords(id || 'Sensor');
  }

  function aircraftName(id) {
    const def = state.platformById.get(String(id));
    return def && def.name ? def.name : titleCaseWords(id || 'Aircraft');
  }

  function unitKindCode(unit) {
    const classId = String(unit.class || '').toLowerCase();
    const first = classId.split('_')[0];
    const abbreviations = {
      ddg: 'DDG',
      cvn: 'CVN',
      ffg: 'FFG',
      mgb: 'MGB',
      ss: 'SSK',
      sub: 'SSK',
      fighter: 'FTR',
      helo: 'HELO',
    };
    if (abbreviations[first]) return abbreviations[first];
    const kind = String(unit.kind || 'UNIT').toUpperCase();
    if (kind === 'AIRCRAFT') return 'AIR';
    if (kind === 'SUBMARINE' || kind === 'SUB') return 'SSK';
    return kind.slice(0, 4);
  }

  function contactSymbolSize(contact) {
    const cat = categoryInfo(contact.category);
    const baseByShape = {
      rect: 8.2,
      triangle: 8.6,
      diamond: 8.8,
      circle: cat.key === 'torpedo' ? 3.8 : 5.2,
      square: 8.6,
    };
    let factor = contact.class_name ? 1.2 : 0.72;
    const platform = platformForContact(contact);
    if (platform) {
      const rcsScale = { Large: 1.42, Medium: 1.16, Small: 0.95, VerySmall: 0.82 };
      factor *= rcsScale[platform.rcs] || 1;
    }
    return clamp(baseByShape[cat.shape] * factor, 3.1, 18);
  }

  // ------------------------------------------------------------------
  // Header / brief / outcome
  // ------------------------------------------------------------------
  function updateHeader(obs) {
    state.observation = obs;
    dom.tickValue.textContent = obs ? String(Math.max(0, Math.floor(numberOr(obs.tick, 0)))).padStart(6, '0') : '000000';
    dom.clockValue.textContent = obs ? fmtClock(obs.time_s) : '000:00:00';

    if (obs && obs.over) {
      dom.overChip.hidden = false;
      const result = isObject(obs.result) ? obs.result : null;
      dom.overChip.textContent = result && result.winner ? `${String(result.winner).toUpperCase()} WIN` : 'OVER';
      dom.overChip.title = result
        ? `${result.winner || 'Draw'} · ${result.reason || 'Scenario complete'}`
        : 'Scenario complete';
    } else {
      dom.overChip.hidden = true;
      dom.overChip.title = '';
    }
  }

  function updateMapBrief(obs) {
    const scenario = state.scenarios.find((sc) => sc.id === (obs ? obs.scenario_id : state.scenarioId));
    const title = obs && obs.scenario_name
      ? obs.scenario_name
      : scenario
        ? scenario.name || scenario.id
        : 'No scenario';
    dom.mapScenario.textContent = title;
    dom.mapScenario.title = title;

    const description = scenario && scenario.description
      ? scenario.description
      : obs
        ? ''
        : 'Awaiting deployment orders.';
    dom.mapDesc.textContent = description || 'Double-blind tactical picture.';
    dom.mapDesc.title = description;

    if (!obs) {
      dom.weatherChips.innerHTML = scenario ? '<span class="wx-chip">AWAITING FEED</span>' : '';
      return;
    }

    const weather = isObject(obs.weather) ? obs.weather : {};
    const jam = Array.isArray(weather.jam) ? weather.jam : [0, 0];
    const visibility = finiteOrNull(weather.visibility_nm);
    const dayNight = obs.day_night === 'Night' ? 'NIGHT' : 'DAY';
    const blueJam = Math.round(clamp(numberOr(jam[0], 0), 0, 1) * 100);
    const redJam = Math.round(clamp(numberOr(jam[1], 0), 0, 1) * 100);
    dom.weatherChips.innerHTML = [
      `<span class="wx-chip">VIS ${visibility === null ? '—' : fmtRange(visibility)} NM</span>`,
      `<span class="wx-chip">${dayNight}</span>`,
      `<span class="wx-chip">JAM B${blueJam} / R${redJam}</span>`,
    ].join('');
  }

  function updateOutcome(obs) {
    if (!obs || !obs.over) {
      dom.outcomePanel.hidden = true;
      dom.outcomePanel.removeAttribute('data-winner');
      dom.outcomeTitle.style.color = '';
      return;
    }

    const result = isObject(obs.result) ? obs.result : null;
    const winner = result && result.winner ? String(result.winner) : null;
    const playerWon = winner && winner === state.side;

    dom.outcomePanel.hidden = false;
    dom.outcomePanel.dataset.winner = winner || 'Draw';

    if (winner) {
      dom.outcomeTitle.textContent = `${winner.toUpperCase()} VICTORY`;
      dom.outcomeTitle.style.color = winner === 'Blue' ? '#4bcaff' : '#ff7162';
    } else {
      dom.outcomeTitle.textContent = 'DRAW / CEASEFIRE';
      dom.outcomeTitle.style.color = '#f2b660';
    }

    dom.outcomeReason.textContent = result && result.reason
      ? result.reason
      : 'Scenario complete.';
    dom.outcomeVp.textContent = result
      ? `VP BLUE ${fmtRange(result.vp_blue)}  ·  RED ${fmtRange(result.vp_red)}${playerWon ? '  ·  COMMAND OBJECTIVE MET' : ''}`
      : 'FINAL SCORE UNAVAILABLE';
  }

  function renderSpeedControls() {
    const buttons = dom.speedGroup.querySelectorAll('button[data-speed]');
    buttons.forEach((button) => {
      const value = Number(button.dataset.speed);
      const active = value === state.speed;
      button.classList.toggle('is-active', active);
      button.setAttribute('aria-pressed', String(active));
    });
  }

  async function setSpeed(speed) {
    const requested = Number(speed);
    if (!Number.isFinite(requested)) return;
    try {
      const result = await postJson('/api/time', { speed: requested });
      state.speed = isObject(result) && Number.isFinite(Number(result.speed))
        ? Number(result.speed)
        : requested;
      renderSpeedControls();
    } catch (err) {
      showToast(`Time control failed: ${errorText(err)}`, 'error');
    }
  }

  function updateScenarioControls() {
    const unavailable = state.scenarios.length === 0;
    dom.scenarioSelect.disabled = state.startingScenario || unavailable;
    dom.btnStart.disabled = state.startingScenario || unavailable;
    dom.btnStart.title = state.observation ? 'Restart selected scenario' : 'Start selected scenario';
  }

  // ------------------------------------------------------------------
  // Side switching
  // ------------------------------------------------------------------
  function setSide(side, options = {}) {
    const { announce = true } = options;
    if (side !== 'Blue' && side !== 'Red') return;
    if (side === state.side) {
      updateSideControls();
      return;
    }

    state.side = side;
    writeStorage('harpoon.side', side);
    document.body.dataset.side = side;
    updateSideControls();
    resetRuntime();
    updateMapBrief(null);
    if (announce) showToast(`COMMANDING ${side.toUpperCase()} FORCES`, 'warn');
    restartTransport();
  }

  function updateSideControls() {
    const buttons = dom.sideToggle.querySelectorAll('button[data-side]');
    buttons.forEach((button) => {
      const active = button.dataset.side === state.side;
      button.classList.toggle('is-active', active);
      button.setAttribute('aria-pressed', String(active));
    });
    dom.logSideBadge.textContent = `${state.side.toUpperCase()} + ALL`;
    dom.logSideBadge.style.color = '';
  }

  // ------------------------------------------------------------------
  // Sidebar rendering
  // ------------------------------------------------------------------
  function renderSidebar() {
    const obs = state.observation;
    const units = obs && Array.isArray(obs.units) ? obs.units : [];
    const unit = selectedUnit();
    const contact = selectedContact();
    const over = !!(obs && obs.over);

    dom.unitCount.textContent = `${units.length} IN PICTURE`;

    if (!unit) {
      dom.unitEmpty.hidden = false;
      dom.unitContent.hidden = true;
      dom.ordersPanel.hidden = true;
      dom.weaponsPanel.hidden = true;
      dom.sensorsPanel.hidden = true;
      dom.missionPanel.hidden = true;
      dom.deckPanel.hidden = true;
      dom.systemsPanel.hidden = true;
    } else {
      dom.unitEmpty.hidden = true;
      dom.unitContent.hidden = false;
      renderUnitDetails(unit, obs);
      renderOrders(unit, over);
      renderWeapons(unit);
      renderSensors(unit);
      renderMission(unit);
      renderDeck(unit, obs);
      renderSystems(unit);
      dom.ordersPanel.hidden = false;
      dom.weaponsPanel.hidden = false;
      dom.sensorsPanel.hidden = false;
      dom.missionPanel.hidden = false;
      dom.deckPanel.hidden = false;
      dom.systemsPanel.hidden = false;
    }

    renderTargetPanel(unit, contact, obs);
  }

  function renderUnitDetails(unit, obs) {
    const style = sideStyle(state.side);
    const hp = numberOr(unit.hp, 0);
    const hpMax = Math.max(1, numberOr(unit.hp_max, 1));
    const hpRatio = clamp(hp / hpMax, 0, 1);
    const platform = platformForUnit(unit);

    dom.unitKindBadge.textContent = unitKindCode(unit);
    dom.unitName.textContent = unit.name || `Unit #${unit.id}`;
    dom.unitName.title = unit.name || '';
    dom.unitClass.textContent = platform && platform.name ? platform.name : titleCaseWords(unit.class || unit.kind || 'Unit');
    dom.unitClass.title = unit.class || '';
    dom.unitId.textContent = `#${unit.id}`;
    dom.unitSideBadge.textContent = state.side.toUpperCase();
    dom.unitSideBadge.style.color = style.hex;

    dom.unitHpBar.style.width = `${hpRatio * 100}%`;
    dom.unitHpBar.className = hpRatio <= 0.28 ? 'is-critical' : hpRatio <= 0.6 ? 'is-warn' : '';
    dom.unitHpText.textContent = `${fmtInt(hp)} / ${fmtInt(hpMax)}`;

    dom.unitSpeed.textContent = `${fmt1(unit.speed_kts)} KT`;
    dom.unitSpeedOrder.textContent = `ORD ${fmt1(unit.ordered_speed_kts)} KT`;
    dom.unitCourse.textContent = fmtCourse(unit.course_deg);
    dom.unitCourseOrder.textContent = `ORD ${fmtCourse(unit.ordered_course_deg)}`;

    dom.unitDepth.textContent = '';
    dom.unitDepthOrder.textContent = '';
    if (unit.kind === 'Sub' || String(unit.kind).toLowerCase() === 'submarine') {
      dom.unitDepthLabel.textContent = 'DEPTH';
      dom.unitDepth.textContent = `${fmtInt(unit.depth_m)} M`;
      dom.unitDepthOrder.textContent = 'SUBMERGED';
    } else if (unit.kind === 'Aircraft') {
      dom.unitDepthLabel.textContent = 'ALTITUDE';
      dom.unitDepth.textContent = `${fmtInt(unit.altitude_m)} M`;
      dom.unitDepthOrder.textContent = finiteOrNull(unit.endurance_s) !== null
        ? `ENDUR ${fmt1(unit.endurance_s / 3600)} H`
        : 'AIRBORNE';
    } else if (unit.kind === 'Base') {
      dom.unitDepthLabel.textContent = 'STATUS';
      dom.unitDepth.textContent = 'ASHORE';
      dom.unitDepthOrder.textContent = 'FIXED SITE';
    } else {
      dom.unitDepthLabel.textContent = 'DEPTH';
      dom.unitDepth.textContent = 'SURFACE';
      dom.unitDepthOrder.textContent = `${numberOr(unit.chaff, 0)} CHAFF`;
    }

    const chips = [];
    chips.push(`<span class="chip chip--accent">${escapeHtml(state.side.toUpperCase())}</span>`);
    chips.push(`<span class="chip ${unit.fire_on ? 'chip--warn' : ''}">FIRE ${unit.fire_on ? 'FREE' : 'HOLD'}</span>`);
    if (unit.assigned !== null && unit.assigned !== undefined) {
      chips.push(`<span class="chip chip--warn">LOCK C${escapeHtml(unit.assigned)}</span>`);
    }
    if (unit.mission) {
      chips.push(`<span class="chip">MISSION ${escapeHtml(String(unit.mission).toUpperCase())}</span>`);
    }
    if (obs && obs.over) {
      chips.push('<span class="chip chip--danger">SCENARIO OVER</span>');
    }
    dom.unitChips.innerHTML = chips.join('');
  }

  function renderOrders(unit, over) {
    const disabled = over || !unit;
    dom.btnWeaponsFree.disabled = disabled;
    dom.btnWeaponsFree.classList.toggle('is-on', !!unit.fire_on);
    dom.btnWeaponsFree.setAttribute('aria-pressed', String(!!unit.fire_on));
    dom.weaponsFreeState.textContent = unit.fire_on ? 'ACTIVE' : 'HOLD';
    dom.weaponsStateChip.textContent = unit.fire_on ? 'WEAPONS FREE' : 'WEAPONS HOLD';
    dom.weaponsStateChip.classList.toggle('is-free', !!unit.fire_on);

    dom.chaffCount.textContent = String(numberOr(unit.chaff, 0));
    dom.btnChaff.disabled = disabled || numberOr(unit.chaff, 0) <= 0;
    dom.btnChaff.title = unit.chaff > 0 ? `Deploy one chaff bundle (${unit.chaff} remaining)` : 'No chaff remaining';

    dom.btnStop.disabled = disabled;
    dom.btnRecover.disabled = disabled || !unit.deck;
    dom.recoverState.textContent = unit.deck ? 'FLIGHT DECK' : 'NO FLIGHT DECK';

    if (unit.assigned !== null && unit.assigned !== undefined) {
      dom.assignedTarget.textContent = `DESIGNATED TARGET · C${unit.assigned}`;
      dom.assignedTarget.classList.add('has-target');
    } else {
      dom.assignedTarget.textContent = 'NO TARGET ASSIGNED';
      dom.assignedTarget.classList.remove('has-target');
    }
  }

  function renderWeapons(unit) {
    const weapons = Array.isArray(unit.weapons) ? unit.weapons : [];
    dom.weaponsCount.textContent = `${weapons.length} MOUNT${weapons.length === 1 ? '' : 'S'}`;
    if (!weapons.length) {
      dom.weaponsList.innerHTML = '<div class="deck-empty" style="padding:7px">NO WEAPON MOUNTS</div>';
      return;
    }
    dom.weaponsList.innerHTML = weapons.map((mount) => {
      const ammo = numberOr(mount.ammo, 0);
      const cooldown = numberOr(mount.cooldown_s, 0);
      return `
        <div class="weapon-row">
          <div class="weapon-name">
            <b title="${escapeHtml(mount.weapon || '')}">${escapeHtml(mount.name || weaponName(mount.weapon))}</b>
            <small>${escapeHtml(mount.weapon || '')}</small>
          </div>
          <div class="weapon-stat">
            <span>AMMO</span>
            <b class="${ammo <= 0 ? 'is-empty' : ''}">${ammo}</b>
          </div>
          <div class="weapon-stat">
            <span>CD</span>
            <b class="${cooldown > 0 ? 'is-cooling' : ''}">${cooldown > 0 ? `${fmt1(cooldown)}s` : 'RDY'}</b>
          </div>
        </div>`;
    }).join('');
  }

  function renderSensors(unit) {
    const sensors = Array.isArray(unit.sensors) ? unit.sensors : [];
    dom.sensorsCount.textContent = `${sensors.length} SET${sensors.length === 1 ? '' : 'S'}`;
    if (!sensors.length) {
      dom.sensorsList.innerHTML = '<div class="deck-empty" style="padding:7px">NO SENSOR SETS</div>';
      return;
    }
    dom.sensorsList.innerHTML = sensors.map((sensor) => {
      const definition = state.sensorById.get(String(sensor.sensor || ''));
      const kind = definition ? String(definition.kind || '') : '';
      let stateLabel = 'ONLINE';
      let online = true;
      if (sensor.active) {
        stateLabel = 'ACTIVE';
      } else if (kind === 'SonarActive') {
        stateLabel = 'STANDBY';
        online = false;
      } else if (kind === 'SonarPassive') {
        stateLabel = 'LISTENING';
      }
      return `
        <div class="sensor-row">
          <div class="sensor-id">
            <i class="sensor-dot ${online ? '' : 'is-off'}" aria-hidden="true"></i>
            <b title="${escapeHtml(sensor.sensor || '')}">${escapeHtml(sensor.name || sensorName(sensor.sensor))}</b>
          </div>
          <span class="sensor-state ${online ? 'is-active' : ''}">${stateLabel}</span>
        </div>`;
    }).join('');
  }

  function renderMission(unit) {
    const mission = unit.mission ? String(unit.mission).toUpperCase() : 'NO TASKING';
    dom.missionValue.textContent = mission;
    dom.missionDetail.textContent = unit.mission
      ? 'Air tasking assigned by the flight deck.'
      : 'No air mission is currently assigned.';
  }

  function renderDeck(unit, obs) {
    if (!unit.deck) {
      dom.deckList.innerHTML = '<span class="deck-empty">NO FLIGHT DECK</span>';
      return;
    }

    const lines = [];
    const ready = Array.isArray(unit.deck.ready) ? unit.deck.ready : [];
    const cycling = Array.isArray(unit.deck.cycling) ? unit.deck.cycling : [];
    for (const entry of ready) {
      if (!Array.isArray(entry)) continue;
      lines.push(`<div class="deck-line"><span>${escapeHtml(aircraftName(entry[0]))}</span><b>×${escapeHtml(entry[1])} READY</b></div>`);
    }
    for (const entry of cycling) {
      if (!Array.isArray(entry)) continue;
      const readyAt = numberOr(entry[2], 0);
      const tick = obs ? numberOr(obs.tick, 0) : 0;
      const remaining = Math.max(0, Math.ceil(readyAt - tick));
      lines.push(`<div class="deck-line"><span>${escapeHtml(aircraftName(entry[0]))}</span><b>×${escapeHtml(entry[1])} · ${remaining}s</b></div>`);
    }
    dom.deckList.innerHTML = lines.length
      ? lines.join('')
      : '<span class="deck-empty">DECK CLEAR · NO READY AIRCRAFT</span>';
  }

  function renderSystems(unit) {
    const systems = isObject(unit.systems) ? unit.systems : {};
    const labels = {
      mobility: 'MOBILITY',
      radar: 'RADAR',
      sonar: 'SONAR',
      fire_control: 'FIRE CTRL',
    };
    const keys = ['mobility', 'radar', 'sonar', 'fire_control'];
    dom.systemsList.innerHTML = keys.map((key) => {
      const ratio = clamp(numberOr(systems[key], 1), 0, 1);
      const pct = Math.round(ratio * 100);
      const color = ratio <= 0.1 ? 'var(--danger)' : ratio < 0.75 ? 'var(--amber)' : 'var(--green)';
      const valueClass = ratio <= 0.1 ? 'is-down' : ratio < 0.75 ? 'is-degraded' : '';
      const value = ratio <= 0 ? 'DOWN' : `${pct}%`;
      return `
        <div class="system-item">
          <div class="system-label">
            <span>${labels[key]}</span>
            <span class="bar"><i style="width:${pct}%;background:${color}"></i></span>
          </div>
          <b class="system-value ${valueClass}">${value}</b>
        </div>`;
    }).join('');
  }

  function renderTargetPanel(unit, contact, obs) {
    const over = !!(obs && obs.over);

    if (!contact) {
      dom.targetEmpty.hidden = false;
      dom.targetContent.hidden = true;
      dom.targetQualityChip.textContent = 'NO TRACK';
      state.fireOptionsKey = null;
      populateFirePlaceholder(unit ? 'Select a contact' : 'Select a unit');
      dom.btnAssign.disabled = true;
      dom.btnClearTarget.disabled = !unit || unit.assigned === null || unit.assigned === undefined || over;
      dom.btnFire.disabled = true;
      dom.fireNote.textContent = unit
        ? 'Left-click a contact to build a firing solution.'
        : 'Select a friendly unit, then a contact.';
      dom.fireNote.classList.remove('is-warn');
      return;
    }

    dom.targetEmpty.hidden = true;
    dom.targetContent.hidden = false;

    const cat = categoryInfo(contact.category);
    const quality = clamp(numberOr(contact.quality, 0), 0, 1);
    const platform = platformForContact(contact);
    const displayName = contact.class_name
      || (platform && platform.name)
      || cat.label;
    const bearingOnly = !!contact.bearing_only;
    const stale = !!contact.stale;
    const range = unit ? rangeNm(unit.pos, contact.estimate) : null;
    const qualityPct = Math.round(quality * 100);

    dom.targetQualityChip.textContent = stale ? 'STALE' : `${qualityPct}% Q`;
    dom.targetQualityChip.style.color = stale ? 'var(--muted)' : '';
    dom.targetSymbol.textContent = cat.short;
    dom.targetSymbol.title = cat.label;
    dom.targetName.textContent = String(displayName).toUpperCase();
    dom.targetName.title = displayName;
    dom.targetSub.textContent = `C${contact.id} · ${cat.label}${bearingOnly ? ' · BEARING ONLY' : ''}${stale ? ' · STALE' : ''}`;
    dom.targetLockTag.textContent = stale ? 'STALE' : bearingOnly ? 'BEARING' : 'TRACK';
    dom.targetLockTag.classList.toggle('is-stale', stale);
    dom.targetQualityBar.style.width = `${qualityPct}%`;
    dom.targetQualityText.textContent = `${qualityPct}%`;
    dom.targetRange.textContent = range === null ? '—' : `${fmtRange(range)} NM`;
    dom.targetCourse.textContent = fmtCourse(contact.est_course_deg);
    dom.targetSpeed.textContent = `${fmt1(contact.est_speed_kts)} KT`;
    dom.targetLastseen.textContent = fmtAge(contact.last_seen_s);

    const tags = [];
    tags.push(`<span class="chip chip--accent">${escapeHtml(cat.label.toUpperCase())}</span>`);
    if (contact.class_name) tags.push('<span class="chip">IDENTIFIED</span>');
    else tags.push('<span class="chip">UNIDENTIFIED</span>');
    if (stale) tags.push('<span class="chip chip--warn">STALE</span>');
    if (bearingOnly) tags.push('<span class="chip chip--warn">BEARING ONLY</span>');
    const sensors = Array.isArray(contact.sensors) ? contact.sensors.slice(0, 3) : [];
    for (const sensor of sensors) {
      tags.push(`<span class="chip">${escapeHtml(String(sensor).replace(/_/g, ' ').toUpperCase())}</span>`);
    }
    dom.targetTags.innerHTML = tags.join('');

    const assigned = unit && unit.assigned !== null && unit.assigned !== undefined
      && Number(unit.assigned) === Number(contact.id);
    dom.btnAssign.disabled = !unit || over;
    dom.btnAssign.textContent = assigned ? 'TARGET ASSIGNED' : 'ASSIGN TARGET';
    dom.btnAssign.classList.toggle('is-on', !!assigned);
    dom.btnClearTarget.disabled = !unit || unit.assigned === null || unit.assigned === undefined || over;

    refreshFireControls(unit, contact, obs);
  }

  function populateFirePlaceholder(label) {
    dom.fireWeapon.replaceChildren();
    const option = document.createElement('option');
    option.textContent = label;
    option.value = '';
    dom.fireWeapon.appendChild(option);
    dom.fireWeapon.disabled = true;
    state.fireWeaponId = null;
  }

  function refreshFireControls(unit, contact, obs) {
    const over = !!(obs && obs.over);
    if (!unit || !contact) {
      populateFirePlaceholder(!unit ? 'Select a unit' : 'Select a contact');
      dom.btnFire.disabled = true;
      dom.fireNote.textContent = unit
        ? 'Left-click a contact to build a firing solution.'
        : 'Select a friendly unit before assigning or firing.';
      dom.fireNote.classList.remove('is-warn');
      return;
    }

    const cat = categoryInfo(contact.category);
    const weapons = Array.isArray(unit.weapons) ? unit.weapons : [];
    const signature = `${unit.id}|${contact.id}|${weapons.map((mount) => {
      const fit = weaponCanEngage(mount.weapon, cat) ? 1 : 0;
      return `${mount.weapon}:${numberOr(mount.ammo, 0)}:${fit}`;
    }).join(',')}`;

    if (signature !== state.fireOptionsKey) {
      state.fireOptionsKey = signature;
      dom.fireWeapon.replaceChildren();
      let firstUsable = null;

      for (const mount of weapons) {
        const def = weaponDefinition(mount.weapon);
        const ammo = numberOr(mount.ammo, 0);
        const fits = weaponCanEngage(mount.weapon, cat);
        const option = document.createElement('option');
        option.value = String(mount.weapon || '');
        const pieces = [mount.name || weaponName(mount.weapon)];
        if (ammo > 0) pieces.push(`${ammo} RD`);
        else pieces.push('EMPTY');
        if (def && Number.isFinite(Number(def.range_nm))) pieces.push(`${fmtRange(def.range_nm)} NM`);
        if (!fits) pieces.push('NO FIT');
        option.textContent = pieces.join(' · ');
        option.disabled = ammo <= 0 || !fits;
        dom.fireWeapon.appendChild(option);
        if (!option.disabled && !firstUsable) firstUsable = String(mount.weapon || '');
      }

      if (!weapons.length) {
        const option = document.createElement('option');
        option.textContent = 'NO WEAPONS';
        option.value = '';
        dom.fireWeapon.appendChild(option);
      } else if (!firstUsable) {
        const option = document.createElement('option');
        option.textContent = 'NO VALID MOUNT';
        option.value = '';
        dom.fireWeapon.appendChild(option);
      }

      const previous = state.fireWeaponId;
      const previousOption = previous
        ? Array.from(dom.fireWeapon.options).find((option) => option.value === previous && !option.disabled)
        : null;
      const selected = previousOption ? previousOption.value : (firstUsable || '');
      dom.fireWeapon.value = selected;
      state.fireWeaponId = selected || null;
    } else if (state.fireWeaponId) {
      dom.fireWeapon.value = state.fireWeaponId;
    }

    const selectedWeapon = dom.fireWeapon.value;
    const mount = weapons.find((weapon) => String(weapon.weapon) === selectedWeapon) || null;
    const def = mount ? weaponDefinition(mount.weapon) : null;
    const fits = mount ? weaponCanEngage(mount.weapon, cat) : false;
    const range = rangeNm(unit.pos, contact.estimate);
    dom.fireWeapon.disabled = !mount || over;
    dom.btnFire.disabled = !mount || over || numberOr(mount.ammo, 0) <= 0 || !fits;

    const notes = [];
    let warn = false;

    if (over) {
      notes.push('Scenario is over. Weapons are locked.');
      warn = true;
    } else if (!mount) {
      notes.push('No weapon is compatible with this contact category.');
      warn = true;
    } else {
      if (!fits) {
        notes.push(`${mount.name || weaponName(mount.weapon)} cannot engage ${cat.label.toLowerCase()} contacts.`);
        warn = true;
      }
      if (numberOr(mount.ammo, 0) <= 0) {
        notes.push('Selected mount is empty.');
        warn = true;
      }
      if (numberOr(mount.cooldown_s, 0) > 0) {
        notes.push(`Mount reloading (${fmt1(mount.cooldown_s)}s) — order may be refused.`);
        warn = true;
      }
      if (def && Number.isFinite(Number(def.range_nm)) && range > Number(def.range_nm)) {
        notes.push(`Est. range ${fmtRange(range)} NM is outside nominal ${fmtRange(def.range_nm)} NM.`);
        warn = true;
      }
      if (def && Number.isFinite(Number(def.min_range_nm)) && range < Number(def.min_range_nm)) {
        notes.push(`Est. range ${fmtRange(range)} NM is inside minimum ${fmtRange(def.min_range_nm)} NM.`);
        warn = true;
      }
      if (!notes.length) {
        notes.push(`Firing solution ready · est. range ${fmtRange(range)} NM.`);
      }
      if (state.speed === 0) notes.push('Simulation paused; the order will wait for the next tick.');
    }

    dom.fireNote.textContent = notes.join(' ');
    dom.fireNote.classList.toggle('is-warn', warn);
    dom.btnAssign.disabled = !unit || over;
  }

  function weaponCanEngage(weaponId, cat) {
    if (!cat || !cat.target) return false;
    const def = weaponDefinition(weaponId);
    if (!def || !isObject(def.targets)) return true;
    return !!def.targets[cat.target];
  }

  // ------------------------------------------------------------------
  // Commands
  // ------------------------------------------------------------------
  async function sendCommandRaw(command, side = state.side) {
    try {
      const result = await postJson('/api/command', { side, command });
      return { ok: true, data: result, status: 200, message: '' };
    } catch (err) {
      return {
        ok: false,
        status: err instanceof ApiError ? err.status : 0,
        message: errorText(err),
      };
    }
  }

  async function postCommand(command, label, options = {}) {
    const { quiet = false } = options;
    const result = await sendCommandRaw(command);
    if (result.ok) {
      if (!quiet) {
        const paused = state.speed === 0 ? ' · SIMULATION PAUSED' : '';
        showToast(`${label || 'ORDER'} QUEUED${paused}`, state.speed === 0 ? 'warn' : 'ok');
      }
    } else if (!quiet) {
      showToast(`Order rejected: ${result.message}`, 'error');
    }
    return result.ok;
  }

  async function sendGoto(unitId, route, side, generation) {
    if (side !== state.side || generation !== state.gameGeneration) return false;
    // The reference API documents waypoints as [x, y] pairs. The in-tree
    // server deserializes Vec2 as {x, y}; try the documented shape first and
    // transparently fall back so this client works against either form.
    let result = await sendCommandRaw({
      cmd: 'goto',
      unit: unitId,
      waypoints: route.map((point) => [point.x, point.y]),
    }, side);

    if (!result.ok && (result.status === 400 || result.status === 422)) {
      result = await sendCommandRaw({
        cmd: 'goto',
        unit: unitId,
        waypoints: route.map((point) => ({ x: point.x, y: point.y })),
      }, side);
    }
    return result.ok;
  }

  function appendWaypoint(worldPoint) {
    const unit = selectedUnit();
    const obs = state.observation;
    if (!obs) {
      showToast('No tactical feed is available yet.', 'warn');
      return;
    }
    if (obs.over) {
      showToast('The scenario is over; movement orders are locked.', 'warn');
      return;
    }
    if (!unit) {
      showToast('Select a friendly unit before plotting a course.', 'warn');
      return;
    }

    const key = routeKey(unit.id);
    const route = (state.routes.get(key) || []).slice();
    const point = {
      x: Math.round(worldPoint.x * 10) / 10,
      y: Math.round(worldPoint.y * 10) / 10,
    };
    const previousPoint = route.length ? route[route.length - 1] : unit.pos;
    if (rangeNm(previousPoint, point) < 0.35) return;

    if (route.length >= MAX_WAYPOINTS) {
      showToast(`Waypoint limit reached (${MAX_WAYPOINTS}). Use ALL STOP to clear the route.`, 'warn');
      return;
    }

    route.push(point);
    state.routes.set(key, route);

    const side = state.side;
    const generation = state.gameGeneration;
    const unitId = Number(unit.id);
    const previousTask = state.gotoQueues.get(key) || Promise.resolve();
    const task = previousTask
      .catch(() => {})
      .then(async () => {
        if (side !== state.side || generation !== state.gameGeneration) return;
        const ok = await sendGoto(unitId, route, side, generation);
        if (side !== state.side || generation !== state.gameGeneration) return;
        if (ok) {
          const paused = state.speed === 0 ? ' · SIMULATION PAUSED' : '';
          showToast(`WAYPOINT ${route.length} QUEUED · X ${fmtRange(point.x)} / Y ${fmtRange(point.y)} NM${paused}`, state.speed === 0 ? 'warn' : 'ok');
        } else {
          if (state.routes.get(key) === route) state.routes.delete(key);
          showToast('Waypoint order rejected by the referee.', 'error');
        }
      })
      .catch(() => {});

    state.gotoQueues.set(key, task);
    task.finally(() => {
      if (state.gotoQueues.get(key) === task) state.gotoQueues.delete(key);
    });
  }

  function routeKey(unitId) {
    return `${state.side}:${unitId}`;
  }

  function pruneRoutes(obs) {
    const units = Array.isArray(obs.units) ? obs.units : [];
    for (const [key, route] of Array.from(state.routes.entries())) {
      const [, idText] = key.split(':');
      const unit = units.find((candidate) => Number(candidate.id) === Number(idText));
      if (!unit) {
        state.routes.delete(key);
        continue;
      }
      while (route.length && rangeNm(unit.pos, route[0]) < 0.7) route.shift();
      if (!route.length) state.routes.delete(key);
    }
  }

  async function issueWeaponsFree() {
    const unit = selectedUnit();
    if (!unit || (state.observation && state.observation.over)) return;
    dom.btnWeaponsFree.disabled = true;
    try {
      await postCommand({ cmd: 'set_weapons_free', unit: Number(unit.id), free: !unit.fire_on }, 'WEAPONS STATE');
    } finally {
      dom.btnWeaponsFree.disabled = false;
      renderOrders(unit, !!(state.observation && state.observation.over));
    }
  }

  async function issueDeployChaff() {
    const unit = selectedUnit();
    if (!unit || (state.observation && state.observation.over)) return;
    dom.btnChaff.disabled = true;
    try {
      await postCommand({ cmd: 'deploy_chaff', unit: Number(unit.id) }, 'CHAFF');
    } finally {
      dom.btnChaff.disabled = false;
      renderOrders(unit, !!(state.observation && state.observation.over));
    }
  }

  async function issueStop() {
    const unit = selectedUnit();
    if (!unit || (state.observation && state.observation.over)) return;
    dom.btnStop.disabled = true;
    try {
      const key = routeKey(unit.id);
      const pending = state.gotoQueues.get(key);
      if (pending) {
        try {
          await pending;
        } catch {
          /* the pending goto already reports its own failure */
        }
      }
      const ok = await postCommand({ cmd: 'stop', unit: Number(unit.id) }, 'ALL STOP');
      if (ok) state.routes.delete(key);
    } finally {
      dom.btnStop.disabled = false;
      renderOrders(unit, !!(state.observation && state.observation.over));
    }
  }

  async function issueRecoverAircraft() {
    const unit = selectedUnit();
    if (!unit || !unit.deck || (state.observation && state.observation.over)) return;
    dom.btnRecover.disabled = true;
    try {
      await postCommand({ cmd: 'recover_aircraft', unit: Number(unit.id) }, 'RECOVER AIRCRAFT');
    } finally {
      dom.btnRecover.disabled = false;
      renderOrders(unit, !!(state.observation && state.observation.over));
    }
  }

  async function issueAssignTarget() {
    const unit = selectedUnit();
    const contact = selectedContact();
    if (!unit || !contact || (state.observation && state.observation.over)) return;
    dom.btnAssign.disabled = true;
    try {
      await postCommand({
        cmd: 'assign_target',
        unit: Number(unit.id),
        contact: Number(contact.id),
      }, 'TARGET DESIGNATION');
    } finally {
      dom.btnAssign.disabled = false;
      renderTargetPanel(unit, contact, state.observation);
    }
  }

  async function issueClearTarget() {
    const unit = selectedUnit();
    if (!unit || (state.observation && state.observation.over)) return;
    dom.btnClearTarget.disabled = true;
    try {
      await postCommand({ cmd: 'clear_target', unit: Number(unit.id) }, 'CLEAR TARGET');
    } finally {
      dom.btnClearTarget.disabled = false;
      const contact = selectedContact();
      renderTargetPanel(unit, contact, state.observation);
    }
  }

  async function issueFire() {
    const unit = selectedUnit();
    const contact = selectedContact();
    if (!unit || !contact || (state.observation && state.observation.over)) return;
    const weapon = dom.fireWeapon.value || state.fireWeaponId;
    if (!weapon) return;
    dom.btnFire.disabled = true;
    try {
      await postCommand({
        cmd: 'fire',
        unit: Number(unit.id),
        contact: Number(contact.id),
        weapon,
        rounds: 1,
      }, `FIRE ${weaponName(weapon).toUpperCase()}`);
    } finally {
      dom.btnFire.disabled = false;
      renderTargetPanel(unit, contact, state.observation);
    }
  }

  // ------------------------------------------------------------------
  // Event log
  // ------------------------------------------------------------------
  function isEventVisible(event) {
    if (!event) return false;
    const vis = event.vis;
    if (vis === 'All') return true;
    if (isObject(vis) && (vis.Side || vis.side)) {
      return String(vis.Side || vis.side) === state.side;
    }
    if (event.side) return String(event.side) === state.side;
    return true;
  }

  function eventVisibilityLabel(event) {
    if (event.vis === 'All') return 'ALL';
    if (isObject(event.vis) && (event.vis.Side || event.vis.side)) {
      return String(event.vis.Side || event.vis.side).toUpperCase();
    }
    return event.side ? String(event.side).toUpperCase() : 'ALL';
  }

  function contactRef(id) {
    return id === null || id === undefined ? '—' : `C${id}`;
  }

  function unitRef(id) {
    return id === null || id === undefined ? '—' : `#${id}`;
  }

  function eventPosition(event) {
    const hint = isObject(event.pos_hint) ? event.pos_hint : null;
    if (!hint) return '';
    return ` · X ${fmtRange(hint.x)} / Y ${fmtRange(hint.y)} NM`;
  }

  function formatEvent(event) {
    const kind = String(event.event || 'event');
    const contact = contactRef(event.contact ?? event.target_contact);
    const unit = unitRef(event.unit);
    const shooter = event.shooter_name || unitRef(event.shooter);
    const unitName = event.unit_name || event.base_name || null;
    const weapon = event.weapon ? weaponName(event.weapon) : 'Weapon';
    const side = event.side ? String(event.side).toUpperCase() : '';

    switch (kind) {
      case 'scenario_started':
        return {
          headline: `SCENARIO STARTED · ${event.name || 'UNNAMED'}`,
          detail: 'Referee clock is running. Double-blind contact picture initialised.',
          tone: 'scenario',
        };
      case 'detection':
        return {
          headline: `DETECTION · ${contact} ${String(event.category || 'CONTACT').toUpperCase()}`,
          detail: `${sensorName(event.sensor || 'sensor')}${eventPosition(event)}`,
          tone: 'intel',
        };
      case 'contact_identified':
        return {
          headline: `CONTACT IDENTIFIED · ${contact}`,
          detail: `Classified as ${event.class_name || 'unknown class'}${side ? ` · ${side}` : ''}`,
          tone: 'intel',
        };
      case 'contact_lost':
        return { headline: `CONTACT LOST · ${contact}`, detail: 'Track decayed past the stale threshold.', tone: 'intel' };
      case 'contact_sank':
        return { headline: `CONTACT SUNK · ${contact}`, detail: 'Target removed from the contact picture.', tone: 'loss' };
      case 'counter_detected':
        return {
          headline: `COUNTER-DETECTED · ${unit}`,
          detail: `Emissions exposed this unit to ${event.by_side || 'the enemy'}.`,
          tone: 'intel',
        };
      case 'weapons_release':
        return {
          headline: `WEAPONS RELEASE · ${shooter}`,
          detail: `${weapon} × ${numberOr(event.count, 1)}${side ? ` · ${side}` : ''}`,
          tone: 'weapon',
        };
      case 'incoming':
        return {
          headline: `INCOMING · ${contact}`,
          detail: `Impact estimate in ${fmtEta(event.eta_s)}.`,
          tone: 'danger',
        };
      case 'sam_shot':
        return {
          headline: `SAM LAUNCH · ${shooter}`,
          detail: `${weapon} → ${contactRef(event.target_contact)}`,
          tone: 'weapon',
        };
      case 'intercept':
        return {
          headline: `INTERCEPT ${event.intercepted ? 'SUCCESS' : 'FAILED'}`,
          detail: event.contact !== null && event.contact !== undefined
            ? `Engagement against ${contact}`
            : 'Terminal engagement',
          tone: event.intercepted ? 'status' : 'weapon',
        };
      case 'chaff_deployed':
        return { headline: `CHAFF DEPLOYED · ${unit}`, detail: 'Localised obscuration active.', tone: 'status' };
      case 'decoy_fooled':
        return {
          headline: 'DECOY FOOLED SEEKER',
          detail: event.contact !== null && event.contact !== undefined ? `Track ${contact} left the threat axis.` : 'Incoming weapon seduced by countermeasures.',
          tone: 'status',
        };
      case 'gun_action':
        return {
          headline: `GUN ACTION · ${shooter}`,
          detail: `${numberOr(event.hits, 0)} hit${numberOr(event.hits, 0) === 1 ? '' : 's'} on ${contact}`,
          tone: 'weapon',
        };
      case 'torpedo_detected':
        return { headline: `TORPEDO DETECTED · ${contact}`, detail: 'Torpedo wake acquired by own sensors.', tone: 'danger' };
      case 'impact':
        return {
          headline: `IMPACT · ${unitName || unit}`,
          detail: `${weapon} · ${fmt1(event.damage)} damage · ${fmt1(event.hp_left)} HP remaining`,
          tone: 'danger',
        };
      case 'critical_hit':
        return {
          headline: `CRITICAL HIT · ${unitName || unit}`,
          detail: `${titleCaseWords(event.system || 'system')} degraded or offline.`,
          tone: 'danger',
        };
      case 'sunk':
        return { headline: `UNIT LOST · ${unitName || unit}`, detail: 'Hull destroyed. Unit removed from the picture.', tone: 'loss' };
      case 'ditched':
        return { headline: `AIRCRAFT DITCHED · ${unitName || unit}`, detail: 'Airframe lost after fuel or damage failure.', tone: 'loss' };
      case 'air_shot_down':
        return {
          headline: `AIRCRAFT DOWN · ${unitName || unit}`,
          detail: `Shot down by ${event.by_side || 'hostile air defences'}.`,
          tone: 'loss',
        };
      case 'air_launch':
        return {
          headline: `AIR LAUNCH · ${numberOr(event.count, 1)} × ${event.class || 'aircraft'}`,
          detail: `Carrier ${unitRef(event.carrier)} · mission ${String(event.mission || 'unknown').toUpperCase()}`,
          tone: 'status',
        };
      case 'air_recovered':
        return {
          headline: `AIR RECOVERED · ${numberOr(event.count, 1)} × ${event.class || 'aircraft'}`,
          detail: `Recovered aboard ${unitRef(event.carrier)}.`,
          tone: 'status',
        };
      case 'deck_busy':
        return { headline: `DECK BUSY · ${unitRef(event.carrier)}`, detail: 'Flight operations already in progress.', tone: 'intel' };
      case 'base_damaged':
        return {
          headline: `BASE DAMAGED · ${event.base_name || unitRef(event.base)}`,
          detail: `${fmt1(event.damage)} damage · ${fmt1(event.hp_left)} HP remaining.`,
          tone: 'danger',
        };
      case 'base_destroyed':
        return { headline: `BASE DESTROYED · ${event.base_name || unitRef(event.base)}`, detail: 'Facility removed from the order of battle.', tone: 'loss' };
      case 'scenario_ended':
        return {
          headline: `SCENARIO ENDED · ${event.winner ? `${String(event.winner).toUpperCase()} VICTORY` : 'DRAW'}`,
          detail: `${event.reason || 'Scenario complete'} · VP B${fmtRange(event.vp_blue)} / R${fmtRange(event.vp_red)}`,
          tone: 'scenario',
        };
      default:
        return {
          headline: kind.replace(/_/g, ' ').toUpperCase(),
          detail: [event.side ? `SIDE ${event.side}` : '', event.contact !== undefined ? `CONTACT ${contact}` : '', event.unit !== undefined ? `UNIT ${unit}` : '']
            .filter(Boolean)
            .join(' · '),
          tone: 'status',
        };
    }
  }

  function buildEventNode(event) {
    const info = formatEvent(event);
    const node = document.createElement('article');
    node.className = `event-row event-row--${info.tone || 'status'}`;
    node.dataset.seq = String(event.seq);
    node.innerHTML = `
      <span class="event-time">${escapeHtml(fmtEventTick(event.tick))}</span>
      <i class="event-marker" aria-hidden="true"></i>
      <div class="event-copy">
        <div class="event-headline">${escapeHtml(info.headline)}</div>
        <div class="event-detail">${escapeHtml(info.detail)}</div>
      </div>
      <span class="event-vis">${escapeHtml(eventVisibilityLabel(event))}</span>`;
    return node;
  }

  function mergeEvents(events) {
    const added = [];
    for (const event of events) {
      if (!isObject(event)) continue;
      if (event.seq === null || event.seq === undefined) continue;
      const seq = Number(event.seq);
      if (!Number.isFinite(seq)) continue;
      if (!isEventVisible(event)) continue;
      if (state.eventMap.has(seq)) continue;
      state.eventMap.set(seq, event);
      added.push(event);
    }

    if (added.length) {
      added.sort((a, b) => Number(b.seq) - Number(a.seq));
      const wasAtTop = dom.eventLog.scrollTop <= 10;
      const previousHeight = dom.eventLog.scrollHeight;
      const previousTop = dom.eventLog.scrollTop;

      const fragment = document.createDocumentFragment();
      for (const event of added) fragment.appendChild(buildEventNode(event));
      dom.eventList.prepend(fragment);
      dom.logEmpty.hidden = true;

      if (wasAtTop) dom.eventLog.scrollTop = 0;
      else dom.eventLog.scrollTop = previousTop + (dom.eventLog.scrollHeight - previousHeight);

      trimEvents();
    }
    updateLogMeta();
  }

  function trimEvents() {
    if (state.eventMap.size <= MAX_EVENTS) return;
    const seqs = Array.from(state.eventMap.keys()).sort((a, b) => a - b);
    const removeCount = state.eventMap.size - MAX_EVENTS;
    for (let i = 0; i < removeCount; i += 1) {
      const seq = seqs[i];
      state.eventMap.delete(seq);
      const node = dom.eventList.querySelector(`[data-seq="${seq}"]`);
      if (node) node.remove();
    }
  }

  function updateLogMeta() {
    const count = state.eventMap.size;
    dom.logMeta.textContent = `NEWEST FIRST · ${count} EVENT${count === 1 ? '' : 'S'}`;
  }

  function clearEventLog() {
    state.eventMap.clear();
    dom.eventList.replaceChildren();
    dom.eventLog.scrollTop = 0;
    dom.logEmpty.hidden = false;
    updateLogMeta();
  }

  // ------------------------------------------------------------------
  // Map: projection, camera, interaction
  // ------------------------------------------------------------------
  function resizeMapCanvas() {
    if (!dom.mapCanvas) return;
    const rect = dom.mapCanvas.getBoundingClientRect();
    const dpr = clamp(window.devicePixelRatio || 1, 1, 3);
    const width = Math.max(1, Math.round(rect.width));
    const height = Math.max(1, Math.round(rect.height));
    const pixelWidth = Math.max(1, Math.round(width * dpr));
    const pixelHeight = Math.max(1, Math.round(height * dpr));

    if (dom.mapCanvas.width !== pixelWidth || dom.mapCanvas.height !== pixelHeight) {
      dom.mapCanvas.width = pixelWidth;
      dom.mapCanvas.height = pixelHeight;
    }
    map.dpr = dpr;
    map.w = width;
    map.h = height;

    if (map.w > 30 && map.h > 30) {
      if (!map.viewInitialized) {
        map.view.scale = clamp(Math.min(map.w, map.h) / 100, 2, 12);
        map.view.cx = 24;
        map.view.cy = 24;
        map.viewInitialized = true;
      }
      if (map.fitPending && state.observation) {
        fitMap();
        map.fitPending = false;
      }
    }
    map.lastScaleKey = '';
  }

  function startRenderLoop() {
    if (map.ctx) return;
    map.ctx = dom.mapCanvas.getContext('2d');
    if (!map.ctx) return;
    requestAnimationFrame(renderFrame);
  }

  function renderFrame(now) {
    const dt = map.lastFrame ? clamp(now - map.lastFrame, 0, 100) : 16;
    map.lastFrame = now;
    if (map.w > 0 && map.h > 0) drawMap(now, dt);
    requestAnimationFrame(renderFrame);
  }

  function worldToScreen(x, y) {
    return {
      x: map.w / 2 + (numberOr(x) - map.view.cx) * map.view.scale,
      y: map.h / 2 - (numberOr(y) - map.view.cy) * map.view.scale,
    };
  }

  function screenToWorld(x, y) {
    return {
      x: map.view.cx + (x - map.w / 2) / map.view.scale,
      y: map.view.cy - (y - map.h / 2) / map.view.scale,
    };
  }

  function fitMap() {
    if (map.w < 30 || map.h < 30) {
      map.fitPending = true;
      return;
    }
    const obs = state.observation;
    const points = [];
    if (obs) {
      for (const unit of Array.isArray(obs.units) ? obs.units : []) {
        if (unit && unit.pos) points.push(unit.pos);
      }
      for (const contact of Array.isArray(obs.contacts) ? obs.contacts : []) {
        if (contact && contact.estimate) points.push(contact.estimate);
      }
    }

    if (!points.length) {
      map.view.cx = 24;
      map.view.cy = 24;
      map.view.scale = clamp(Math.min(map.w, map.h) / 100, 2, 12);
      map.lastScaleKey = '';
      return;
    }

    let minX = Infinity;
    let maxX = -Infinity;
    let minY = Infinity;
    let maxY = -Infinity;
    for (const point of points) {
      const x = numberOr(point.x);
      const y = numberOr(point.y);
      minX = Math.min(minX, x);
      maxX = Math.max(maxX, x);
      minY = Math.min(minY, y);
      maxY = Math.max(maxY, y);
    }

    let spanX = Math.max(24, maxX - minX);
    let spanY = Math.max(24, maxY - minY);
    const minSpan = 52;
    if (spanX < minSpan) {
      const extra = (minSpan - spanX) / 2;
      minX -= extra;
      maxX += extra;
      spanX = minSpan;
    }
    if (spanY < minSpan) {
      const extra = (minSpan - spanY) / 2;
      minY -= extra;
      maxY += extra;
      spanY = minSpan;
    }

    const availableW = Math.max(120, map.w - 190);
    const availableH = Math.max(120, map.h - 150);
    const scale = clamp(Math.min(availableW / spanX, availableH / spanY), 0.3, 28);
    map.view.cx = (minX + maxX) / 2;
    map.view.cy = (minY + maxY) / 2;
    map.view.scale = scale;
    map.viewInitialized = true;
    map.lastScaleKey = '';
  }

  function maybeAutoFit() {
    if (map.w < 30 || map.h < 30) return;
    if (map.fitPending) {
      fitMap();
      map.fitPending = false;
      syncSeenContacts();
      return;
    }
    if (!map.userAdjusted && state.observation) {
      const contacts = Array.isArray(state.observation.contacts) ? state.observation.contacts : [];
      const unseen = contacts.filter((contact) => (
        !map.seenContacts.has(Number(contact.id))
        && !contact.stale
        && numberOr(contact.quality, 0) >= 0.2
      ));
      if (unseen.some((contact) => !pointComfortablyVisible(contact.estimate))) {
        fitMap();
      }
    }
    syncSeenContacts();
  }

  function syncSeenContacts() {
    const contacts = state.observation && Array.isArray(state.observation.contacts)
      ? state.observation.contacts
      : [];
    map.seenContacts = new Set(contacts.map((contact) => Number(contact.id)));
  }

  function pointComfortablyVisible(point) {
    if (!point) return false;
    const screen = worldToScreen(point.x, point.y);
    return screen.x > 55 && screen.x < map.w - 55 && screen.y > 70 && screen.y < map.h - 65;
  }

  function zoomAt(screenX, screenY, factor) {
    const before = screenToWorld(screenX, screenY);
    map.view.scale = clamp(map.view.scale * factor, 0.3, 32);
    map.view.cx = before.x - (screenX - map.w / 2) / map.view.scale;
    map.view.cy = before.y + (screenY - map.h / 2) / map.view.scale;
    map.userAdjusted = true;
    map.lastScaleKey = '';
  }

  function zoomCenter(factor) {
    zoomAt(map.w / 2, map.h / 2, factor);
  }

  function localPoint(event) {
    const rect = dom.mapCanvas.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  }

  function screenPositionForUnit(unit) {
    const smoothed = map.smooth.get(`u:${unit.id}`);
    const pos = smoothed || unit.pos;
    return worldToScreen(pos.x, pos.y);
  }

  function screenPositionForContact(contact) {
    const smoothed = map.smooth.get(`c:${contact.id}`);
    const pos = smoothed || contact.estimate;
    return worldToScreen(pos.x, pos.y);
  }

  function pickAt(screenX, screenY) {
    const obs = state.observation;
    if (!obs) return null;
    let best = null;
    let bestDistance = Infinity;

    for (const unit of Array.isArray(obs.units) ? obs.units : []) {
      const screen = screenPositionForUnit(unit);
      const distance = Math.hypot(screen.x - screenX, screen.y - screenY);
      if (distance <= 18 && distance < bestDistance) {
        bestDistance = distance;
        best = { type: 'unit', id: Number(unit.id) };
      }
    }
    for (const contact of Array.isArray(obs.contacts) ? obs.contacts : []) {
      const screen = screenPositionForContact(contact);
      const radius = contactSymbolSize(contact) + 8;
      const distance = Math.hypot(screen.x - screenX, screen.y - screenY);
      if (distance <= radius && distance < bestDistance) {
        bestDistance = distance;
        best = { type: 'contact', id: Number(contact.id) };
      }
    }
    return best;
  }

  function updateHover(screenX, screenY) {
    map.cursorWorld = screenToWorld(screenX, screenY);
    map.hover = pickAt(screenX, screenY);
    map.pointerInside = true;
    dom.mapCanvas.classList.toggle('is-targeting', !!map.hover);
    updateCursorReadout();
  }

  function updateCursorReadout() {
    if (!map.pointerInside || !map.cursorWorld) {
      dom.cursorReadout.textContent = 'X — · Y — NM';
      return;
    }

    const unit = selectedUnit();
    if (map.hover && map.hover.type === 'contact') {
      const contact = findContactById(map.hover.id);
      if (contact) {
        const cat = categoryInfo(contact.category);
        const name = contact.class_name || cat.label;
        const range = unit ? rangeNm(unit.pos, contact.estimate) : null;
        dom.cursorReadout.textContent = `C${contact.id} ${name.toUpperCase()}${range === null ? '' : ` · ${fmtRange(range)} NM`}`;
        return;
      }
    }
    if (map.hover && map.hover.type === 'unit') {
      const unit = (state.observation && state.observation.units || []).find((candidate) => Number(candidate.id) === map.hover.id);
      if (unit) {
        dom.cursorReadout.textContent = `${unit.name || `UNIT #${unit.id}`}`;
        return;
      }
    }
    dom.cursorReadout.textContent = `X ${fmtRangeSigned(map.cursorWorld.x)} · Y ${fmtRangeSigned(map.cursorWorld.y)} NM`;
  }

  function fmtRangeSigned(value) {
    const n = numberOr(value, 0);
    const sign = n > 0 ? '+' : '';
    return `${sign}${n.toFixed(1)}`;
  }

  function handleMapClick(screenX, screenY) {
    const hit = pickAt(screenX, screenY);
    if (hit) {
      if (hit.type === 'unit') {
        state.selectedUnitId = hit.id;
        state.fireOptionsKey = null;
        renderSidebar();
        return;
      }
      if (hit.type === 'contact') {
        state.selectedContactId = hit.id;
        state.fireOptionsKey = null;
        renderSidebar();
        return;
      }
    }
    appendWaypoint(screenToWorld(screenX, screenY));
  }

  function setupMapInteractions() {
    const canvas = dom.mapCanvas;

    canvas.addEventListener('pointerdown', (event) => {
      if (event.button !== 0) return;
      canvas.focus({ preventScroll: true });
      const point = localPoint(event);
      map.drag = {
        pointerId: event.pointerId,
        startX: point.x,
        startY: point.y,
        lastX: point.x,
        lastY: point.y,
        moved: false,
      };
      try {
        canvas.setPointerCapture(event.pointerId);
      } catch {
        /* pointer capture is best-effort */
      }
    });

    canvas.addEventListener('pointermove', (event) => {
      const point = localPoint(event);
      if (map.drag && map.drag.pointerId === event.pointerId) {
        const dx = point.x - map.drag.lastX;
        const dy = point.y - map.drag.lastY;
        const totalDx = point.x - map.drag.startX;
        const totalDy = point.y - map.drag.startY;
        if (!map.drag.moved && Math.hypot(totalDx, totalDy) > 4) map.drag.moved = true;
        if (map.drag.moved) {
          map.view.cx -= dx / map.view.scale;
          map.view.cy += dy / map.view.scale;
          map.userAdjusted = true;
          canvas.classList.add('is-dragging');
          map.cursorWorld = screenToWorld(point.x, point.y);
        }
        map.drag.lastX = point.x;
        map.drag.lastY = point.y;
        updateCursorReadout();
        return;
      }
      updateHover(point.x, point.y);
    });

    const endDrag = (event) => {
      if (!map.drag || (event.pointerId !== undefined && event.pointerId !== map.drag.pointerId)) return;
      const point = localPoint(event);
      const wasMoved = map.drag.moved;
      map.drag = null;
      canvas.classList.remove('is-dragging');
      try {
        if (event.pointerId !== undefined) canvas.releasePointerCapture(event.pointerId);
      } catch {
        /* pointer may already be released */
      }
      if (!wasMoved) handleMapClick(point.x, point.y);
      updateHover(point.x, point.y);
    };

    canvas.addEventListener('pointerup', endDrag);
    canvas.addEventListener('pointercancel', () => {
      map.drag = null;
      canvas.classList.remove('is-dragging');
    });
    canvas.addEventListener('pointerleave', () => {
      if (map.drag) return;
      map.pointerInside = false;
      map.hover = null;
      dom.mapCanvas.classList.remove('is-targeting');
      updateCursorReadout();
    });
    canvas.addEventListener('contextmenu', (event) => event.preventDefault());

    canvas.addEventListener('wheel', (event) => {
      event.preventDefault();
      const point = localPoint(event);
      const factor = Math.exp(-event.deltaY * 0.0014);
      zoomAt(point.x, point.y, clamp(factor, 0.55, 1.8));
      updateHover(point.x, point.y);
    }, { passive: false });

    dom.mapFit.addEventListener('click', () => {
      map.userAdjusted = false;
      map.fitPending = false;
      fitMap();
    });
    dom.mapZoomIn.addEventListener('click', () => zoomCenter(1.35));
    dom.mapZoomOut.addEventListener('click', () => zoomCenter(1 / 1.35));

    new ResizeObserver(() => resizeMapCanvas()).observe(dom.mapShell);
    window.addEventListener('resize', () => resizeMapCanvas());
  }

  // ------------------------------------------------------------------
  // Map rendering
  // ------------------------------------------------------------------
  function updateSmoothPositions(dt) {
    if (!state.observation) return;
    const obs = state.observation;
    const valid = new Set();
    const smoothing = 1 - Math.exp(-dt / 135);

    const update = (key, pos) => {
      if (!pos) return;
      valid.add(key);
      const targetX = numberOr(pos.x);
      const targetY = numberOr(pos.y);
      let current = map.smooth.get(key);
      if (!current) {
        current = { x: targetX, y: targetY };
        map.smooth.set(key, current);
        return;
      }
      const jump = Math.hypot(targetX - current.x, targetY - current.y);
      if (jump > 160) {
        current.x = targetX;
        current.y = targetY;
      } else {
        current.x += (targetX - current.x) * smoothing;
        current.y += (targetY - current.y) * smoothing;
      }
    };

    for (const unit of Array.isArray(obs.units) ? obs.units : []) {
      if (unit && unit.pos) update(`u:${unit.id}`, unit.pos);
    }
    for (const contact of Array.isArray(obs.contacts) ? obs.contacts : []) {
      if (contact && contact.estimate) update(`c:${contact.id}`, contact.estimate);
    }
    for (const key of Array.from(map.smooth.keys())) {
      if (!valid.has(key)) map.smooth.delete(key);
    }
  }

  function drawMap(now, dt) {
    const ctx = map.ctx;
    if (!ctx) return;
    updateSmoothPositions(dt);

    ctx.setTransform(map.dpr, 0, 0, map.dpr, 0, 0);
    ctx.clearRect(0, 0, map.w, map.h);
    drawOcean(ctx);
    drawGrid(ctx);

    const obs = state.observation;
    if (!obs) {
      drawIdleMessage(ctx);
      updateScaleDisplay();
      return;
    }

    const selected = selectedUnit();
    drawRangeRings(ctx, selected);
    drawRoutes(ctx, selected);
    drawAssignmentLine(ctx, selected);
    drawContacts(ctx, obs, now);
    drawUnits(ctx, obs, selected, now);
    drawHoverPreview(ctx, selected);
    updateScaleDisplay();
  }

  function drawOcean(ctx) {
    const gradient = ctx.createLinearGradient(0, 0, map.w, map.h);
    gradient.addColorStop(0, '#081b2b');
    gradient.addColorStop(0.48, '#061522');
    gradient.addColorStop(1, '#030d17');
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, map.w, map.h);

    const glow = ctx.createRadialGradient(
      map.w * 0.22, map.h * 0.14, 0,
      map.w * 0.22, map.h * 0.14, Math.max(map.w, map.h) * 0.72,
    );
    glow.addColorStop(0, 'rgba(30, 103, 137, 0.13)');
    glow.addColorStop(0.55, 'rgba(12, 49, 72, 0.035)');
    glow.addColorStop(1, 'rgba(0, 0, 0, 0)');
    ctx.fillStyle = glow;
    ctx.fillRect(0, 0, map.w, map.h);

    const vignette = ctx.createRadialGradient(
      map.w / 2, map.h / 2, Math.min(map.w, map.h) * 0.24,
      map.w / 2, map.h / 2, Math.max(map.w, map.h) * 0.72,
    );
    vignette.addColorStop(0, 'rgba(0, 0, 0, 0)');
    vignette.addColorStop(1, 'rgba(0, 4, 9, 0.42)');
    ctx.fillStyle = vignette;
    ctx.fillRect(0, 0, map.w, map.h);
  }

  function niceStep(raw) {
    const value = Math.max(raw, 1e-6);
    const exponent = Math.floor(Math.log10(value));
    const base = Math.pow(10, exponent);
    const fraction = value / base;
    const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10;
    return nice * base;
  }

  function formatGridValue(value, step) {
    const n = numberOr(value, 0);
    const abs = Math.abs(n);
    if (abs < 0.0001) return '0';
    if (step < 0.1) return n.toFixed(2);
    if (step < 1) return n.toFixed(1);
    return String(Math.round(n));
  }

  function drawGrid(ctx) {
    const scale = map.view.scale;
    if (!Number.isFinite(scale) || scale <= 0) return;

    const topLeft = screenToWorld(0, 0);
    const bottomRight = screenToWorld(map.w, map.h);
    const minX = Math.min(topLeft.x, bottomRight.x);
    const maxX = Math.max(topLeft.x, bottomRight.x);
    const minY = Math.min(topLeft.y, bottomRight.y);
    const maxY = Math.max(topLeft.y, bottomRight.y);

    const major = niceStep(68 / scale);
    const minor = major / 5;
    const drawMinor = minor * scale >= 9;

    ctx.save();
    ctx.beginPath();
    ctx.rect(0, 0, map.w, map.h);
    ctx.clip();

    if (drawMinor) {
      ctx.beginPath();
      ctx.strokeStyle = 'rgba(77, 132, 159, 0.095)';
      ctx.lineWidth = 1;
      for (let x = Math.ceil(minX / minor) * minor; x <= maxX; x += minor) {
        const sx = worldToScreen(x, 0).x;
        ctx.moveTo(Math.round(sx) + 0.5, 0);
        ctx.lineTo(Math.round(sx) + 0.5, map.h);
      }
      for (let y = Math.ceil(minY / minor) * minor; y <= maxY; y += minor) {
        const sy = worldToScreen(0, y).y;
        ctx.moveTo(0, Math.round(sy) + 0.5);
        ctx.lineTo(map.w, Math.round(sy) + 0.5);
      }
      ctx.stroke();
    }

    ctx.beginPath();
    ctx.strokeStyle = 'rgba(84, 152, 184, 0.2)';
    ctx.lineWidth = 1;
    for (let x = Math.ceil(minX / major) * major; x <= maxX; x += major) {
      const sx = worldToScreen(x, 0).x;
      ctx.moveTo(Math.round(sx) + 0.5, 0);
      ctx.lineTo(Math.round(sx) + 0.5, map.h);
    }
    for (let y = Math.ceil(minY / major) * major; y <= maxY; y += major) {
      const sy = worldToScreen(0, y).y;
      ctx.moveTo(0, Math.round(sy) + 0.5);
      ctx.lineTo(map.w, Math.round(sy) + 0.5);
    }
    ctx.stroke();

    // Referee origin and axes.
    const origin = worldToScreen(0, 0);
    if (origin.x > -2 && origin.x < map.w + 2) {
      ctx.beginPath();
      ctx.strokeStyle = 'rgba(123, 195, 222, 0.4)';
      ctx.moveTo(Math.round(origin.x) + 0.5, 0);
      ctx.lineTo(Math.round(origin.x) + 0.5, map.h);
      ctx.stroke();
    }
    if (origin.y > -2 && origin.y < map.h + 2) {
      ctx.beginPath();
      ctx.strokeStyle = 'rgba(123, 195, 222, 0.4)';
      ctx.moveTo(0, Math.round(origin.y) + 0.5);
      ctx.lineTo(map.w, Math.round(origin.y) + 0.5);
      ctx.stroke();
    }
    if (origin.x > 8 && origin.x < map.w - 8 && origin.y > 45 && origin.y < map.h - 38) {
      ctx.strokeStyle = 'rgba(150, 214, 238, 0.52)';
      ctx.beginPath();
      ctx.moveTo(origin.x - 3, origin.y);
      ctx.lineTo(origin.x + 3, origin.y);
      ctx.moveTo(origin.x, origin.y - 3);
      ctx.lineTo(origin.x, origin.y + 3);
      ctx.stroke();
    }

    ctx.font = `8px ${MONO}`;
    ctx.fillStyle = 'rgba(140, 185, 203, 0.55)';
    ctx.strokeStyle = 'rgba(3, 12, 20, 0.85)';
    ctx.lineWidth = 2.5;
    ctx.textBaseline = 'bottom';
    const xLabelY = map.h - 40;
    for (let x = Math.ceil(minX / major) * major; x <= maxX; x += major) {
      const sx = worldToScreen(x, 0).x;
      if (sx < 8 || sx > map.w - 8) continue;
      const label = formatGridValue(x, major);
      ctx.textAlign = 'center';
      ctx.strokeText(label, sx, xLabelY);
      ctx.fillText(label, sx, xLabelY);
    }

    ctx.textAlign = 'left';
    ctx.textBaseline = 'middle';
    for (let y = Math.ceil(minY / major) * major; y <= maxY; y += major) {
      const sy = worldToScreen(0, y).y;
      if (sy < 45 || sy > map.h - 42) continue;
      const label = formatGridValue(y, major);
      ctx.strokeText(label, 7, sy);
      ctx.fillText(label, 7, sy);
    }

    ctx.restore();
  }

  function drawIdleMessage(ctx) {
    ctx.save();
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillStyle = 'rgba(138, 184, 202, 0.38)';
    ctx.font = `600 10px ${MONO}`;
    ctx.fillText('AWAITING TACTICAL PICTURE', map.w / 2, map.h / 2 - 6);
    ctx.fillStyle = 'rgba(80, 123, 142, 0.42)';
    ctx.font = `8px ${MONO}`;
    ctx.fillText('SELECT A SCENARIO TO DEPLOY', map.w / 2, map.h / 2 + 12);
    ctx.restore();
  }

  function drawRangeRings(ctx, unit) {
    if (!unit) return;
    const pos = map.smooth.get(`u:${unit.id}`) || unit.pos;
    const center = worldToScreen(pos.x, pos.y);
    const ringNm = niceStep(74 / map.view.scale);
    const color = `rgba(${sideStyle(state.side).rgb}, 0.16)`;
    ctx.save();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1;
    ctx.setLineDash([2, 6]);
    ctx.font = `7px ${MONO}`;
    ctx.fillStyle = `rgba(${sideStyle(state.side).rgb}, 0.44)`;
    for (let index = 1; index <= 2; index += 1) {
      const radius = ringNm * index * map.view.scale;
      if (radius > Math.max(map.w, map.h) * 0.75) continue;
      ctx.beginPath();
      ctx.arc(center.x, center.y, radius, 0, Math.PI * 2);
      ctx.stroke();
      if (index === 1) {
        const labelX = center.x + radius;
        const labelY = center.y - 4;
        if (labelX < map.w - 22 && labelX > 12) {
          ctx.setLineDash([]);
          ctx.textAlign = 'left';
          ctx.textBaseline = 'bottom';
          ctx.fillText(`${formatGridValue(ringNm, ringNm)} NM`, labelX + 4, labelY);
        }
      }
    }
    ctx.restore();
  }

  function drawRoutes(ctx, unit) {
    if (!unit) return;
    const route = state.routes.get(routeKey(unit.id));
    if (!route || !route.length) return;
    const pos = map.smooth.get(`u:${unit.id}`) || unit.pos;
    const start = worldToScreen(pos.x, pos.y);
    const style = sideStyle(state.side);

    ctx.save();
    ctx.strokeStyle = `rgba(${style.rgb}, 0.7)`;
    ctx.lineWidth = 1.2;
    ctx.setLineDash([4, 5]);
    ctx.beginPath();
    ctx.moveTo(start.x, start.y);
    for (const point of route) {
      const screen = worldToScreen(point.x, point.y);
      ctx.lineTo(screen.x, screen.y);
    }
    ctx.stroke();
    ctx.setLineDash([]);

    for (let index = 0; index < route.length; index += 1) {
      const point = route[index];
      const screen = worldToScreen(point.x, point.y);
      const last = index === route.length - 1;
      const radius = last ? 6 : 4.5;
      ctx.beginPath();
      ctx.fillStyle = 'rgba(3, 13, 21, 0.88)';
      ctx.arc(screen.x, screen.y, radius, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = `rgba(${style.rgb}, ${last ? 0.95 : 0.68})`;
      ctx.lineWidth = 1;
      ctx.stroke();
      ctx.fillStyle = last ? style.hex : `rgba(${style.rgb}, 0.8)`;
      ctx.font = `600 7px ${MONO}`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(String(index + 1), screen.x, screen.y + 0.5);
      if (last) {
        ctx.strokeStyle = `rgba(${style.rgb}, 0.5)`;
        ctx.beginPath();
        ctx.moveTo(screen.x - 10, screen.y);
        ctx.lineTo(screen.x - 7, screen.y);
        ctx.moveTo(screen.x + 7, screen.y);
        ctx.lineTo(screen.x + 10, screen.y);
        ctx.moveTo(screen.x, screen.y - 10);
        ctx.lineTo(screen.x, screen.y - 7);
        ctx.moveTo(screen.x, screen.y + 7);
        ctx.lineTo(screen.x, screen.y + 10);
        ctx.stroke();
      }
    }
    ctx.restore();
  }

  function drawAssignmentLine(ctx, unit) {
    if (!unit || unit.assigned === null || unit.assigned === undefined) return;
    const contact = findContactById(unit.assigned);
    if (!contact) return;
    const start = screenPositionForUnit(unit);
    const end = screenPositionForContact(contact);
    ctx.save();
    ctx.strokeStyle = 'rgba(242, 182, 96, 0.34)';
    ctx.lineWidth = 1;
    ctx.setLineDash([2, 6]);
    ctx.beginPath();
    ctx.moveTo(start.x, start.y);
    ctx.lineTo(end.x, end.y);
    ctx.stroke();
    ctx.restore();
  }

  function drawContacts(ctx, obs, now) {
    const contacts = Array.isArray(obs.contacts) ? obs.contacts : [];
    for (const contact of contacts) {
      const pos = map.smooth.get(`c:${contact.id}`) || contact.estimate;
      const screen = worldToScreen(pos.x, pos.y);
      if (!Number.isFinite(screen.x) || !Number.isFinite(screen.y)) continue;
      const margin = 90;
      if (screen.x < -margin || screen.x > map.w + margin || screen.y < -margin || screen.y > map.h + margin) continue;
      drawContact(ctx, contact, screen, now);
    }
  }

  function drawContact(ctx, contact, screen, now) {
    const cat = categoryInfo(contact.category);
    const quality = clamp(numberOr(contact.quality, 0), 0, 1);
    const stale = !!contact.stale;
    const selected = state.selectedContactId !== null && Number(contact.id) === state.selectedContactId;
    const hovered = !!(map.hover && map.hover.type === 'contact' && map.hover.id === Number(contact.id));
    const assigned = (() => {
      const unit = selectedUnit();
      return !!(unit && unit.assigned !== null && unit.assigned !== undefined && Number(unit.assigned) === Number(contact.id));
    })();
    const size = contactSymbolSize(contact);
    const rgb = cat.color;
    const qualityAlpha = clamp(0.28 + quality * 0.7, 0.28, 0.98) * (stale ? 0.62 : 1);

    ctx.save();
    // Track quality always modulates the contact plot. Selection brackets are
    // drawn separately at full strength so a faint plot is still selectable.
    ctx.globalAlpha = qualityAlpha;
    ctx.translate(screen.x, screen.y);
    ctx.rotate((numberOr(contact.est_course_deg) * Math.PI) / 180);

    if (numberOr(contact.est_speed_kts, 0) > 0) {
      const vectorLength = 8 + Math.min(20, numberOr(contact.est_speed_kts, 0) * 0.3);
      ctx.beginPath();
      ctx.strokeStyle = `rgba(${rgb}, 0.58)`;
      ctx.lineWidth = 1;
      ctx.setLineDash(stale ? [2, 3] : []);
      ctx.moveTo(0, -size * 1.05);
      ctx.lineTo(0, -size * 1.05 - vectorLength);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(-2, -size * 1.05 - vectorLength + 3);
      ctx.lineTo(0, -size * 1.05 - vectorLength);
      ctx.lineTo(2, -size * 1.05 - vectorLength + 3);
      ctx.stroke();
    }

    ctx.beginPath();
    if (cat.shape === 'rect') {
      const width = size * 1.9;
      const height = size * 0.92;
      ctx.rect(-width / 2, -height / 2, width, height);
    } else if (cat.shape === 'triangle') {
      ctx.moveTo(0, -size);
      ctx.lineTo(size * 0.88, size * 0.7);
      ctx.lineTo(0, size * 0.34);
      ctx.lineTo(-size * 0.88, size * 0.7);
      ctx.closePath();
    } else if (cat.shape === 'diamond') {
      ctx.moveTo(0, -size);
      ctx.lineTo(size * 0.9, 0);
      ctx.lineTo(0, size);
      ctx.lineTo(-size * 0.9, 0);
      ctx.closePath();
    } else if (cat.shape === 'square') {
      ctx.rect(-size * 0.75, -size * 0.75, size * 1.5, size * 1.5);
    } else {
      const radius = cat.key === 'torpedo' ? Math.max(2.4, size * 0.8) : size * 0.82;
      ctx.arc(0, 0, radius, 0, Math.PI * 2);
    }

    ctx.fillStyle = `rgba(${rgb}, 0.16)`;
    ctx.fill();
    ctx.strokeStyle = `rgba(${rgb}, ${stale ? 0.62 : 0.94})`;
    ctx.lineWidth = stale ? 1 : 1.25;
    ctx.setLineDash(stale ? [3, 3] : []);
    ctx.stroke();
    ctx.setLineDash([]);

    if (!contact.class_name) {
      ctx.beginPath();
      ctx.fillStyle = `rgba(${rgb}, 0.72)`;
      ctx.arc(0, 0, 1.4, 0, Math.PI * 2);
      ctx.fill();
    }

    if (contact.bearing_only) {
      // A small azimuth tick marks a bearing-only track without inventing
      // a range solution.
      ctx.beginPath();
      ctx.strokeStyle = `rgba(${rgb}, 0.5)`;
      ctx.setLineDash([1, 3]);
      ctx.arc(0, 0, size + 4, -0.42, 0.42);
      ctx.stroke();
      ctx.setLineDash([]);
    }

    ctx.restore();

    if (hovered && !selected) {
      ctx.save();
      ctx.strokeStyle = 'rgba(220, 240, 248, 0.36)';
      ctx.lineWidth = 1;
      ctx.setLineDash([2, 3]);
      ctx.beginPath();
      ctx.arc(screen.x, screen.y, size + 7, 0, Math.PI * 2);
      ctx.stroke();
      ctx.restore();
    }

    if (selected) {
      drawSelectionBrackets(ctx, screen.x, screen.y, size + 8, '#f2b660', now, 0.28);
    } else if (assigned) {
      drawSelectionBrackets(ctx, screen.x, screen.y, size + 8, '#f2b660', now, 0);
    }

    const anonymousProjectile = (cat.key === 'missile' || cat.key === 'torpedo') && !contact.class_name;
    if (selected || hovered || assigned || !anonymousProjectile) {
      drawContactLabel(ctx, screen, size, contact, cat, selected, stale);
    }
  }

  function drawContactLabel(ctx, screen, size, contact, cat, selected, stale) {
    const quality = Math.round(clamp(numberOr(contact.quality, 0), 0, 1) * 100);
    const className = contact.class_name ? String(contact.class_name).toUpperCase() : cat.label.toUpperCase();
    const speed = numberOr(contact.est_speed_kts, 0);
    const line1 = `C${contact.id}  ${className}`;
    const line2Parts = [`Q${quality}`];
    if (speed > 0) line2Parts.push(`${Math.round(speed)}KT`);
    if (contact.bearing_only) line2Parts.push('BRG');
    if (stale) line2Parts.push('STALE');
    const line2 = line2Parts.join(' · ');

    ctx.save();
    ctx.globalAlpha = clamp(0.32 + numberOr(contact.quality, 0) * 0.64, 0.32, 0.94) * (stale ? 0.66 : 1);
    ctx.font = `600 8.5px ${MONO}`;
    const maxLabelWidth = 150;
    const fitted1 = fitCanvasText(ctx, line1, maxLabelWidth);
    ctx.font = `7px ${MONO}`;
    const fitted2 = fitCanvasText(ctx, line2, maxLabelWidth);
    const width = clamp(Math.max(ctx.measureText(fitted1).width, ctx.measureText(fitted2).width) + 11, 48, 160);
    const height = 27;
    let x = screen.x + size + 8;
    if (x + width > map.w - 6) x = screen.x - size - 8 - width;
    x = clamp(x, 6, Math.max(6, map.w - width - 6));
    let y = screen.y - height / 2;
    y = clamp(y, 45, Math.max(45, map.h - 43 - height));

    ctx.fillStyle = selected ? 'rgba(13, 22, 27, 0.93)' : 'rgba(3, 12, 20, 0.76)';
    ctx.fillRect(x, y, width, height);
    ctx.strokeStyle = selected ? 'rgba(242, 182, 96, 0.74)' : 'rgba(137, 188, 207, 0.18)';
    ctx.lineWidth = 1;
    ctx.strokeRect(x + 0.5, y + 0.5, width - 1, height - 1);
    if (selected) {
      ctx.fillStyle = '#f2b660';
      ctx.fillRect(x, y, 2, height);
    }

    ctx.textAlign = 'left';
    ctx.textBaseline = 'top';
    ctx.font = `600 8.5px ${MONO}`;
    ctx.fillStyle = selected ? '#f4e3c5' : `rgba(${cat.color}, 0.95)`;
    ctx.fillText(fitted1, x + 6, y + 5);
    ctx.font = `7px ${MONO}`;
    ctx.fillStyle = 'rgba(179, 207, 218, 0.68)';
    ctx.fillText(fitted2, x + 6, y + 16);
    ctx.restore();
  }

  function drawUnits(ctx, obs, selectedUnitInPanel, now) {
    const units = Array.isArray(obs.units) ? obs.units : [];
    const style = sideStyle(state.side);
    const entries = [];

    for (const unit of units) {
      const pos = map.smooth.get(`u:${unit.id}`) || unit.pos;
      const screen = worldToScreen(pos.x, pos.y);
      if (!Number.isFinite(screen.x) || !Number.isFinite(screen.y)) continue;
      if (screen.x < -160 || screen.x > map.w + 160 || screen.y < -80 || screen.y > map.h + 80) continue;
      entries.push({
        unit,
        screen,
        selected: !!(selectedUnitInPanel && Number(selectedUnitInPanel.id) === Number(unit.id)),
      });
    }

    const layouts = layoutUnitLabels(entries);
    for (const entry of entries) {
      drawUnit(ctx, entry.unit, entry.screen, style, entry.selected, now, layouts.get(Number(entry.unit.id)));
    }
  }

  function intersectArea(a, b) {
    const width = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
    const height = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
    return width > 0 && height > 0 ? width * height : 0;
  }

  function screenObstacles() {
    const shell = dom.mapShell.getBoundingClientRect();
    const nodes = [
      dom.mapShell.querySelector('.map-brief'),
      dom.mapShell.querySelector('.map-legend'),
      dom.mapShell.querySelector('.map-tools'),
      dom.mapShell.querySelector('.map-footer'),
      dom.outcomePanel.hidden ? null : dom.outcomePanel,
    ].filter(Boolean);

    return nodes.map((node) => {
      const rect = node.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return null;
      return {
        x: rect.left - shell.left - 5,
        y: rect.top - shell.top - 5,
        w: rect.width + 10,
        h: rect.height + 10,
      };
    }).filter(Boolean);
  }

  function layoutUnitLabels(entries) {
    const width = 145;
    const height = 45;
    const iconRadius = 20;
    const gap = 10;
    const occupied = screenObstacles();
    const iconBoxes = entries.map((entry) => ({
      id: Number(entry.unit.id),
      x: entry.screen.x - 17,
      y: entry.screen.y - 17,
      w: 34,
      h: 34,
    }));
    const layouts = new Map();
    const ordered = entries.slice().sort((a, b) => (
      Number(b.selected) - Number(a.selected) || Number(a.unit.id) - Number(b.unit.id)
    ));

    for (const entry of ordered) {
      const candidates = [
        { x: entry.screen.x + iconRadius + gap, y: entry.screen.y - height / 2 },
        { x: entry.screen.x - iconRadius - gap - width, y: entry.screen.y - height / 2 },
        { x: entry.screen.x + iconRadius + gap, y: entry.screen.y - height - 8 },
        { x: entry.screen.x - iconRadius - gap - width, y: entry.screen.y - height - 8 },
        { x: entry.screen.x + iconRadius + gap, y: entry.screen.y + 8 },
        { x: entry.screen.x - iconRadius - gap - width, y: entry.screen.y + 8 },
        { x: entry.screen.x - width / 2, y: entry.screen.y - iconRadius - gap - height },
        { x: entry.screen.x - width / 2, y: entry.screen.y + iconRadius + gap },
      ].map((candidate) => ({
        ...candidate,
        x: clamp(candidate.x, 6, Math.max(6, map.w - width - 6)),
        y: clamp(candidate.y, 44, Math.max(44, map.h - 42 - height)),
        w: width,
        h: height,
      }));

      let selectedBox = candidates[0];
      let bestScore = Infinity;
      for (const box of candidates) {
        let score = 0;
        for (const reserved of occupied) score += intersectArea(box, reserved);
        for (const icon of iconBoxes) {
          if (icon.id === Number(entry.unit.id)) continue;
          score += intersectArea(box, icon) * 2;
        }
        if (score < bestScore - 0.01) {
          bestScore = score;
          selectedBox = box;
          if (score === 0) break;
        }
      }

      layouts.set(Number(entry.unit.id), selectedBox);
      occupied.push({
        x: selectedBox.x - 3,
        y: selectedBox.y - 3,
        w: selectedBox.w + 6,
        h: selectedBox.h + 6,
      });
    }

    return layouts;
  }

  function drawUnit(ctx, unit, screen, style, selected, now, labelBox) {
    const size = 20;
    const course = numberOr(unit.course_deg, 0);
    const speed = numberOr(unit.speed_kts, 0);
    const color = style.hex;
    const hovered = !!(map.hover && map.hover.type === 'unit' && map.hover.id === Number(unit.id));
    const angle = (course * Math.PI) / 180;

    ctx.save();

    if (selected) {
      const pulse = 1 + Math.sin(now / 300) * 0.035;
      const glow = ctx.createRadialGradient(screen.x, screen.y, 3, screen.x, screen.y, size * 2.2 * pulse);
      glow.addColorStop(0, `rgba(${style.rgb}, 0.17)`);
      glow.addColorStop(1, `rgba(${style.rgb}, 0)`);
      ctx.fillStyle = glow;
      ctx.beginPath();
      ctx.arc(screen.x, screen.y, size * 2.2 * pulse, 0, Math.PI * 2);
      ctx.fill();
    }

    // Course vector. The solid line is the actual course.
    const vector = 14 + clamp(speed * 0.75, 0, 30);
    ctx.strokeStyle = selected ? 'rgba(235, 251, 255, 0.86)' : `rgba(${style.rgb}, 0.7)`;
    ctx.lineWidth = selected ? 1.5 : 1.1;
    ctx.beginPath();
    ctx.moveTo(
      screen.x + Math.sin(angle) * (size * 0.55),
      screen.y - Math.cos(angle) * (size * 0.55),
    );
    ctx.lineTo(
      screen.x + Math.sin(angle) * (size * 0.55 + vector),
      screen.y - Math.cos(angle) * (size * 0.55 + vector),
    );
    ctx.stroke();
    const tipX = screen.x + Math.sin(angle) * (size * 0.55 + vector);
    const tipY = screen.y - Math.cos(angle) * (size * 0.55 + vector);
    drawArrowHead(ctx, tipX, tipY, angle, selected ? 'rgba(235, 251, 255, 0.86)' : `rgba(${style.rgb}, 0.7)`);

    if (selected && Math.abs(((numberOr(unit.ordered_course_deg, course) - course + 540) % 360) - 180) > 4) {
      const orderedAngle = (numberOr(unit.ordered_course_deg, course) * Math.PI) / 180;
      ctx.strokeStyle = `rgba(${style.rgb}, 0.38)`;
      ctx.setLineDash([2, 4]);
      ctx.beginPath();
      ctx.moveTo(screen.x, screen.y);
      ctx.lineTo(
        screen.x + Math.sin(orderedAngle) * (size + 24),
        screen.y - Math.cos(orderedAngle) * (size + 24),
      );
      ctx.stroke();
      ctx.setLineDash([]);
    }

    ctx.translate(screen.x, screen.y);
    ctx.rotate(angle);

    ctx.beginPath();
    const kind = String(unit.kind || 'Ship');
    if (kind === 'Aircraft') {
      ctx.moveTo(0, -12);
      ctx.lineTo(9, 7);
      ctx.lineTo(0, 3.5);
      ctx.lineTo(-9, 7);
      ctx.closePath();
    } else if (kind === 'Sub') {
      ctx.moveTo(0, -11);
      ctx.lineTo(6, -3);
      ctx.lineTo(5, 8);
      ctx.lineTo(0, 11);
      ctx.lineTo(-5, 8);
      ctx.lineTo(-6, -3);
      ctx.closePath();
    } else if (kind === 'Base') {
      ctx.rect(-7, -7, 14, 14);
      ctx.rect(-2.5, -2.5, 5, 5);
    } else {
      // Surface hull: fine bow, broad stern.
      ctx.moveTo(0, -12);
      ctx.lineTo(5, -4);
      ctx.lineTo(5.5, 8);
      ctx.lineTo(-5.5, 8);
      ctx.lineTo(-5, -4);
      ctx.closePath();
    }
    ctx.fillStyle = selected ? `rgba(${style.rgb}, 0.22)` : `rgba(${style.rgb}, 0.12)`;
    ctx.fill();
    ctx.strokeStyle = color;
    ctx.lineWidth = selected ? 1.9 : 1.45;
    ctx.stroke();

    if (selected) {
      ctx.beginPath();
      ctx.arc(0, 0, 2.2, 0, Math.PI * 2);
      ctx.fillStyle = color;
      ctx.fill();
    } else {
      ctx.beginPath();
      ctx.arc(0, 0, 1.4, 0, Math.PI * 2);
      ctx.fillStyle = `rgba(${style.rgb}, 0.72)`;
      ctx.fill();
    }
    ctx.restore();

    if (hovered && !selected) {
      ctx.save();
      ctx.strokeStyle = `rgba(${style.rgb}, 0.4)`;
      ctx.lineWidth = 1;
      ctx.setLineDash([2, 3]);
      ctx.beginPath();
      ctx.arc(screen.x, screen.y, size + 6, 0, Math.PI * 2);
      ctx.stroke();
      ctx.restore();
    }

    if (selected) drawSelectionBrackets(ctx, screen.x, screen.y, size + 8, color, now, 0.34);
    drawUnitLabel(ctx, screen, size, unit, style, selected, labelBox);
  }

  function drawArrowHead(ctx, x, y, angle, color) {
    const len = 4.6;
    ctx.save();
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x - Math.sin(angle + 0.52) * len, y + Math.cos(angle + 0.52) * len);
    ctx.lineTo(x - Math.sin(angle - 0.52) * len, y + Math.cos(angle - 0.52) * len);
    ctx.closePath();
    ctx.fill();
    ctx.restore();
  }

  function drawSelectionBrackets(ctx, cx, cy, radius, color, now, glow) {
    const pulse = radius + Math.sin(now / 260) * 1.3;
    const arm = Math.max(5, pulse * 0.34);
    ctx.save();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1;
    ctx.globalAlpha = 0.95;
    ctx.setLineDash([]);
    ctx.beginPath();
    ctx.moveTo(cx - pulse, cy - pulse + arm);
    ctx.lineTo(cx - pulse, cy - pulse);
    ctx.lineTo(cx - pulse + arm, cy - pulse);
    ctx.moveTo(cx + pulse - arm, cy - pulse);
    ctx.lineTo(cx + pulse, cy - pulse);
    ctx.lineTo(cx + pulse, cy - pulse + arm);
    ctx.moveTo(cx + pulse, cy + pulse - arm);
    ctx.lineTo(cx + pulse, cy + pulse);
    ctx.lineTo(cx + pulse - arm, cy + pulse);
    ctx.moveTo(cx - pulse + arm, cy + pulse);
    ctx.lineTo(cx - pulse, cy + pulse);
    ctx.lineTo(cx - pulse, cy + pulse - arm);
    ctx.stroke();

    if (glow > 0) {
      ctx.globalAlpha = 0.35 + Math.sin(now / 300) * 0.12;
      ctx.setLineDash([2, 5]);
      ctx.beginPath();
      ctx.arc(cx, cy, pulse + 4, 0, Math.PI * 2);
      ctx.stroke();
    }
    ctx.restore();
  }

  function drawUnitLabel(ctx, screen, iconRadius, unit, style, selected, layout) {
    const name = unit.name || `UNIT #${unit.id}`;
    const speed = numberOr(unit.speed_kts, 0);
    const hp = numberOr(unit.hp, 0);
    const hpMax = Math.max(1, numberOr(unit.hp_max, 1));
    const ratio = clamp(hp / hpMax, 0, 1);
    const width = 145;
    const height = 45;
    const x = layout ? layout.x : screen.x + iconRadius + 10;
    const y = layout ? layout.y : screen.y - height / 2;

    ctx.save();

    // A short leader keeps displaced labels associated with their hulls.
    const nearestX = clamp(screen.x, x, x + width);
    const nearestY = clamp(screen.y, y, y + height);
    const dx = nearestX - screen.x;
    const dy = nearestY - screen.y;
    const distance = Math.hypot(dx, dy);
    if (distance > iconRadius + 7) {
      const startX = screen.x + (dx / distance) * (iconRadius + 5);
      const startY = screen.y + (dy / distance) * (iconRadius + 5);
      ctx.strokeStyle = selected ? `rgba(${style.rgb}, 0.6)` : 'rgba(130, 190, 214, 0.28)';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(startX, startY);
      ctx.lineTo(nearestX, nearestY);
      ctx.stroke();
      ctx.beginPath();
      ctx.fillStyle = selected ? style.hex : 'rgba(146, 197, 217, 0.44)';
      ctx.arc(nearestX, nearestY, 1.4, 0, Math.PI * 2);
      ctx.fill();
    }

    ctx.fillStyle = selected ? 'rgba(5, 19, 29, 0.94)' : 'rgba(3, 12, 20, 0.82)';
    ctx.fillRect(x, y, width, height);
    ctx.strokeStyle = selected ? `rgba(${style.rgb}, 0.75)` : 'rgba(126, 186, 212, 0.2)';
    ctx.lineWidth = 1;
    ctx.strokeRect(x + 0.5, y + 0.5, width - 1, height - 1);
    if (selected) {
      ctx.fillStyle = style.hex;
      ctx.fillRect(x, y, 2, height);
    }

    ctx.textAlign = 'left';
    ctx.textBaseline = 'top';
    ctx.font = `600 9.5px ${MONO}`;
    ctx.fillStyle = selected ? '#effaff' : 'rgba(218, 236, 244, 0.9)';
    ctx.fillText(fitCanvasText(ctx, name, width - 12), x + 6, y + 5);

    ctx.font = `7px ${MONO}`;
    ctx.fillStyle = `rgba(${style.rgb}, 0.8)`;
    ctx.fillText(`${fmt1(speed)} KT · ${fmtCourse(unit.course_deg)}`, x + 6, y + 19);

    const barX = x + 6;
    const barY = y + 33;
    const barW = 76;
    const barH = 3;
    ctx.fillStyle = 'rgba(255, 255, 255, 0.1)';
    ctx.fillRect(barX, barY, barW, barH);
    ctx.fillStyle = ratio <= 0.28 ? '#ff6266' : ratio <= 0.6 ? '#f2b660' : '#55dda3';
    ctx.fillRect(barX, barY, barW * ratio, barH);
    ctx.fillStyle = 'rgba(184, 209, 220, 0.66)';
    ctx.font = `6.5px ${MONO}`;
    ctx.fillText(`HP ${fmtInt(hp)}/${fmtInt(hpMax)}`, x + 88, y + 31);
    ctx.restore();
  }

  function drawHoverPreview(ctx, selected) {
    if (!selected || !map.pointerInside || !map.cursorWorld || map.hover) return;
    const obs = state.observation;
    if (!obs || obs.over) return;
    const unitPos = map.smooth.get(`u:${selected.id}`) || selected.pos;
    const origin = worldToScreen(unitPos.x, unitPos.y);
    const cursor = worldToScreen(map.cursorWorld.x, map.cursorWorld.y);
    const style = sideStyle(state.side);

    ctx.save();
    ctx.strokeStyle = `rgba(${style.rgb}, 0.3)`;
    ctx.lineWidth = 1;
    ctx.setLineDash([3, 5]);
    ctx.beginPath();
    ctx.moveTo(origin.x, origin.y);
    ctx.lineTo(cursor.x, cursor.y);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.strokeStyle = `rgba(${style.rgb}, 0.72)`;
    ctx.beginPath();
    ctx.moveTo(cursor.x - 7, cursor.y);
    ctx.lineTo(cursor.x - 2, cursor.y);
    ctx.moveTo(cursor.x + 2, cursor.y);
    ctx.lineTo(cursor.x + 7, cursor.y);
    ctx.moveTo(cursor.x, cursor.y - 7);
    ctx.lineTo(cursor.x, cursor.y - 2);
    ctx.moveTo(cursor.x, cursor.y + 2);
    ctx.lineTo(cursor.x, cursor.y + 7);
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(cursor.x, cursor.y, 2.2, 0, Math.PI * 2);
    ctx.stroke();
    ctx.fillStyle = `rgba(${style.rgb}, 0.62)`;
    ctx.font = `7px ${MONO}`;
    ctx.textAlign = 'left';
    ctx.textBaseline = 'top';
    const distance = rangeNm(map.cursorWorld, unitPos);
    if (cursor.x + 56 < map.w) ctx.fillText(`${fmtRange(distance)} NM`, cursor.x + 10, cursor.y + 8);
    ctx.restore();
  }

  function updateScaleDisplay() {
    if (!map.ctx) return;
    const key = `${map.view.scale.toFixed(4)}:${map.w}`;
    if (key === map.lastScaleKey) return;
    map.lastScaleKey = key;

    const candidates = [0.1, 0.2, 0.5, 1, 2, 5, 10, 20, 25, 50, 100, 200, 500, 1000];
    let distance = candidates[candidates.length - 1];
    for (const candidate of candidates) {
      if (candidate * map.view.scale >= 55) {
        distance = candidate;
        break;
      }
    }
    const pixels = clamp(distance * map.view.scale, 26, 155);
    dom.scaleBar.style.width = `${pixels}px`;
    dom.scaleText.textContent = `${distance < 1 ? distance.toFixed(1) : distance.toFixed(0)} NM`;
  }

  function fitCanvasText(ctx, text, maxWidth) {
    const source = String(text ?? '');
    if (ctx.measureText(source).width <= maxWidth) return source;
    let candidate = source;
    while (candidate.length > 2 && ctx.measureText(`${candidate}…`).width > maxWidth) {
      candidate = candidate.slice(0, -1);
    }
    return `${candidate}…`;
  }

  // ------------------------------------------------------------------
  // UI event wiring and small helpers
  // ------------------------------------------------------------------
  function showToast(message, kind = 'info', duration = 3000) {
    if (!dom.toastStack) return;
    while (dom.toastStack.children.length >= 4) {
      dom.toastStack.firstElementChild.remove();
    }
    const toast = document.createElement('div');
    toast.className = `toast${kind === 'error' ? ' toast--error' : kind === 'ok' ? ' toast--ok' : kind === 'warn' ? ' toast--warn' : ''}`;
    toast.textContent = message;
    dom.toastStack.appendChild(toast);
    window.setTimeout(() => {
      toast.classList.add('is-leaving');
      window.setTimeout(() => toast.remove(), 200);
    }, duration);
  }

  function rangeNm(a, b) {
    if (!a || !b) return 0;
    return Math.hypot(numberOr(b.x) - numberOr(a.x), numberOr(b.y) - numberOr(a.y));
  }

  function bindUi() {
    dom.sideToggle.addEventListener('click', (event) => {
      const button = event.target.closest('button[data-side]');
      if (!button) return;
      setSide(button.dataset.side);
    });

    dom.speedGroup.addEventListener('click', (event) => {
      const button = event.target.closest('button[data-speed]');
      if (!button) return;
      void setSpeed(Number(button.dataset.speed));
    });

    dom.scenarioSelect.addEventListener('change', () => {
      const id = dom.scenarioSelect.value;
      if (!id || state.startingScenario) return;
      const previous = state.observation ? state.observation.scenario_id : state.scenarioId;
      void startScenario(id).then((started) => {
        if (!started && previous && state.scenarios.some((sc) => sc.id === previous)) {
          dom.scenarioSelect.value = previous;
          state.scenarioId = previous;
        }
      });
    });

    dom.btnStart.addEventListener('click', () => {
      const id = dom.scenarioSelect.value || state.scenarioId;
      if (id) void startScenario(id);
    });

    dom.btnWeaponsFree.addEventListener('click', () => void issueWeaponsFree());
    dom.btnChaff.addEventListener('click', () => void issueDeployChaff());
    dom.btnStop.addEventListener('click', () => void issueStop());
    dom.btnRecover.addEventListener('click', () => void issueRecoverAircraft());
    dom.btnAssign.addEventListener('click', () => void issueAssignTarget());
    dom.btnClearTarget.addEventListener('click', () => void issueClearTarget());
    dom.btnFire.addEventListener('click', () => void issueFire());

    dom.fireWeapon.addEventListener('change', () => {
      state.fireWeaponId = dom.fireWeapon.value || null;
      if (state.selectedUnitId !== null && state.selectedContactId !== null) {
        refreshFireControls(selectedUnit(), selectedContact(), state.observation);
      }
    });

    document.addEventListener('keydown', (event) => {
      const target = event.target;
      const typing = target && (target.tagName === 'INPUT' || target.tagName === 'SELECT' || target.tagName === 'TEXTAREA');
      if (typing) return;
      if (event.key === 'Escape') {
        if (state.selectedContactId !== null) {
          state.selectedContactId = null;
          renderSidebar();
        }
      } else if (event.key.toLowerCase() === 'f') {
        map.userAdjusted = false;
        fitMap();
      }
    });
  }

  function syncInitialSideUi() {
    document.body.dataset.side = state.side;
    updateSideControls();
    renderSpeedControls();
    updateLogMeta();
    updateHeader(null);
    updateOutcome(null);
    updateMapBrief(null);
    renderSidebar();
    dom.scenarioSelect.disabled = true;
    dom.btnStart.disabled = true;
  }

  async function init() {
    syncInitialSideUi();
    bindUi();
    setupMapInteractions();
    resizeMapCanvas();
    startRenderLoop();
    await bootstrap();
  }

  void init();
})();
