'use strict';
// The console's service worker (T19, 2026-09-06). It exists so a
// notification can be shown at all: Chrome on Android has no `Notification`
// constructor and shows a page's notification only through a registration's
// `showNotification`, so the page's own foreground call is routed through
// here. Measured on the emulator before it was written: the constructor path
// posted nothing and its catch said nothing.
//
// It listens to nothing on the network. No `fetch` handler, so every request
// still goes to the server and `no-store` still means what it says; no `push`
// handler, because Web Push would reach this phone through Google's or
// Apple's relay and is a service nobody here consented to. What it does is
// take a tap on a notification back to the page.
self.addEventListener('install', function () {
  self.skipWaiting();
});
self.addEventListener('activate', function (event) {
  event.waitUntil(self.clients.claim());
});
self.addEventListener('notificationclick', function (event) {
  event.notification.close();
  event.waitUntil(
    self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then(function (list) {
      for (var i = 0; i < list.length; i++) {
        if ('focus' in list[i]) return list[i].focus();
      }
      return self.clients.openWindow('/');
    })
  );
});
