/* Mobile chrome: the nav toggle, and the figure scroll hint.
   Inlined into every page by build.py.

   PROGRESSIVE ENHANCEMENT, deliberately. The markup ships with the nav
   visible; this script is what collapses it behind the button. So if the
   script never runs — blocked, failed, ancient browser — the nav degrades to
   the scrolling single row it was before, rather than to a menu that cannot
   be opened. The button is hidden by CSS until `data-js` says otherwise, for
   the same reason: a toggle that toggles nothing is worse than no toggle.

   The nav is `aria-expanded` on a real <button> rather than a checkbox hack,
   because the checkbox announces itself as a checkbox and this is a menu. */
(function () {
  var nav = document.querySelector('.r-nav');
  if (!nav) return;
  var btn = nav.querySelector('.r-navtoggle');
  var list = nav.querySelector('.r-nav-list');
  if (!btn || !list) return;

  nav.setAttribute('data-js', '');          // CSS only collapses once this is set
  var open = false;

  function set(next) {
    open = next;
    nav.toggleAttribute('data-open', open);
    btn.setAttribute('aria-expanded', open ? 'true' : 'false');
  }
  set(false);

  btn.addEventListener('click', function (e) {
    e.stopPropagation();
    set(!open);
  });

  // Following a link should not leave the panel hanging open behind the
  // next page in browsers that restore scroll and DOM from bfcache.
  list.addEventListener('click', function (e) {
    if (e.target.closest('a')) set(false);
  });

  document.addEventListener('click', function (e) {
    if (open && !nav.contains(e.target)) set(false);
  });

  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape' && open) { set(false); btn.focus(); }
  });

  // Leaving mobile width with the panel open would otherwise strand the
  // attribute on a layout that no longer uses it.
  var wide = window.matchMedia('(min-width: 721px)');
  (wide.addEventListener ? wide.addEventListener.bind(wide, 'change')
                         : wide.addListener.bind(wide))(function (m) {
    if (m.matches && open) set(false);
  });
})();

/* Mark figures that actually overflow their container, so the "scroll to see
   the whole diagram" hint only appears where it is true. A CSS-only hint had
   to guess from viewport width and was therefore wrong on any screen wide
   enough to fit the diagram but narrow enough to match the breakpoint. */
(function () {
  var figs = [].slice.call(document.querySelectorAll('.r-fig .r-scroll'));
  if (!figs.length) return;

  function mark() {
    figs.forEach(function (d) {
      d.toggleAttribute('data-scrollable', d.scrollWidth > d.clientWidth + 2);
    });
  }
  mark();

  // Fonts land after first paint and change the measurement.
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(mark);

  var t;
  window.addEventListener('resize', function () {
    clearTimeout(t);
    t = setTimeout(mark, 150);
  });
})();
