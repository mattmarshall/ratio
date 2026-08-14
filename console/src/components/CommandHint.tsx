"use client";

import { useEffect, useState } from "react";
import { useKBar } from "@/lib/kbar";

/**
 * Where an operator finds out the palette is there.
 *
 * ⛔ A BUTTON, NOT A LABEL. A shortcut printed where it cannot be clicked tells
 * somebody with a pointer what they are missing and gives them no way to have it.
 *
 * ⚠ THE MODIFIER CANNOT BE RENDERED ON THE SERVER, so it starts empty and lands
 * after mount. There is no platform to read during SSR, and guessing one means
 * the server and the first client render disagree — a hydration mismatch, for a
 * glyph. `aria-keyshortcuts` carries the fact meanwhile, so the shortcut is
 * announced even in the frame where nothing is drawn, and `.kbd` has a
 * `min-width` so the header does not jump when it arrives.
 *
 * ⛔ THIS PREDICATE IS kbar's, CHARACTER FOR CHARACTER. The chord is bound by the
 * copy of tinykeys vendored into `kbar/lib`, which resolves `$mod` ONCE at module
 * load from `/Mac|iPod|iPhone|iPad/.test(navigator.platform)`. kbar also exports
 * an `isModKey` that tests `platform === "MacIntel"` — a DIFFERENT question, and
 * using it here would print ⌘ on an iPad where Control is what fires.
 */
export function CommandHint() {
  const { query } = useKBar();
  const [mod, setMod] = useState<string | null>(null);

  useEffect(() => {
    setMod(/Mac|iPod|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl");
  }, []);

  return (
    <button
      type="button"
      className="cmdk"
      onClick={() => query.toggle()}
      aria-keyshortcuts="Meta+K Control+K"
    >
      Search
      <kbd className="kbd">{mod === null ? "" : `${mod} K`}</kbd>
    </button>
  );
}
