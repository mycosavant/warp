> A decision on record, 2026-09-13. Binding until a later decision names it.

- **A native Android app is a destination for the phone surface. Nothing is
  scoped.** The maintainer, verbatim: *"APK is also a destination and T3-code
  has a good example of how i think this might work on-thesis and minimally for
  a first effort. it's well designed, but is basically just a remote-control
  GUI that pairs with the desk instance. not sure of the architecture, but i
  think it's react native. I'd like to do a native android, rust-core app,
  which may/may not require composer, maybe Kotlin MP if the rust-core is
  compatible. just thoughts for now."* Later the same evening: *"Honestly, the
  CA cert is probably the biggest single motivator to move toward a native
  APK. i have mostly switched to mosh+tmux for now since under my current
  browser configs, it still says unsecure despite having installed the CA."*
  And: *"this is all really enforcing the need for an APK. i've really found
  the mosh+tmux experience to be better anyways, but this risk has been part of
  the reason i've pushed for that."*

  T3 Code's app has not been read here. What the app would talk to already
  exists: `/remote-control`'s pairing and scoped actions
  (`docs/remote-control.md`), the console over TLS, and reach over the tailnet
  (`reach.html`). An app can pin the fork's authority inside itself rather than
  asking the phone to trust a root system-wide, which answers the browser
  problem and most of what a stolen authority key could do to the phone.
  Until a spec exists, `HANDOFF-SECURITY.md` task 5 hardens the authority the
  browser console uses.
