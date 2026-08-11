//! What the dot under a history row says about itself.
//!
//! Green means the editor rephrased the line, yellow means it didn't, red means
//! the line never reached the app. Yellow used to stop there — "not rephrased",
//! nothing more — and that is exactly the moment the user needs to decide
//! between waiting (rate limit, provider down) and fixing something (no key, a
//! model that no longer exists). The reason travels with the entry from the
//! backend (`llm_error`); this module only turns it into the row's words.
//!
//! Kept out of main.js so the wording can be unit-tested without a DOM.

/// Shown when a row has no reason of its own: the editor simply wasn't asked.
/// Old entries logged before reasons existed land here too — they were, in
/// almost every case, dictated with the editor off.
const NO_ATTEMPT = "the editor was off";

/// Everything the meta row under one transcript renders: which colour the dot
/// is, what it says on hover, and the label beside it.
export function logMeta({ edited, llmError, llmHost, llmModel, insertError }) {
  // A dictation that never got typed outranks the rephrased/not-rephrased
  // signal — those rows are the ones the user has to find.
  if (insertError) {
    return {
      dotClass: "failed",
      hint: `never typed into the app — click the line to copy it. ${insertError}`,
      label: "",
    };
  }
  if (edited === true) {
    return {
      dotClass: "edited",
      hint: "rephrased",
      label: llmHost && llmModel ? `${llmHost} | ${llmModel}` : "",
    };
  }
  const why = llmError || NO_ATTEMPT;
  return {
    dotClass: "unedited",
    hint: `not rephrased — ${why}`,
    // The provider is half the answer: the same "timed out" reads differently
    // depending on which rung of the stack was live at the time.
    label: llmHost ? `${llmHost} | ${why}` : why,
  };
}
