/** The wordmark. Three bars, the same shape as `site/marks/mark-ratio.svg`. */
export function Brand() {
  return (
    <span className="brand">
      <svg viewBox="0 0 64 64" fill="currentColor" aria-hidden="true">
        <rect x="8" y="19" width="16.34" height="10" rx="2" />
        <rect x="29.34" y="19" width="26.66" height="10" rx="2" />
        <rect x="8" y="35" width="48" height="10" rx="2" />
      </svg>
      ratio
    </span>
  );
}
