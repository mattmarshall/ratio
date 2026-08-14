"use client";

import type { ReactNode } from "react";
import { useState } from "react";

/**
 * The shape every write screen in this console has.
 *
 * ⭐ THE FOUR WRITES WERE FOUR FORMS, AND THEY SHOULD BE ONE PATTERN. `/record`,
 * `/ingest`, `/mark` and `/trade` each grew their own stack of bare `<label>`s
 * with a Preview and a commit button underneath — the same idea rendered four
 * ways, none of which used the form styling `globals.css` had been carrying
 * since the concept. An operator who has learned one of them has learned
 * nothing about the next.
 *
 * So there is one shell, and it offers both ways through:
 *
 *   * **Guided.** A tree of the steps with the answers so far, and one question
 *     on screen at a time. The tree is the thing that makes a stepper honest —
 *     it shows what has been answered without making anybody click back to find
 *     out, and every step on it is a way back to that step.
 *   * **Form.** Every field at once, compact.
 *
 * ⛔ TWO RENDERINGS OF ONE STATE, NEVER TWO FORMS. The caller owns the state and
 * hands this component two views of it, so switching mid-ticket keeps every
 * answer. Two components each holding half the answers is how a "compact mode"
 * comes to silently drop the field the other one had.
 *
 * ⚠ AND THE LAST STEP IS ALWAYS REVIEW. A stepper whose final page can be
 * opened with three questions unanswered is a form with extra clicks, so a step
 * is reachable only when every step before it is answered.
 */
export interface Step {
  /** Stable id. Also the key. */
  readonly id: string;
  /** Two or three words, for the tree. */
  readonly label: string;
  /** The question, asked as a question. */
  readonly ask: string;
  /** Why it is asked, or what will be done with it. */
  readonly why?: ReactNode;
  /**
   * What the tree shows for this step, or null when it is unanswered.
   *
   * ⛔ THE TREE AND `Next` READ THE SAME FIELD. A step that can be complete on
   * the tree and incomplete for the button is how a stepper comes to let
   * somebody past a question it then complains about at the end.
   */
  readonly answer: string | null;
  /** The control, shown when this step has focus. */
  readonly body: ReactNode;
}

export function Ticket({
  title,
  note,
  summary,
  steps,
  compact,
  actions,
  children,
}: {
  title: string;
  /** The right-hand side of the section head. */
  note?: string;
  /** The ticket said back as a sentence. Shown in both views. */
  summary?: ReactNode;
  /** In order. The last one is the review. */
  steps: readonly Step[];
  /** Every field at once, for people who do this forty times a day. */
  compact: ReactNode;
  /** Preview and commit. Shown on the last step, and always in Form. */
  actions: ReactNode;
  /** Below both views: refusals, and what the server said. */
  children?: ReactNode;
}) {
  const [guided, setGuided] = useState(true);
  const [at, setAt] = useState(steps[0]?.id ?? "");

  const index = Math.max(
    0,
    steps.findIndex((s) => s.id === at),
  );
  const step = steps[index];
  const last = index === steps.length - 1;
  // Reachable when everything before it has an answer.
  const reachable = (i: number) => steps.slice(0, i).every((s) => s.answer !== null);
  const go = (d: number) => {
    const next = steps[Math.min(steps.length - 1, Math.max(0, index + d))];
    if (next) setAt(next.id);
  };

  return (
    <section className="rec" aria-label={title}>
      <div className="loghead">
        <span>{title}</span>
        <span className="hd">
          {note ? <span className="sortnote">{note}</span> : null}
          {/* ⚠ BUTTONS AND `aria-pressed`, NOT LINKS AND `aria-current`. These
              narrow what is on screen rather than navigate, and a half-typed
              ticket is not a URL anybody should be sent. The view tabs above are
              links for exactly the opposite reason. */}
          <div className="views" role="group" aria-label="How to fill this in">
            <button type="button" aria-pressed={guided} onClick={() => setGuided(true)}>
              Guided
            </button>
            <button type="button" aria-pressed={!guided} onClick={() => setGuided(false)}>
              Form
            </button>
          </div>
        </span>
      </div>

      {summary ? <p className="cap">{summary}</p> : null}

      {guided ? (
        <div className="ticket">
          <ol className="steps" aria-label="Steps">
            {steps.map((s, i) => (
              <li key={s.id}>
                <button
                  type="button"
                  className="stepbtn"
                  aria-current={s.id === step?.id ? "step" : undefined}
                  disabled={!reachable(i)}
                  onClick={() => setAt(s.id)}
                >
                  <span className="sk">
                    {/* ⚠ A SHAPE AS WELL AS A COLOUR, which is the rule the fund
                        rail and the severity stripes already follow — so state
                        survives a printout, a projector and colour blindness. */}
                    <span
                      className={`state ${
                        s.id === step?.id
                          ? "review"
                          : s.answer !== null
                            ? "struck"
                            : "waiting"
                      }`}
                    />
                    {s.label}
                  </span>
                  <span className="sv">{s.answer ?? "—"}</span>
                </button>
              </li>
            ))}
          </ol>

          {step ? (
            <div className="step">
              <h3>{step.ask}</h3>
              {step.why ? <p className="why">{step.why}</p> : null}
              {step.body}
            </div>
          ) : null}

          <div className="formbar">
            <span className="sortnote">
              {step && step.answer === null && !last
                ? "Answer this to go on."
                : `Step ${index + 1} of ${steps.length}`}
            </span>
            <button
              className="act"
              type="button"
              onClick={() => go(-1)}
              disabled={index === 0}
            >
              Back
            </button>
            {last ? null : (
              <button
                className="act primary"
                type="button"
                onClick={() => go(1)}
                disabled={step?.answer === null || !reachable(index + 1)}
              >
                Next
              </button>
            )}
          </div>
        </div>
      ) : (
        <div className="ticket compact">{compact}</div>
      )}

      {/* ⚠ ON THE LAST STEP ONLY, in Guided. Putting a commit button beside
          `Next` would make the primary control mean two different things one
          step apart. */}
      {!guided || last ? actions : null}

      {children}
    </section>
  );
}

/**
 * One labelled control.
 *
 * ⛔ NEVER `type="number"`. A number input hands back a double, and these values
 * become journal figures — `lib/format.ts` refuses to parse money into a number
 * for the same reason, one layer down. Digits in, digits out.
 */
export function Field({
  name,
  value,
  onValue,
  hint,
  mode,
  rows,
}: {
  name: string;
  value: string;
  onValue: (v: string) => void;
  hint?: string;
  /** Set for a figure: monospace, tabular, and the right soft keyboard. */
  mode?: "decimal" | "numeric";
  /** Set for a file. */
  rows?: number;
}) {
  return (
    <label>
      <span>{name}</span>
      {rows ? (
        <textarea
          value={value}
          onChange={(e) => onValue(e.target.value)}
          rows={rows}
          placeholder={hint}
          aria-label={name}
        />
      ) : (
        <input
          className={mode ? "num" : undefined}
          value={value}
          onChange={(e) => onValue(e.target.value)}
          inputMode={mode}
          placeholder={hint}
          aria-label={name}
        />
      )}
    </label>
  );
}

/** One labelled choice from a closed list. */
export function Picker({
  name,
  value,
  onValue,
  empty,
  options,
}: {
  name: string;
  value: string;
  onValue: (v: string) => void;
  /** What the disabled first option says when nothing is chosen. */
  empty: string;
  options: readonly { readonly value: string; readonly label: string }[];
}) {
  return (
    <label>
      <span>{name}</span>
      <select
        value={value}
        onChange={(e) => onValue(e.target.value)}
        aria-label={name}
        disabled={options.length === 0}
      >
        <option value="" disabled>
          {empty}
        </option>
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}

/**
 * Preview and commit, with the discipline the console asks for enforced.
 *
 * ⛔ EVERY WRITE SCREEN HERE SAID "preview, then post" AND NOT ONE OF THEM MADE
 * IT SO. Each would let somebody preview, change a figure, and commit the
 * changed one with the old preview still on screen describing what the button
 * was about to do. `previewed` is the caller's comparison of what came back
 * against what it currently holds, and the commit stays shut until they agree.
 */
export function Commit({
  preview,
  commit,
  previewed,
  pending,
  ready,
}: {
  /** The word on the safe button. */
  preview: string;
  /** The word on the one that writes. */
  commit: string;
  previewed: boolean;
  pending: boolean;
  ready: boolean;
}) {
  return (
    <div className="formbar">
      <span className="sortnote">
        {previewed
          ? `Previewed. ${commit} writes exactly what is shown below.`
          : "Preview runs the same code path on the server and writes nothing."}
      </span>
      <button className="act" type="submit" disabled={pending || !ready}>
        {pending ? "…" : preview}
      </button>
      <button
        className="act primary"
        type="submit"
        name="commit"
        value="1"
        disabled={pending || !previewed}
      >
        {commit}
      </button>
    </div>
  );
}
