//! Two-step confirmation for the handful of controls that destroy something the
//! user built by hand — a vocabulary word with every alias taught for it, a
//! provider with its saved key. Deliberately not a dialog: the row stays visible
//! and in place while you decide, and one more click is the whole cost.
//!
//! Lives in its own module because it is the only piece of main.js that can be
//! tested without a browser — it touches nothing but textContent, dataset and
//! classList.

/// How long an armed button waits before it answers "no" for you.
export const CONFIRM_WINDOW_MS = 3000;

/**
 * Arm `btn` for a second click, or report that it is already armed.
 *
 * First click: the button becomes the question and returns false — the caller
 * must not act. Second click within the window: returns true. After the window
 * the button silently goes back to what it was.
 */
export function armConfirm(btn, question, timers = globalThis) {
  if (btn.dataset.armed === "1") return true;

  const original = btn.textContent;
  btn.dataset.armed = "1";
  btn.textContent = question;
  btn.classList.add("confirming");

  timers.setTimeout(() => {
    // A second click already consumed the arming, or the list re-rendered and
    // this button is a ghost. Either way its state is not ours to restore.
    if (btn.dataset.armed !== "1") return;
    btn.dataset.armed = "";
    btn.textContent = original;
    btn.classList.remove("confirming");
  }, CONFIRM_WINDOW_MS);

  return false;
}
