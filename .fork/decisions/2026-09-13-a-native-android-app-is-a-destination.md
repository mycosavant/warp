> A decision on record, 2026-09-13. Binding until a later decision names it.

- **A native Android app is a destination for the phone surface**, recorded at
  the maintainer's word on 2026-09-13 after friction row 3 noted that nothing
  tracked it. The first version would be minimal and on-thesis: a
  remote-control GUI that pairs with the desk instance, the way the maintainer
  describes T3 Code's app (believed React Native; its architecture has not
  been read here). The maintainer's leaning, stated as thoughts rather than a
  choice: native Android with a Rust core, Compose if needed, Kotlin
  Multiplatform if the Rust core is compatible with it. **Nothing is scoped.**
  What already exists for it to talk to: `/remote-control`'s pairing, device
  token and scoped actions (`docs/remote-control.md`), the console over TLS
  with Warp's own authority, and reach over the tailnet (`reach.html`). The
  browser console remains the surface in use until a spec is written.

  **The strongest motivation, added the same evening:** the console's private
  certificate authority. With it installed, the maintainer's DDG (Chromium)
  and Firefox still show the page as not secure, so day-to-day use has moved to
  mosh+tmux. A native app can pin the fork's authority inside the app instead of
  asking the phone to trust a root system-wide, which removes both the browser
  friction and most of what a stolen authority key could do to the phone.
  `HANDOFF-SECRETS.md` task 3 handles the authority in the meantime.
