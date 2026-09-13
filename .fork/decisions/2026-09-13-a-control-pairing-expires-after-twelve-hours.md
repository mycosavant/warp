> A decision on record, 2026-09-13. Binding until a later decision names it.
> Supersedes the third bullet of `2026-09-06-the-phone-surface-four-decisions.md`,
> "A control pairing has no clock."

- **A phone paired to control a conversation holds a token that expires after
  twelve hours, like a watch pairing, in addition to ending on Stop sharing,
  on the conversation being deleted, or on Warp closing.** Decided by the
  maintainer, 2026-09-13. They added that this risk is one more reason for the native app
  (`2026-09-13-a-native-android-app-is-a-destination.md`), and they recalled
  the original design as a token that could not be used twice.

  Today a control token has no clock (READ, `app/src/local_control/pairing.rs:436-439`).
  That was deliberate: commit `ae694127a` (2026-09-06, T19) implemented the
  09-06 decision, which argued that a control pairing is minted by a person
  pointing at a conversation and that the credential purge on stop bounds a
  leak. What the maintainer remembers as single-use is the pairing **code**,
  which still cannot be redeemed twice (READ, `pairing.rs:425-432`). The reason
  for adding the clock is the fable-advisor's 2026-09-13 assessment: the phone
  keeps the token in `localStorage`, so a lost or backed-up phone holds a live
  credential for as long as Warp keeps running with that conversation shared,
  and with `WARP_FORK_REMOTE_APPROVE` set it can approve agent actions. The
  work is `HANDOFF-SECURITY.md` task 4.
