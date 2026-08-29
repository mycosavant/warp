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
  var CONTROL = '/v1/control';
  var PROTOCOL_VERSION = 1;

  // The three T11.5 actions. `APPROVALS` and `DENY` are pairable unconditionally;
  // `ALLOW` is on the list only when the machine's owner set
  // WARP_FORK_REMOTE_APPROVE, which is why nothing here hardcodes its presence.
  var APPROVALS = 'agent.approvals';
  var ALLOW = 'agent.approve';
  var DENY = 'agent.deny';

  // How long an armed Yes stays armed. Long enough to be a deliberate second
  // tap, short enough that an armed button left on screen disarms itself rather
  // than waiting to be pressed by a pocket.
  var ARM_MS = 4000;

  // **`localStorage` since T12.3, and the reason is structural rather than a
  // change of mind.** T12.1 chose `sessionStorage` — per tab, so a device token
  // never touches the disk of a phone that may not be only yours — and said it
  // should only change for a measured reason. Making the console installable is
  // that reason, and it is not a preference: a home-screen launch is a *new*
  // browsing context every cold start, so `sessionStorage` is empty by
  // definition. An installed app that demands a fresh QR scan on every launch is
  // not installed in any useful sense.
  //
  // What keeps the trade honest: the server bounds the token to twelve hours and
  // `loadDevice` refuses an expired one, a 401 clears it, and `unpair` in the
  // header lets a person end it from the device holding it.
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
    approvals: document.getElementById('approvals'),
    waitingCount: document.getElementById('waiting-count'),
    waitingNote: document.getElementById('waiting-note'),
    waitingError: document.getElementById('waiting-error'),
    agents: document.getElementById('agents'),
    agentsCount: document.getElementById('agents-count'),
    agentsNote: document.getElementById('agents-note'),
    events: document.getElementById('events'),
    eventsCount: document.getElementById('events-count'),
    eventsNote: document.getElementById('events-note'),
    unpair: document.getElementById('unpair')
  };

  var device = null;
  var credentials = {};
  var eventCount = 0;
  var approvalRefresh = null;

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
  //
  // **Two envelope shapes reach this page, and reading only one is how a useful
  // message becomes "HTTP 400".** `ErrorResponseEnvelope` — a refused route, a
  // bad bearer, an unpairable action — carries `error` at the top level.
  // `ResponseEnvelope` — a typed action that was accepted and then failed —
  // nests it under `response`. The second is the one carrying the sentences
  // worth reading, including the stale-digest refusal that names what to do
  // next, so a client that understands only the first swallows exactly the
  // errors this page exists to show.
  function describeFailure(response, body) {
    var detail = null;
    try {
      var parsed = JSON.parse(body) || {};
      var error = parsed.error || (parsed.response && parsed.response.error);
      if (error && error.message) {
        detail = error.message;
        if (error.details) detail += ' — ' + error.details;
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
      var raw = localStorage.getItem(DEVICE_KEY);
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
      localStorage.setItem(DEVICE_KEY, JSON.stringify(saved));
    } catch (_) {
      // A browser refusing storage is survivable: this tab keeps the token
      // in memory and only a reload has to re-pair.
    }
  }

  function forgetDevice(why) {
    device = null;
    credentials = {};
    try { localStorage.removeItem(DEVICE_KEY); } catch (_) { /* see saveDevice */ }
    // Nothing on screen is true any more, and leaving the last approval drawn
    // beside "not paired" would invite a tap that cannot land.
    clear(el.approvals);
    el.waitingCount.textContent = '0';
    el.waitingNote.textContent = '';
    showPairing(why);
  }

  function showPairing(why) {
    el.pairing.hidden = false;
    el.pairingNote.textContent = why;
    el.unpair.hidden = true;
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

  // ---------------------------------------------------------------- approvals

  // What this device may ask for, as the server told it at pairing time.
  //
  // `/v1/pair` returns the action list precisely so a client can "present a
  // truthful capability list rather than discovering the boundary one refusal at
  // a time" — T11.4's words, and exactly what is needed here. The list cannot go
  // stale underneath us: `pairable_actions` reads an environment variable, and a
  // process cannot change its own, so the answer is fixed for an instance's life
  // — and a restart drops the pairing map, forcing a fresh scan anyway.
  function can(action) {
    return !!(device && device.actions && device.actions.indexOf(action) >= 0);
  }

  // One typed action over `POST /v1/control`, the same envelope `warpctrl` sends.
  function control(action, params) {
    return credentialFor(action).then(function (token) {
      var body = JSON.stringify({
        protocol_version: PROTOCOL_VERSION,
        request_id: uuid4(),
        action: { kind: action, params: params || {} }
      });
      return fetch(CONTROL, {
        method: 'POST',
        headers: Object.assign({ 'content-type': 'application/json' }, authorized(token)),
        body: body
      }).then(function (response) {
        return response.text().then(function (raw) {
          if (!response.ok) throw new Error(describeFailure(response, raw));
          var envelope = JSON.parse(raw);
          if (envelope.response && envelope.response.status === 'error') {
            throw new Error(envelope.response.error.message || (action + ' failed'));
          }
          return envelope.response ? envelope.response.data : null;
        });
      });
    });
  }

  // A tap on Yes runs a command on the machine this page is watching, so it takes
  // two. Not a modal — a button that arms itself, says so, and disarms on its
  // own. The cost is one extra tap on the one action that can make something
  // happen; `No` stays a single tap, because saying no can only ever make less
  // happen (the same asymmetry that keeps `agent.deny` pairable and
  // `agent.approve` behind a variable).
  function armThenRun(button, label, run) {
    var timer = null;
    var disarm = function () {
      timer = null;
      button.className = 'allow';
      button.textContent = label;
    };
    button.addEventListener('click', function () {
      if (timer) {
        clearTimeout(timer);
        disarm();
        run();
        return;
      }
      button.className = 'allow armed';
      button.textContent = 'tap again to allow';
      timer = setTimeout(disarm, ARM_MS);
    });
  }

  // Why an answer's failure has its own line, found by running it: the refresh
  // that follows an answer re-renders the list, and the list's own note lives
  // there — so a shared line meant the reason an answer was refused was wiped
  // roughly a heartbeat after it appeared. The most important message this page
  // can show is "that yes did not land, and here is why".
  function answerFailed(message) {
    el.waitingError.hidden = false;
    el.waitingError.textContent = message;
  }

  function answerSucceeded() {
    el.waitingError.hidden = true;
    el.waitingError.textContent = '';
  }

  function answer(approval, action, buttons) {
    buttons.forEach(function (b) { b.disabled = true; });
    // `digest` is not optional and not decorative: it is what binds this answer
    // to the request that was on screen when it was read. The server refuses a
    // stale one rather than applying it to whatever the agent is asking now.
    control(action, { approval_id: approval.approval_id, digest: approval.digest })
      .then(answerSucceeded)
      .catch(function (err) { answerFailed(String(err.message || err)); })
      // Either way, including on failure: a refused answer usually means the
      // request moved, and the only useful next thing is what is true now.
      .then(refreshApprovals, refreshApprovals);
  }

  function approvalRow(approval) {
    var row = document.createElement('li');
    row.appendChild(text(
      'div',
      'ask',
      (approval.agent || '?') + ' ' +
        (approval.kind === 'question' ? 'is asking you' : 'wants permission')
    ));
    if (approval.summary) row.appendChild(text('div', 'title', approval.summary));
    if (approval.tool_name) {
      row.appendChild(text(
        'code',
        'cmd',
        approval.tool_name + (approval.tool_input ? ': ' + approval.tool_input : '')
      ));
    }
    var where = [approval.project, approval.cwd].filter(Boolean);
    if (where.length) row.appendChild(text('div', 'meta', where.join(' · ')));

    // Its own line, and never folded into the one above. `cwd` is where the
    // *session* is — for an ACP request that is a directory Warp chose — while
    // this is what the agent said *this call* touches, recovered by joining the
    // permission request to the tool-call stream. Joining the two strings would
    // present one as the other, and T14.6 measured that the directory a call
    // acts in is what decides whose permission rules were consulted at all.
    // Absent means the agent never said; nothing here fills it in from `cwd`.
    if (approval.acts_on && approval.acts_on.length) {
      row.appendChild(text('div', 'meta', 'acts on ' + approval.acts_on.join(', ')));
    }

    var answers = text('div', 'answers');
    var buttons = [];
    var deny = text('button', 'deny', 'No');
    buttons.push(deny);
    deny.addEventListener('click', function () { answer(approval, DENY, buttons); });

    // Two independent reasons there may be no Yes, and they are not the same
    // fact: `can(ALLOW)` is about this *device*, `approval.can_approve` is about
    // this *entry*. Drawing the button from the device alone was wrong — the
    // listing reports every blocked session while `agent.approve` refuses
    // unverified agents, so a phone with remote approve enabled showed a Yes on
    // rows the handler would always reject.
    if (can(ALLOW) && approval.can_approve) {
      var allow = text('button', 'allow', 'Yes');
      buttons.push(allow);
      armThenRun(allow, 'Yes', function () { answer(approval, ALLOW, buttons); });
      answers.appendChild(allow);
    }
    answers.appendChild(deny);
    row.appendChild(answers);

    // Said rather than hidden, because a person looking at a row with only a No
    // button needs to know whether that is a setting or a fault. The entry's own
    // reason wins when there is one: it is the more specific truth, and it is
    // the one that stays true after the device is granted approve.
    if (!approval.can_approve) {
      row.appendChild(text('div', 'meta', approval.approve_refused_because || 'This request cannot be approved from here.'));
    } else if (!can(ALLOW)) {
      row.appendChild(text(
        'div',
        'meta',
        'Yes does not travel to a paired device unless WARP_FORK_REMOTE_APPROVE is set on the machine.'
      ));
    }
    return row;
  }

  function renderApprovals(approvals) {
    clear(el.approvals);
    el.waitingCount.textContent = String(approvals.length);
    el.waitingCount.className = 'badge' + (approvals.length ? ' waiting' : '');
    approvals.forEach(function (approval) {
      el.approvals.appendChild(approvalRow(approval));
    });
    // Both branches assign, and the empty one is not the only one that has to.
    // Found by running it: setting the note only when the list was empty left
    // "nothing is waiting on you" printed above a request that was, which is the
    // one sentence this page must never get wrong.
    el.waitingNote.className = 'note';
    el.waitingNote.textContent = approvals.length ? '' : 'nothing is waiting on you.';
  }

  function refreshApprovals() {
    // The polls outlive an unpair, so they have to check. Without this, tapping
    // `unpair` replaces a working page with one printing "not paired" every five
    // seconds in red, which reads as a fault rather than as what was asked for.
    if (!device) return Promise.resolve();
    return control(APPROVALS)
      .then(function (data) { renderApprovals((data && data.approvals) || []); })
      .catch(function (err) {
        el.waitingNote.className = 'note bad';
        el.waitingNote.textContent = String(err.message || err);
      });
  }

  // Approvals are event-driven with a poll as a backstop. Any CLI-agent event can
  // change what is waiting — a `permission_request` creates one, a
  // `tool_complete` or a `stop` clears one — so rather than curate a list of
  // which events matter and be wrong about one, every event schedules a refresh
  // and the debounce absorbs a chatty agent.
  function scheduleApprovalRefresh() {
    if (approvalRefresh) return;
    approvalRefresh = setTimeout(function () {
      approvalRefresh = null;
      refreshApprovals();
    }, 300);
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
    if (!device) return Promise.resolve();
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
    scheduleApprovalRefresh();
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
    el.unpair.hidden = false;
    refreshApprovals();
    refreshState();
    // The backstop, not the mechanism — `scheduleApprovalRefresh` on every event
    // is what makes an approval appear promptly. This covers a stream that
    // lagged or a frame that never arrived.
    setInterval(refreshApprovals, 5000);
    setInterval(refreshState, 5000);
    keepStreaming();
  }

  function boot() {
    tickClock();
    setInterval(tickClock, 30000);
    el.unpair.addEventListener('click', function () {
      forgetDevice('unpaired on this device. Run `warpctrl pair show` and scan again to come back.');
    });

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
