// The console's script (T12.1). Served as its own route so the page can be
// delivered under `script-src 'self'` — see `console.rs` for why that matters.
//
// Three rules this file is written to, and each of them is load-bearing:
//
//   1. Nothing is ever assigned to `innerHTML`. Every value drawn here was
//      authored by an agent, a tool, or a file an agent read, and `textContent`
//      is the only escaping that cannot be got wrong by accident.
//   2. No secret is ever put in a URL. The pairing code arrives in the fragment
//      (which no browser sends to a server), is spent immediately, and is wiped
//      from the address bar before the first render.
//   3. Server errors are shown, not swallowed. A console that renders an empty
//      list when the truth is "403, wrong Host header" is the silent failure
//      this whole phase exists to detect.

'use strict';

(function () {
  var STATE = '/v1/state';
  var EVENTS = '/v1/events';
  var PAIR = '/v1/pair';
  var CREDENTIAL = '/v1/pair/credential';
  var PROTOCOL_VERSION = 1;

  // Per-tab, deliberately. A device token is a bearer secret for a control
  // plane and lives twelve hours; `localStorage` would put it on the disk of a
  // phone that may not be only yours, to save re-scanning a QR that takes two
  // seconds to display. If that trade turns out to be wrong in daily use, it
  // should be changed for that measured reason and not for this guessed one.
  var DEVICE_KEY = 'warp.console.device';

  // How long before a credential expires we replace it. The server issues five
  // minutes; a minute of margin covers a phone that slept mid-request.
  var REFRESH_MARGIN_MS = 60 * 1000;

  var MAX_EVENT_ROWS = 300;

  var el = {
    link: document.getElementById('link'),
    clock: document.getElementById('clock'),
    pairing: document.getElementById('pairing'),
    pairingNote: document.getElementById('pairing-note'),
    agents: document.getElementById('agents'),
    agentsCount: document.getElementById('agents-count'),
    agentsNote: document.getElementById('agents-note'),
    events: document.getElementById('events'),
    eventsCount: document.getElementById('events-count'),
    eventsNote: document.getElementById('events-note')
  };

  var device = null;
  var credentials = {};
  var eventCount = 0;

  // ---------------------------------------------------------------- utilities

  // `crypto.randomUUID` is restricted to secure contexts, and this page is
  // plain HTTP on a LAN address — so on the one device this console exists for,
  // it is undefined. `getRandomValues` has no such restriction. The server
  // parses `request_id` as a UUID, so the shape is not cosmetic.
  function uuid4() {
    var b = new Uint8Array(16);
    crypto.getRandomValues(b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    var hex = [];
    for (var i = 0; i < 16; i++) hex.push((b[i] + 0x100).toString(16).slice(1));
    return hex.slice(0, 4).join('') + '-' + hex.slice(4, 6).join('') + '-' +
      hex.slice(6, 8).join('') + '-' + hex.slice(8, 10).join('') + '-' +
      hex.slice(10, 16).join('');
  }

  function text(tag, className, value) {
    var node = document.createElement(tag);
    if (className) node.className = className;
    if (value !== undefined && value !== null) node.textContent = String(value);
    return node;
  }

  function clear(node) {
    while (node.firstChild) node.removeChild(node.firstChild);
  }

  function badge(node, label, kind) {
    node.textContent = label;
    node.className = 'badge' + (kind ? ' ' + kind : '');
  }

  function shortTime(iso) {
    if (!iso) return '';
    var t = String(iso);
    var i = t.indexOf('T');
    return i < 0 ? t.slice(0, 8) : t.slice(i + 1, i + 9);
  }

  // What the server said went wrong, preferred over what the browser inferred.
  // Every error route here answers with an `ErrorResponseEnvelope`.
  function describeFailure(response, body) {
    var detail = null;
    try {
      var parsed = JSON.parse(body);
      if (parsed && parsed.error && parsed.error.message) {
        detail = parsed.error.message;
        if (parsed.error.details) detail += ' — ' + parsed.error.details;
      }
    } catch (_) { /* not JSON; the status is all there is */ }
    return 'HTTP ' + response.status + (detail ? ': ' + detail : '');
  }

  function authorized(token) {
    return { authorization: 'Bearer ' + token };
  }

  // ------------------------------------------------------------------ pairing

  function loadDevice() {
    try {
      var raw = sessionStorage.getItem(DEVICE_KEY);
      if (!raw) return null;
      var saved = JSON.parse(raw);
      if (!saved || !saved.device_token) return null;
      if (saved.expires_at && Date.parse(saved.expires_at) <= Date.now()) return null;
      return saved;
    } catch (_) {
      return null;
    }
  }

  function saveDevice(saved) {
    device = saved;
    try {
      sessionStorage.setItem(DEVICE_KEY, JSON.stringify(saved));
    } catch (_) {
      // A browser refusing storage is survivable: this tab keeps the token in
      // memory and only a reload has to re-pair.
    }
  }

  function forgetDevice(why) {
    device = null;
    credentials = {};
    try { sessionStorage.removeItem(DEVICE_KEY); } catch (_) { /* see saveDevice */ }
    showPairing(why);
  }

  function showPairing(why) {
    el.pairing.hidden = false;
    el.pairingNote.textContent = why;
    badge(el.link, 'not paired', 'warn');
  }

  // Redeems a pairing code for a device token. The code is spent whether or not
  // this succeeds, which is why it is read once and never retried.
  function redeem(code) {
    return fetch(PAIR, { method: 'POST', headers: authorized(code) })
      .then(function (response) {
        return response.text().then(function (body) {
          if (!response.ok) throw new Error(describeFailure(response, body));
          return JSON.parse(body);
        });
      })
      .then(function (paired) {
        saveDevice(paired);
        el.pairing.hidden = true;
        return paired;
      });
  }

  // One short-lived, action-scoped credential, cached until it is nearly stale.
  // Minted per action because that is the unit the server grants: a page with
  // one credential for everything is a page asking for authority it does not
  // need, and `pairable_actions` would refuse it anyway.
  function credentialFor(action) {
    var held = credentials[action];
    if (held && held.expires - REFRESH_MARGIN_MS > Date.now()) {
      return Promise.resolve(held.token);
    }
    if (!device) return Promise.reject(new Error('not paired'));
    var body = JSON.stringify({
      protocol_version: PROTOCOL_VERSION,
      request_id: uuid4(),
      action: action
    });
    return fetch(CREDENTIAL, {
      method: 'POST',
      headers: Object.assign({ 'content-type': 'application/json' }, authorized(device.device_token)),
      body: body
    }).then(function (response) {
      return response.text().then(function (raw) {
        if (response.status === 401) {
          forgetDevice('the pairing expired or this instance restarted — pair again.');
          throw new Error('device token rejected');
        }
        if (!response.ok) throw new Error(describeFailure(response, raw));
        var issued = JSON.parse(raw);
        credentials[action] = {
          token: issued.bearer_token,
          expires: Date.parse(issued.grant.expires_at)
        };
        return issued.bearer_token;
      });
    });
  }

  // ------------------------------------------------------------------- agents

  function statusKind(status) {
    if (status === 'in_progress') return 'busy';
    if (status === 'blocked') return 'block';
    if (status === 'success') return 'ok';
    return '';
  }

  function renderAgents(conversations) {
    clear(el.agents);
    el.agentsCount.textContent = String(conversations.length);
    conversations.forEach(function (c) {
      var row = document.createElement('li');
      var title = text('div', 'title');
      title.appendChild(text('span', 'dot ' + statusKind(c.status)));
      title.appendChild(document.createTextNode(c.title || '(untitled)'));
      row.appendChild(title);
      var meta = [c.status];
      if (c.blocked_action) meta.push('on ' + c.blocked_action);
      if (c.settled) meta.push('settled');
      if (c.is_hidden) meta.push('hidden');
      if (c.pane_id) meta.push('pane ' + c.pane_id);
      row.appendChild(text('div', 'meta', meta.join(' · ')));
      el.agents.appendChild(row);
    });
    // Recorded on screen rather than only in a doc, because it is the first
    // thing a person will misread. `/v1/state` is `agent.list`, which reports
    // Warp's *own* conversations; a `claude` running in a pane has none, so an
    // empty list here does not mean nothing is running. T11.5 found this the
    // hard way and `agent.approvals` is the half that sees the rest (T12.2).
    el.agentsNote.className = 'note';
    el.agentsNote.textContent = conversations.length
      ? ''
      : 'none — note this counts Warp’s own agent threads, not CLI agents running in panes.';
  }

  function refreshState() {
    return credentialFor('agent.list')
      .then(function (token) {
        return fetch(STATE, { headers: authorized(token) }).then(function (response) {
          return response.text().then(function (body) {
            if (!response.ok) throw new Error(describeFailure(response, body));
            return JSON.parse(body);
          });
        });
      })
      .then(function (envelope) {
        if (envelope.response && envelope.response.status === 'error') {
          throw new Error(envelope.response.error.message || 'agent.list failed');
        }
        var data = envelope.response ? envelope.response.data : null;
        renderAgents((data && data.conversations) || []);
      })
      .catch(function (err) {
        el.agentsNote.className = 'note bad';
        el.agentsNote.textContent = String(err.message || err);
      });
  }

  // ------------------------------------------------------------------- events

  function renderEvent(record) {
    var row = document.createElement('li');
    if (record.event === 'permission_request' || record.event === 'question_asked') {
      row.className = 'blocked';
    }
    row.appendChild(text('span', 't', shortTime(record.ts)));
    var body = document.createElement('span');
    body.appendChild(text('span', 'k', (record.agent || '?') + ' ' + (record.event || '?')));
    var detail = record.summary || record.tool_input_preview || record.tool_name || '';
    if (record.applied === false) detail = (detail ? detail + ' ' : '') + '(dropped)';
    if (detail) {
      body.appendChild(document.createTextNode(' '));
      body.appendChild(text('span', 'd', detail));
    }
    row.appendChild(body);
    el.events.insertBefore(row, el.events.firstChild);
    while (el.events.childNodes.length > MAX_EVENT_ROWS) {
      el.events.removeChild(el.events.lastChild);
    }
    eventCount += 1;
    el.eventsCount.textContent = String(eventCount);
    // Clears a previous complaint as well as its text: a "missed 40 events"
    // warning that stayed red after the stream recovered would keep reporting a
    // gap that had closed.
    el.eventsNote.className = 'note';
    el.eventsNote.textContent = '';
  }

  // One SSE frame, as the wire delivers it: `event:` and `data:` lines, with a
  // `data:` repeated for each line of a multi-line payload.
  function handleFrame(frame) {
    var kind = 'message';
    var data = [];
    frame.split('\n').forEach(function (line) {
      if (line.charAt(0) === ':') return; // keepalive comment
      if (line.indexOf('event:') === 0) kind = line.slice(6).trim();
      else if (line.indexOf('data:') === 0) data.push(line.slice(5).replace(/^ /, ''));
    });
    var payload = data.join('\n');
    if (kind === 'expired') return; // the reconnect loop already handles this
    if (kind === 'lagged') {
      el.eventsNote.className = 'note bad';
      el.eventsNote.textContent = 'missed ' + payload + ' events — the stream fell behind.';
      return;
    }
    if (!payload) return;
    try {
      renderEvent(JSON.parse(payload));
    } catch (_) {
      renderEvent({ event: 'unparsed', summary: payload });
    }
  }

  // Streamed with `fetch` rather than `EventSource`, because `EventSource`
  // cannot set an `Authorization` header — and the alternative it pushes people
  // towards is a token in the query string, which is the one place a secret is
  // guaranteed to be written down by something else.
  function streamEvents() {
    return credentialFor('events.subscribe')
      .then(function (token) {
        return fetch(EVENTS, { headers: authorized(token) });
      })
      .then(function (response) {
        if (!response.ok) {
          return response.text().then(function (body) {
            throw new Error(describeFailure(response, body));
          });
        }
        badge(el.link, 'live', 'live');
        var reader = response.body.getReader();
        var decoder = new TextDecoder();
        var buffer = '';
        function pump() {
          return reader.read().then(function (chunk) {
            if (chunk.done) return;
            buffer += decoder.decode(chunk.value, { stream: true });
            var split;
            while ((split = buffer.indexOf('\n\n')) >= 0) {
              handleFrame(buffer.slice(0, split));
              buffer = buffer.slice(split + 2);
            }
            return pump();
          });
        }
        return pump();
      });
  }

  // The stream ends every time the credential does — five minutes, by design,
  // so a connection cannot outlive its own authority. Reconnecting is therefore
  // the normal case and not an error path.
  function keepStreaming() {
    if (!device) return;
    streamEvents()
      .then(function () {
        badge(el.link, 'reconnecting', 'warn');
        setTimeout(keepStreaming, 250);
      })
      .catch(function (err) {
        badge(el.link, 'offline', 'bad');
        el.eventsNote.className = 'note bad';
        el.eventsNote.textContent = String(err.message || err);
        setTimeout(keepStreaming, 3000);
      });
  }

  // --------------------------------------------------------------------- boot

  function tickClock() {
    var now = new Date();
    var pad = function (n) { return (n < 10 ? '0' : '') + n; };
    el.clock.textContent = pad(now.getHours()) + ':' + pad(now.getMinutes());
  }

  function start() {
    badge(el.link, 'connecting');
    el.pairing.hidden = true;
    refreshState();
    setInterval(refreshState, 5000);
    keepStreaming();
  }

  function boot() {
    tickClock();
    setInterval(tickClock, 30000);

    // Read the fragment once and erase it before anything can render, so the
    // code is never on screen, in history, or in a `Referer`.
    var code = location.hash.replace(/^#/, '');
    if (code) history.replaceState(null, '', location.pathname);

    device = loadDevice();
    if (device) {
      start();
      return;
    }
    if (!code) {
      showPairing('run `warpctrl pair show` on the machine running Warp, then scan the QR it prints.');
      return;
    }
    redeem(code).then(start).catch(function (err) {
      showPairing('pairing failed: ' + String(err.message || err) +
        ' — codes last two minutes and are spent on first use.');
    });
  }

  boot();
})();
