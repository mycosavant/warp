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
  var WORKER = '/sw.js';
  var CREDENTIAL = '/v1/pair/credential';
  var CONTROL = '/v1/control';
  var PROTOCOL_VERSION = 1;

  // The three T11.5 actions. `APPROVALS` and `DENY` are pairable unconditionally;
  // `ALLOW` is on the list only when the machine's owner set
  // WARP_FORK_REMOTE_APPROVE, which is why nothing here hardcodes its presence.
  var APPROVALS = 'agent.approvals';
  var ALLOW = 'agent.approve';
  var DENY = 'agent.deny';
  // Stop, not kill. Pairable on the same argument as DENY and one tap for the
  // same reason: it can only prevent what was proposed, never destroy what
  // exists. See `pairing.rs` for the full argument and its honest delta.
  var CANCEL = 'agent.cancel';
  // The record of one conversation (board item 6, phase 3): the agent's own
  // session file joined with Warp's event log, the same merge `warpctrl agent
  // trace` prints, run inside the instance because this device has neither
  // file. Pairable on the argument in `pairing.rs`: a read, wider than the
  // event stream, asked for so a phone can answer "what is it doing".
  var TRACE = 'agent.trace';
  // What a device paired by `/remote-control` holds and a watching device does
  // not: a prompt to the one conversation it was handed. The server confines
  // every credential such a device mints to that conversation, so the box is
  // drawn from the action list the same way Yes is, and cannot reach another
  // conversation whatever the page believes is open.
  var PROMPT = 'agent.prompt';

  // How often the conversation view asks for the tail of the record. Fast
  // while the turn runs, because that is what watching means; slow once it
  // has stopped, because nothing changes but a late compaction. Any event
  // for the open conversation also schedules one, so these are the floor.
  var TRACE_BUSY_MS = 2000;
  var TRACE_IDLE_MS = 10000;

  // How long an armed Yes stays armed. Long enough to be a deliberate second
  // tap, short enough that an armed button left on screen disarms itself rather
  // than waiting to be pressed by a pocket.
  var ARM_MS = 4000;

  // What `cwd` is, per population, because the field means two different things.
  //
  // A pane entry's is the agent's own working directory, reported over OSC. An
  // ACP entry's is the directory *Warp* chose for the session and sent in
  // `session/new` — which is not necessarily where the call acts, and which
  // T14.6 measured deciding whether the user's own permission rules were loaded
  // at all. The population comes from `source`, which the server states
  // first-hand; a key that is missing here draws no label at all rather than
  // guess, which is what keeps a future third population from being described
  // as something it is not.
  var CWD_LABELS = { acp: 'session directory ', pane: 'working directory ' };

  // How many controls are waiting for their second tap.
  //
  // **Found by tapping Yes in a real browser (T14.6).** `renderApprovals` calls
  // `clear()` and rebuilds every row, and it runs on a 5s poll as well as on any
  // agent event — so a refresh landing inside the 4s arm window destroyed the
  // armed button and drew a fresh `Yes` in its place. The tap that was meant to
  // confirm then *armed* the new button instead, and the person's answer simply
  // did not happen. Two fast taps worked; one tap and a pause did not, which is
  // the worst shape for a bug to have because it looks like an unreliable
  // feature rather than a broken one — the T14.2 lesson exactly.
  //
  // It fails safe rather than dangerous: a discarded arm can only ever lose a
  // yes, never invent one. That is why this is a counter and not a lock.
  var armedControls = 0;

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
    pairingTrust: document.getElementById('pairing-trust'),
    pairingInstall: document.getElementById('pairing-install'),
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
    unpair: document.getElementById('unpair'),
    notify: document.getElementById('notify'),
    back: document.getElementById('back'),
    main: document.querySelector('main'),
    home: Array.prototype.slice.call(document.querySelectorAll('section.home')),
    conversation: document.getElementById('conversation'),
    convTitle: document.getElementById('conv-title'),
    convMeta: document.getElementById('conv-meta'),
    convApprovals: document.getElementById('conv-approvals'),
    convControls: document.getElementById('conv-controls'),
    convNote: document.getElementById('conv-note'),
    convError: document.getElementById('conv-error'),
    trace: document.getElementById('trace'),
    traceFoot: document.getElementById('trace-foot'),
    promptForm: document.getElementById('conv-prompt'),
    promptText: document.getElementById('prompt-text'),
    promptSend: document.getElementById('prompt-send'),
    promptError: document.getElementById('prompt-error')
  };

  var device = null;
  var credentials = {};
  var eventCount = 0;
  var approvalRefresh = null;
  // What the last two polls said, kept so the conversation view can be drawn
  // from them when it opens rather than waiting for the next poll.
  var lastApprovals = [];
  var lastConversations = [];
  // The conversation view's state, or null on the home page. See `openConversation`.
  // Named `viewing` rather than `open` so it never reads as `window.open`,
  // which this file must not call.
  var viewing = null;

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
    // Read off the browser, not inferred from the URL: a secure context is
    // the browser's decision, and it is what decides whether this page can
    // notify at all (item 5). Over http:// at a LAN address it is false.
    el.pairingTrust.textContent = window.isSecureContext
      ? 'secure context: yes — this browser trusts the console\'s certificate, and notifications can work.'
      : 'secure context: no — the browser does not trust this address, so it will not notify you. Install the certificate below and open the https:// address.';
    el.pairingInstall.hidden = !!window.isSecureContext;
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
  function control(action, params, retried) {
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
          // A credential the page still holds and the server no longer
          // does: *Stop sharing* purged it, or the clocks disagree about its
          // expiry. Either way the answer is to mint again, once — and the
          // mint is where a cut-off device learns it was cut off
          // (`credentialFor` forgets it on 401). Measured on the emulator:
          // without this, a stopped page kept its cached credentials and
          // read `live` until they aged out.
          if (response.status === 401 && !retried) {
            delete credentials[action];
            return control(action, params, true);
          }
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
      if (timer) armedControls -= 1;
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
      // Counted, because the list refresh rebuilds every row from scratch and
      // would otherwise throw this button away mid-arm. See `armedControls`.
      armedControls += 1;
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
    // **`cwd` means two different things and must not be drawn as one.** For a
    // pane it is the agent's own working directory, reported over OSC. For an
    // ACP request it is the directory *Warp* chose for the session and sent in
    // `session/new` — which is not necessarily where the call acts, and which
    // T14.6 measured deciding whether the user's own permission rules loaded at
    // all. An unlabelled path directly under a command reads as "where this
    // runs", and this row now grows a Yes button, so the vagueness stopped being
    // cosmetic. The population is read from `source`, which the server states,
    // rather than guessed from `tab_id` or from the shape of the id.
    if (approval.project) row.appendChild(text('div', 'meta', approval.project));
    if (approval.cwd) {
      // An unknown `source` gets the bare path and no claim about what kind of
      // directory it is. The first draft was a two-way ternary, which would have
      // labelled a third population "working directory" — confidently and
      // wrongly. Saying less is the only safe direction here, because the whole
      // point of the label is that the two mean different things.
      var label = CWD_LABELS[approval.source];
      row.appendChild(text('div', 'meta', label ? label + approval.cwd : approval.cwd));
    }

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
    } else if (approval.approve_selects) {
      // What a Yes actually sends, and how far it reaches. The scope sentence is
      // the load-bearing half: Warp only ever selects a single-shot allow, so a
      // yes here cannot widen the session's policy — and a person tapping a
      // button on a phone has no other way to know that.
      row.appendChild(text(
        'div',
        'meta',
        'Yes selects "' + approval.approve_selects + '" — this call only, nothing after it.'
      ));
    }
    if (approval.can_approve && !can(ALLOW)) {
      row.appendChild(text(
        'div',
        'meta',
        'Yes does not travel to a paired device unless WARP_FORK_REMOTE_APPROVE is set on the machine.'
      ));
    }
    return row;
  }

  function renderApprovals(approvals) {
    lastApprovals = approvals;
    renderConversationApprovals();
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
    // **Never redraw the list out from under a half-given answer.** Rebuilding
    // the rows destroys an armed button, so a refresh here discarded the arm and
    // the confirming tap landed on a fresh `Yes`. Deferring is bounded by
    // `ARM_MS`, and a list that is at most four seconds stale is not a hazard:
    // every answer still carries the digest of what was shown, so an entry that
    // moved in the meantime is refused by the server rather than mis-answered.
    if (armedControls > 0) return Promise.resolve();
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
    lastConversations = conversations;
    if (viewing) {
      conversations.forEach(function (c) {
        if (c.conversation_id === viewing.id) {
          viewing.summary = c;
          renderConversationHead();
        }
      });
    }
    clear(el.agents);
    el.agentsCount.textContent = String(conversations.length);
    conversations.forEach(function (c) {
      var row = document.createElement('li');
      // Tapping the row opens the conversation's record. Only when this
      // device may read it: a row that opens onto a refusal teaches that the
      // feature is unreliable rather than that it is off.
      if (can(TRACE)) {
        row.className = 'openable';
        row.addEventListener('click', function () { openConversation(c); });
      }
      var title = text('div', 'title');
      title.appendChild(text('span', 'dot ' + statusKind(c.status)));
      title.appendChild(document.createTextNode(c.title || '(untitled)'));
      row.appendChild(title);
      var meta = [c.status];
      if (c.blocked_action) meta.push('on ' + c.blocked_action);
      if (c.settled) meta.push('settled');
      if (c.is_hidden) meta.push('hidden');
      if (c.pane_id) meta.push('pane ' + c.pane_id);
      // The wedge signal, drawn next to the only control that answers it. It is
      // a symptom and not a verdict — a long compile and a dead agent look
      // identical from here — so it is shown and never acted on automatically.
      if (typeof c.quiet_for_seconds === 'number') {
        meta.push('quiet ' + c.quiet_for_seconds + 's');
      }
      row.appendChild(text('div', 'meta', meta.join(' · ')));

      // Two facts again, not one: `can(CANCEL)` is about this *device*,
      // `c.is_busy` is about this *entry*. `is_busy` is true for `in_progress`
      // alone, so the button appears only where there is a turn to stop —
      // drawing it from the device would offer to cancel finished work.
      if (can(CANCEL) && c.is_busy) {
        var stopRow = text('div', 'answers');
        var stop = text('button', 'deny', 'Stop');
        stop.addEventListener('click', function (click) {
          // The row underneath opens the conversation; a Stop is not that.
          click.stopPropagation();
          stop.disabled = true;
          control(CANCEL, { conversation_id: c.conversation_id })
            .then(answerSucceeded)
            .catch(function (err) { answerFailed(String(err.message || err)); })
            .then(refreshState, refreshState);
        });
        stopRow.appendChild(stop);
        row.appendChild(stopRow);
        // Said plainly, because the button cannot do the other half. Restarting
        // needs `agent.prompt`, which causes effects and so is never pairable.
        row.appendChild(text(
          'div',
          'meta',
          'Stopping keeps the conversation — picking the work back up happens at the machine.'
        ));
      }
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
    // `session_id` on a Warp event is Warp's conversation id, which is what
    // the view is keyed by. The harness writes its line before Warp hears of
    // the call, so by the time this fires the tail is already on disk.
    if (viewing && record.session_id === viewing.id) scheduleTracePoll(300);
    maybeNotify(record);
  }

  // ---------------------------------------------------------- being told
  //
  // Foreground only (the second decision of 2026-09-06). While the page is
  // open, installed or in a background tab of an awake phone, a permission
  // request or the end of a turn on the conversation this device is on posts
  // a notification and buzzes. Nothing server-side and nothing leaves the
  // machine: the events stream already carries both, and the Notification API
  // talks to the phone's own shade. It is withheld outside a secure context,
  // which is why the wide listener speaks TLS. A locked phone needs Web Push,
  // a nudge through Google's or Apple's relay, and that is not built.

  function notificationsPossible() {
    return !!(window.isSecureContext && typeof Notification === 'function');
  }

  function syncNotifyButton() {
    el.notify.hidden = !(notificationsPossible() && Notification.permission === 'default');
  }

  // Asked from a tap, never on load: a permission prompt that appears unasked
  // is the one browsers have learned to bury.
  // Chrome on Android has no `Notification` constructor: a page's notification
  // is shown only through a service worker registration's `showNotification`.
  // Measured on the emulator on 2026-09-06 — the constructor path posted
  // nothing and its catch said nothing. So a worker is registered the moment
  // the person asks to be told, and at start when they already have; it
  // handles no fetch and no push (`/sw.js`), it is a place for
  // `showNotification` to live and a tap-to-focus handler. Resolves to the
  // registration, or to null where there is none to be had.
  function registerNotifier() {
    if (!notificationsPossible() || !('serviceWorker' in navigator)) return Promise.resolve(null);
    return navigator.serviceWorker.register(WORKER)
      .then(function () { return navigator.serviceWorker.ready; })
      .catch(function () { return null; });
  }
  function askToNotify() {
    if (!notificationsPossible()) return;
    Notification.requestPermission().then(function () {
      syncNotifyButton();
      if (Notification.permission === 'granted') registerNotifier();
    }, syncNotifyButton);
  }
  // The header is the one place a phone reads, so a notification that could
  // not be posted says so there rather than in a console nobody opens.
  function noteNotifyFailure(err) {
    text(el.notify, 'notify failed: ' + String((err && err.message) || err || 'unknown').slice(0, 80));
    el.notify.hidden = false;
  }

  function conversationOfInterest() {
    if (viewing) return viewing.id;
    if (device && device.conversation_id) return device.conversation_id;
    return null;
  }

  function maybeNotify(record) {
    if (!notificationsPossible() || Notification.permission !== 'granted') return;
    if (!document.hidden) return;
    if (!record || record.session_id !== conversationOfInterest()) return;
    var body = null;
    if (record.event === 'permission_request') {
      // The summary opens with the approval's id, which is for the answer
      // and not for a glance at the shade; what follows the separator is the
      // call. Measured on the emulator: a body of `asks: approval b865…:0 ·
      // Write /tmp/…` spent its two lines on the id.
      var ask = String(record.summary || '');
      var cut = ask.indexOf(' · ');
      if (/^approval /.test(ask) && cut > 0) ask = ask.slice(cut + 3);
      body = 'asks: ' + (ask || record.tool_input_preview || record.tool_name || 'a permission');
    } else if (record.event === 'stop') {
      body = 'the turn ended';
    } else if (record.event === 'stop_failure') {
      body = 'the turn ended: ' + (record.error_type || 'error');
    }
    if (!body) return;
    var title = (viewing && viewing.summary && viewing.summary.title) || 'warp';
    // `tag` collapses repeats of the same event on the same conversation into
    // one entry in the shade rather than a stack; `renotify` makes the second
    // ask buzz anyway, since a replaced entry is otherwise silent.
    var options = { body: body, tag: record.session_id + ':' + record.event, renotify: true, vibrate: [120, 60, 120] };
    if (navigator.vibrate) navigator.vibrate([120, 60, 120]);
    var viaWorker = ('serviceWorker' in navigator)
      ? navigator.serviceWorker.getRegistration().then(function (reg) {
          if (!reg || !reg.active) throw new Error('no worker');
          return reg.showNotification(title, options);
        })
      : Promise.reject(new Error('no serviceWorker'));
    viaWorker.catch(function () {
      // A browser with no worker to route through gets the constructor,
      // which desktop browsers still honour. One that refuses both (iOS
      // outside an installed page does) is told about it in the header.
      try {
        var shown = new Notification(title, options);
        shown.addEventListener('click', function () { window.focus(); shown.close(); });
      } catch (err) {
        noteNotifyFailure(err);
      }
    });
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
          // A cached stream credential the server no longer holds: drop it,
          // so the reconnect mints, and the mint is what tells a cut-off
          // device it was cut off. See `control` for the same rule.
          if (response.status === 401) delete credentials['events.subscribe'];
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

  // ------------------------------------------------------------- conversation
  //
  // One conversation's record, live (board item 6, phase 3). The rows come
  // from `agent.trace`, which returns the tail of both files after the line
  // counts of the last call; the page keeps every row it has been sent, orders
  // them by time and redraws. The folding rules are `trace_render.rs`'s: a
  // tool call is one row built from up to six lines across the two files, a
  // compaction is a rule with the summary folded under it, usage is the
  // footer. Redrawing from scratch on every poll is cheaper than it sounds --
  // a long session is a few hundred rows -- and it is what keeps the ordering
  // right when a tail from one file lands before rows already drawn from the
  // other, which the two clocks guarantee will happen.

  function openConversation(c) {
    if (viewing && viewing.id === c.conversation_id) return;
    viewing = {
      id: c.conversation_id,
      summary: c,
      warpAfter: 0,
      harnessAfter: 0,
      rows: [],
      header: null,
      timer: null,
      inflight: false
    };
    // A history entry rather than a hash: the fragment is where a pairing code
    // arrives, and a reload landing on `#something` would try to spend it.
    history.pushState({ conversation: c.conversation_id }, '', location.pathname);
    el.home.forEach(function (section) { section.hidden = true; });
    el.conversation.hidden = false;
    // A device handed one conversation has nowhere to go back to: the home
    // page would list that conversation alone, because the server filters
    // everything it reads to it.
    el.back.hidden = !!(device && device.conversation_id);
    el.promptForm.hidden = !can(PROMPT);
    el.promptError.hidden = true;
    clear(el.trace);
    el.traceFoot.textContent = '';
    el.convError.hidden = true;
    el.convNote.className = 'note';
    el.convNote.textContent = 'reading the record…';
    renderConversationHead();
    renderConversationApprovals();
    pollTrace();
  }

  // The one place a prompt is composed: the text, and the conversation the
  // view is on. Sent through the same credential path as every other action,
  // where the server checks the conversation against the grant.
  function sendPrompt() {
    if (!viewing) return;
    var mine = viewing;
    var prompt = el.promptText.value.trim();
    if (!prompt) return;
    el.promptSend.disabled = true;
    el.promptError.hidden = true;
    control(PROMPT, { prompt: prompt, conversation_id: mine.id })
      .then(function () {
        if (viewing !== mine) return;
        el.promptText.value = '';
        scheduleTracePoll(300);
        refreshState();
      })
      .catch(function (err) {
        if (viewing !== mine) return;
        el.promptError.hidden = false;
        el.promptError.textContent = String(err.message || err);
      })
      .then(function () { el.promptSend.disabled = false; });
  }

  function closeConversation() {
    if (!viewing) return;
    if (viewing.timer) clearTimeout(viewing.timer);
    viewing = null;
    el.conversation.hidden = true;
    el.back.hidden = true;
    el.home.forEach(function (section) { section.hidden = false; });
  }

  // What the record says about the session, for the header (2026-09-06):
  // the mode and model in force, from the latest `session_mode` and
  // `session_model` lines, and the directory, from any Warp line's `cwd`.
  // "Is the agent asking me or its classifier" is the first thing a phone
  // wants to know, and a zero on permission requests means Warp was not in
  // the loop, never that nothing was decided.
  // The mode and model lines are the agent's whole disclosure ("current
  // `auto`; offered `default` (…)"), which the record keeps whole; the
  // header wants the current id, so the backticked value after "current" is
  // taken when there is one. Photographed whole first
  // (`.fork/runs/page-2026-09-06/page-head.png`), trimmed after.
  function currentOf(text) {
    var m = /current (`[^`]*`)/.exec(text);
    return m ? m[1] : text;
  }

  function recordFacts(rows) {
    var facts = {};
    rows.forEach(function (row) {
      if (row.from !== 'warp' || !row.raw) return;
      if (row.kind === 'session_mode' && row.text) facts.mode = currentOf(row.text);
      if (row.kind === 'session_model' && row.text) facts.model = currentOf(row.text);
      if (row.kind === 'session_agent' && row.text) facts.agent = row.text;
      if (row.raw.cwd && !facts.cwd) facts.cwd = String(row.raw.cwd);
    });
    return facts;
  }

  function renderConversationHead() {
    if (!viewing) return;
    var c = viewing.summary;
    var facts = viewing.facts || {};
    clear(el.convTitle);
    el.convTitle.appendChild(text('span', 'dot ' + statusKind(c.status)));
    el.convTitle.appendChild(document.createTextNode(c.title || '(untitled)'));
    var meta = [c.status];
    if (c.blocked_action) meta.push('on ' + c.blocked_action);
    if (typeof c.quiet_for_seconds === 'number') meta.push('quiet ' + c.quiet_for_seconds + 's');
    if (facts.mode || c.session_mode) meta.push('mode ' + (facts.mode || c.session_mode));
    if (facts.model) meta.push('model ' + facts.model);
    if (facts.agent) meta.push('agent ' + facts.agent);
    if (facts.cwd) meta.push('in ' + facts.cwd);
    meta.push(c.conversation_id);
    el.convMeta.textContent = meta.join(' · ');
    clear(el.convControls);
    if (can(CANCEL) && c.is_busy) {
      var stop = text('button', 'deny', 'Stop');
      stop.addEventListener('click', function () {
        stop.disabled = true;
        control(CANCEL, { conversation_id: c.conversation_id })
          .then(answerSucceeded)
          .catch(function (err) { answerFailed(String(err.message || err)); })
          .then(refreshState, refreshState);
      });
      el.convControls.appendChild(stop);
    }
  }

  // The approvals that belong to the open conversation, drawn with the same
  // rows as the home page so an answer here is the same answer. `conversation_id`
  // is set by the server for the ACP population; a pane agent has none and
  // stays on the home page, because a pane is not a conversation.
  function renderConversationApprovals() {
    if (!viewing) return;
    clear(el.convApprovals);
    lastApprovals.forEach(function (approval) {
      if (approval.conversation_id === viewing.id) {
        el.convApprovals.appendChild(approvalRow(approval));
      }
    });
  }

  function scheduleTracePoll(delay) {
    if (!viewing) return;
    if (viewing.timer) clearTimeout(viewing.timer);
    viewing.timer = setTimeout(function () {
      if (viewing) viewing.timer = null;
      pollTrace();
    }, delay);
  }

  function pollTrace() {
    if (!viewing || viewing.inflight) return;
    var mine = viewing;
    mine.inflight = true;
    control(TRACE, { conversation_id: mine.id, warp_after: mine.warpAfter, harness_after: mine.harnessAfter })
      .then(function (data) {
        if (viewing !== mine) return;
        mine.inflight = false;
        var header = (data && data.header) || {};
        mine.header = header;
        if (typeof header.warp_lines === 'number') mine.warpAfter = header.warp_lines;
        if (typeof header.harness_lines === 'number') mine.harnessAfter = header.harness_lines;
        ((data && data.rows) || []).forEach(function (row) {
          row.at = row.ts ? Date.parse(row.ts) : NaN;
          mine.rows.push(row);
        });
        mine.facts = recordFacts(mine.rows);
        // The bottom of the list is where the new rows land; keep the reader
        // there if that is where they are, and leave them alone if they
        // scrolled up. Measured at the moment the list changes, not when the
        // poll was sent: measured before the fetch, a thumb that started
        // scrolling up while a poll was in flight was pulled back to the tail
        // when it landed (the emulator, 2026-09-06, twice in one minute).
        var atBottom = el.main.scrollHeight - el.main.scrollTop - el.main.clientHeight < 40;
        renderConversationHead();
        renderTrace(mine);
        if (atBottom) el.main.scrollTop = el.main.scrollHeight;
        scheduleTracePoll(mine.summary && mine.summary.is_busy ? TRACE_BUSY_MS : TRACE_IDLE_MS);
      })
      .catch(function (err) {
        if (viewing !== mine) return;
        mine.inflight = false;
        el.convError.hidden = false;
        el.convError.textContent = String(err.message || err);
        scheduleTracePoll(TRACE_IDLE_MS);
      });
  }

  // -- the folding rules, as `trace_render.rs` has them

  function byTime(a, b) {
    var x = isNaN(a.at), y = isNaN(b.at);
    if (x && y) return 0;
    if (x) return 1;
    if (y) return -1;
    return a.at - b.at;
  }

  function clockOf(at) {
    if (isNaN(at)) return '';
    var d = new Date(at);
    var pad = function (n) { return (n < 10 ? '0' : '') + n; };
    return pad(d.getHours()) + ':' + pad(d.getMinutes()) + ':' + pad(d.getSeconds());
  }

  function toolUseBlock(row, id) {
    var content = row.raw && row.raw.message && row.raw.message.content;
    if (!Array.isArray(content)) return null;
    for (var i = 0; i < content.length; i++) {
      if (content[i] && content[i].id === id) return content[i];
    }
    return null;
  }

  function callState(call) {
    if (call.replied && call.replied.raw && call.replied.raw.decision === 'denied') return 'denied';
    if (call.asked && !call.replied && !call.result) return 'unanswered';
    if ((call.result && call.result.error === true) || (call.completed && call.completed.error === true)) return 'failed';
    if (call.result || call.completed) return 'done';
    return 'open';
  }

  var STATE_WORDS = {
    open: 'no result yet',
    done: 'done',
    failed: 'failed',
    denied: 'denied',
    unanswered: 'asked, never answered'
  };

  function callName(call) {
    var block = call.used && toolUseBlock(call.used, call.id);
    if (block && block.name) return String(block.name);
    var warp = call.started || call.asked;
    if (warp && warp.raw && warp.raw.tool_name) return String(warp.raw.tool_name);
    return 'tool';
  }

  function callInput(call) {
    var block = call.used && toolUseBlock(call.used, call.id);
    if (block && block.input !== undefined) {
      if (block.input && typeof block.input.command === 'string') return block.input.command;
      // An edit is drawn as a diff by `callEdit`; the input line is its file.
      if (callEdit(call)) return typeof block.input.file_path === 'string' ? block.input.file_path : null;
      try { return JSON.stringify(block.input); } catch (_) { return null; }
    }
    var warp = call.started || call.asked;
    if (warp && warp.raw && warp.raw.tool_input_preview) return String(warp.raw.tool_input_preview);
    return null;
  }

  // An edit tool's input as hunks (2026-09-06): Claude Code's `Edit` carries
  // `old_string`/`new_string`, `MultiEdit` a list of them, `Write` the whole
  // new content. Two strings side by side are a diff drawn badly; this draws
  // it as one. Only from the harness's own `tool_use` block, since the ACP
  // path writes no input to Warp's log.
  function callEdit(call) {
    var block = call.used && toolUseBlock(call.used, call.id);
    var input = block && block.input;
    if (!input || typeof input !== 'object') return null;
    var hunks = [];
    if (typeof input.old_string === 'string' && typeof input.new_string === 'string') {
      hunks.push({ old: input.old_string, now: input.new_string });
    } else if (Array.isArray(input.edits)) {
      input.edits.forEach(function (e) {
        if (e && typeof e.old_string === 'string' && typeof e.new_string === 'string') hunks.push({ old: e.old_string, now: e.new_string });
      });
    } else if (typeof input.content === 'string' && typeof input.file_path === 'string') {
      hunks.push({ old: '', now: input.content });
    }
    if (!hunks.length) return null;
    return { file: typeof input.file_path === 'string' ? input.file_path : '', hunks: hunks };
  }

  // Lines of `a` and `b` as [op, line] pairs by longest common subsequence.
  // Quadratic, so past a few hundred lines a side it falls back to "all of a
  // removed, all of b added", which is still true and still readable.
  function lineDiff(a, b) {
    var x = a === '' ? [] : a.split('\n');
    var y = b === '' ? [] : b.split('\n');
    var out = [];
    if (x.length * y.length > 250000) {
      x.forEach(function (l) { out.push(['del', l]); });
      y.forEach(function (l) { out.push(['add', l]); });
      return out;
    }
    var n = x.length, m = y.length, i, j;
    var table = new Array(n + 1);
    for (i = 0; i <= n; i++) { table[i] = new Array(m + 1); table[i][m] = 0; }
    for (j = 0; j <= m; j++) table[n][j] = 0;
    for (i = n - 1; i >= 0; i--) {
      for (j = m - 1; j >= 0; j--) {
        table[i][j] = x[i] === y[j] ? table[i + 1][j + 1] + 1 : Math.max(table[i + 1][j], table[i][j + 1]);
      }
    }
    i = 0; j = 0;
    while (i < n && j < m) {
      if (x[i] === y[j]) { out.push(['ctx', x[i]]); i++; j++; }
      else if (table[i + 1][j] >= table[i][j + 1]) { out.push(['del', x[i]]); i++; }
      else { out.push(['add', y[j]]); j++; }
    }
    while (i < n) { out.push(['del', x[i++]]); }
    while (j < m) { out.push(['add', y[j++]]); }
    return out;
  }

  // The diff as a <pre> of spans, text nodes only: the strings are an agent's
  // and are never parsed as markup.
  function diffElement(edit) {
    var pre = document.createElement('pre');
    pre.className = 'diff';
    if (edit.file) pre.appendChild(text('span', 'file', edit.file + '\n'));
    var marks = { del: '- ', add: '+ ', ctx: '  ' };
    edit.hunks.forEach(function (hunk, index) {
      if (index > 0) pre.appendChild(text('span', 'file', '…\n'));
      lineDiff(hunk.old, hunk.now).forEach(function (pair) {
        pre.appendChild(text('span', pair[0], marks[pair[0]] + pair[1] + '\n'));
      });
    });
    return pre;
  }

  function callFirst(call) {
    var first = NaN;
    [call.used, call.result, call.started, call.completed, call.asked, call.replied].forEach(function (row) {
      if (row && !isNaN(row.at) && (isNaN(first) || row.at < first)) first = row.at;
    });
    return first;
  }

  function callDecision(call) {
    if (!call.replied || !call.replied.raw || !call.replied.raw.decision) return null;
    var by = call.replied.raw.answered_by;
    var via = viaLabel(call.replied.raw);
    return 'Warp: ' + call.replied.raw.decision + (by ? ' by ' + by : '') + (via ? ' ' + via : '');
  }

  // `via: paired_device` on a Warp line (T19) is the record saying the prompt
  // or the answer came from a phone paired for this conversation. The door
  // was the control plane either way; this names who was at it.
  function viaLabel(raw) {
    if (!raw || !raw.via) return null;
    return raw.via === 'paired_device' ? 'from the phone' : 'from another device';
  }

  function callStamps(call) {
    var span = function (openRow, closeRow) {
      if (!openRow || isNaN(openRow.at)) return null;
      var s = clockOf(openRow.at);
      if (closeRow && !isNaN(closeRow.at)) s += ' → ' + clockOf(closeRow.at);
      return s;
    };
    var parts = [];
    var w = span(call.started, call.completed);
    if (w) parts.push('warp ' + w);
    var h = span(call.used, call.result);
    if (h) parts.push('harness ' + h);
    return parts.join(' · ');
  }

  // The rows folded into items: each call once, at its first appearance, and
  // the harness's copy of a prompt dropped when Warp's has the same text --
  // Warp's is kept because it is stamped on Warp's clock, the one the rest of
  // the page runs on.
  function foldItems(rows) {
    var sorted = rows.slice().sort(byTime);
    var calls = {};
    var slots = {
      'harness tool_use': 'used', 'harness tool_result': 'result',
      'warp tool_start': 'started', 'warp tool_complete': 'completed',
      'warp permission_request': 'asked', 'warp permission_replied': 'replied'
    };
    sorted.forEach(function (row) {
      if (!row.call_id) return;
      var slot = slots[row.from + ' ' + row.kind];
      if (!slot) return;
      var call = calls[row.call_id] || (calls[row.call_id] = { id: row.call_id });
      if (!call[slot]) call[slot] = row;
    });
    var warpPrompts = {};
    sorted.forEach(function (row) {
      if (row.from === 'warp' && (row.kind === 'session_start' || row.kind === 'prompt_submit') && row.text) {
        warpPrompts[row.text] = true;
      }
    });
    var drawn = {};
    var items = [];
    sorted.forEach(function (row) {
      if (row.call_id) {
        if (!drawn[row.call_id] && calls[row.call_id]) {
          drawn[row.call_id] = true;
          items.push({ call: calls[row.call_id] });
        }
        return;
      }
      if (row.from === 'harness' && row.kind === 'prompt' && row.text && warpPrompts[row.text]) return;
      items.push({ row: row });
    });
    return items;
  }

  // What a non-call row says: Warp's frame lines get a label, the harness's
  // bookkeeping gets its subtype, the rest is the text.
  function caption(row) {
    var key = row.from + ' ' + row.kind;
    var labels = {
      'warp session_agent': 'agent', 'warp session_mode': 'mode', 'warp session_model': 'model',
      'warp session_start': '', 'warp prompt_submit': '',
      'harness prompt': '', 'harness text': '', 'harness thinking': 'thinking',
      'harness permission_mode': 'permission mode',
      'harness compact_summary': 'summary the agent continued from', 'harness usage': 'usage'
    };
    if (key === 'warp stop') return { label: 'stop', text: row.raw && row.raw.error_type ? String(row.raw.error_type) : null };
    if (row.kind === 'unparsed') return { label: 'unparsed line', text: row.text };
    if (key === 'warp prompt_submit' || key === 'warp session_start') return { label: viaLabel(row.raw) || '', text: row.text };
    if (labels[key] !== undefined) return { label: labels[key], text: row.text };
    return { label: row.kind, text: row.text };
  }

  function compactionCaption(row) {
    var meta = (row.raw && row.raw.compactMetadata) || {};
    var parts = ['compaction'];
    if (meta.trigger) parts.push(String(meta.trigger));
    if (typeof meta.preTokens === 'number') parts.push(meta.preTokens + ' tokens before');
    if (typeof meta.durationMs === 'number') parts.push((meta.durationMs / 1000).toFixed(1) + ' s');
    return parts.join(', ');
  }

  // A block of text, folded behind a summary when it is long. `<details>` is
  // markup the page creates, not markup it parses: the text is a text node.
  function block(content, foldAfter) {
    var lines = String(content).split('\n');
    var pre = text('pre', null, content);
    if (lines.length <= foldAfter) return pre;
    var details = document.createElement('details');
    details.appendChild(text('summary', null, lines.length + ' lines'));
    details.appendChild(pre);
    return details;
  }

  function traceItem(item) {
    var li = document.createElement('li');
    if (item.row) {
      var row = item.row;
      if (row.kind === 'system/compact_boundary') {
        li.className = 'boundary';
        li.textContent = '──── ' + compactionCaption(row) + ' · ' + clockOf(row.at) + ' ────';
        return li;
      }
      li.className = 'who-' + row.who + (row.kind === 'thinking' ? ' thinking' : '');
      li.appendChild(text('span', 't', clockOf(row.at)));
      var body = text('div', 'body');
      var cap = caption(row);
      if (row.kind === 'compact_summary') {
        body.appendChild(text('span', 'label', cap.label));
        body.appendChild(block(cap.text || '', 1));
      } else {
        if (cap.label) body.appendChild(text('span', 'label', cap.label + (cap.text ? ': ' : '')));
        if (cap.text) body.appendChild(document.createTextNode(cap.text));
        else if (!cap.label) body.appendChild(document.createTextNode(row.kind));
      }
      li.appendChild(body);
      return li;
    }
    var call = item.call;
    var state = callState(call);
    li.className = 'who-agent call state-' + state;
    li.appendChild(text('span', 't', clockOf(callFirst(call))));
    var callBody = text('div', 'body');
    var head = text('div', 'head');
    head.appendChild(text('b', null, callName(call)));
    head.appendChild(text('span', 'state', STATE_WORDS[state]));
    var decision = callDecision(call);
    if (decision) {
      head.appendChild(document.createTextNode(' '));
      head.appendChild(text('span', 'label', decision));
    }
    callBody.appendChild(head);
    var input = callInput(call);
    if (input) callBody.appendChild(text('code', 'cmd', input));
    var edit = callEdit(call);
    if (edit) callBody.appendChild(diffElement(edit));
    var stamps = callStamps(call);
    if (stamps) callBody.appendChild(text('div', 'stamps', stamps));
    if (call.result && call.result.text) callBody.appendChild(block(call.result.text, 6));
    li.appendChild(callBody);
    return li;
  }

  function renderTrace(state) {
    var header = state.header || {};
    var note = [];
    if (header.harness) {
      note.push(header.harness + ' ' + (header.harness_versions || []).join('/') + ', ' + header.harness_lines + ' lines');
    } else {
      note.push('Warp\'s half only');
    }
    if (typeof header.clock_offset_ms === 'number') {
      note.push('harness clock ' + (header.clock_offset_ms > 0 ? '+' : '') + header.clock_offset_ms + ' ms, disclosed and not applied');
    }
    if (header.note) note.push(header.note);
    el.convNote.className = 'note';
    el.convNote.textContent = note.join(' · ');
    el.convError.hidden = true;

    // Usage is summed here because each tail carries its own footer row.
    var usage = { messages: 0, input: 0, output: 0, cache_read: 0, cache_creation: 0 };
    var rows = state.rows.filter(function (row) {
      if (row.kind !== 'usage' || !row.raw) return true;
      usage.messages += row.raw.assistant_messages || 0;
      usage.input += row.raw.input_tokens || 0;
      usage.output += row.raw.output_tokens || 0;
      usage.cache_read += row.raw.cache_read_input_tokens || 0;
      usage.cache_creation += row.raw.cache_creation_input_tokens || 0;
      return false;
    });
    clear(el.trace);
    foldItems(rows).forEach(function (item) { el.trace.appendChild(traceItem(item)); });
    el.traceFoot.textContent = usage.messages
      ? 'usage: ' + usage.messages + ' assistant messages · ' + usage.input + ' in · ' + usage.output +
        ' out · ' + usage.cache_read + ' cache read · ' + usage.cache_creation + ' cache written'
      : '';
    if (!rows.length) {
      el.convNote.textContent = (el.convNote.textContent ? el.convNote.textContent + ' · ' : '') + 'nothing recorded yet.';
    }
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
    syncNotifyButton();
    if (notificationsPossible() && Notification.permission === 'granted') registerNotifier();
    // A device paired by `/remote-control` was handed one conversation, and
    // the server told it which at pairing. Open it straight away; the summary
    // fills in from the first state poll.
    if (device && device.conversation_id && !viewing) {
      openConversation({ conversation_id: device.conversation_id, status: 'connecting' });
    }
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
      closeConversation();
      forgetDevice('unpaired on this device. Run `warpctrl pair show` and scan again to come back.');
    });
    el.notify.addEventListener('click', askToNotify);
    // Back goes through history so the phone's own gesture and the button are
    // one path; `popstate` is where the view actually closes.
    el.back.addEventListener('click', function () { history.back(); });
    window.addEventListener('popstate', closeConversation);
    el.promptForm.addEventListener('submit', function (submit) {
      submit.preventDefault();
      sendPrompt();
    });

    // Read the fragment once and erase it before anything can render, so the
    // code is never on screen, in history, or in a `Referer`.
    var code = location.hash.replace(/^#/, '');
    if (code) history.replaceState(null, '', location.pathname);

    // A code in the URL is the person's own act, just now, and outranks a
    // device this browser remembers. Measured on the emulator (2026-09-06):
    // the remembered device is dead after every Warp restart, and a page
    // that tried it first read the fresh code out of the URL, erased it, and
    // then said "pair again" — the second scan after any restart failed
    // until the person found *unpair*. If the code is refused, the
    // remembered device is still worth a try; if there is neither, say so.
    device = loadDevice();
    if (code) {
      redeem(code).then(start).catch(function (err) {
        if (device) { start(); return; }
        showPairing('pairing failed: ' + String(err.message || err) +
          ' — codes last two minutes and are spent on first use.');
      });
      return;
    }
    if (device) {
      start();
      return;
    }
    showPairing('run `warpctrl pair show` on the machine running Warp, then scan the QR it prints.');
  }

  boot();
})();
