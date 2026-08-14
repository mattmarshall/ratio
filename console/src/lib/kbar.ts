// The half of kbar that every fund-scoped screen pays for.
//
// ⛔ DEEP IMPORTS, AND THIS FILE EXISTS SO THERE IS EXACTLY ONE PLACE THEY LIVE.
// kbar is CommonJS with no `exports` map and no `module` field, so `next build`
// cannot tree-shake it: one `import { KBarProvider } from "kbar"` pulls
// `lib/index.js`, which requires every component in the package — and with them
// `fuse.js` and `@tanstack/react-virtual`. Measured, that is 28.0 KB gzipped on
// every screen under `/funds`, whether or not anybody presses ⌘K.
//
// ⭐ THE REQUIRE GRAPH IS WHAT MAKES THE SPLIT POSSIBLE, and it is worth writing
// down because it is the whole reason this is not premature:
//
//     KBarContextProvider -> useStore, InternalEvents -> tinykeys, useKBar, utils
//     useRegisterActions  -> useKBar
//     useMatches          -> fuse.js                      ⟵ the search
//     KBarResults         -> @tanstack/react-virtual      ⟵ the list
//
// Nothing on the first two lines needs either heavy dependency. So the provider,
// the hooks and the two enums come from here, eagerly; `PaletteUI.tsx` imports
// the rest and is loaded on first open.
//
// ⚠ REACHING PAST A PACKAGE'S ENTRY POINT IS A REAL LIABILITY, and the mitigation
// is that it cannot fail quietly: these are typed paths with `.d.ts` files beside
// them, so a release that moves `lib/` fails `tsc --noEmit` rather than shipping.
// Pinned exactly at 1.0.0, as everything in this package.json is.
//
// ⛔ AND NOTHING HERE MAY RE-EXPORT `KBarResults`, `KBarSearch`, `KBarPortal` OR
// `useMatches`. Adding one puts fuse.js back in the eager chunk for every
// importer of this file, and the bundle would quietly return to where it started.

export { KBarProvider } from "kbar/lib/KBarContextProvider";
export { useKBar } from "kbar/lib/useKBar";
export { useRegisterActions } from "kbar/lib/useRegisterActions";
export { Priority } from "kbar/lib/utils";
export { VisualState } from "kbar/lib/types";

// ⚠ Types only, so `verbatimModuleSyntax` erases the import entirely and the
// barrel is never required at runtime. This is the one place `"kbar"` itself may
// be named.
export type { Action, ActionImpl, KBarOptions } from "kbar";
