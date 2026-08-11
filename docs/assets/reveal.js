// Adds .mg-in to anything marked .mg-reveal once it scrolls into view.
// Without this file the CSS leaves those elements visible, so a failed
// or blocked script costs an animation rather than the page.
(function () {
  function watch() {
    var targets = document.querySelectorAll(".mg-reveal:not(.mg-in)");
    if (!targets.length) return;

    if (!("IntersectionObserver" in window)) {
      targets.forEach(function (el) {
        el.classList.add("mg-in");
      });
      return;
    }

    var seen = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (!entry.isIntersecting) return;
          entry.target.classList.add("mg-in");
          seen.unobserve(entry.target);
        });
      },
      { rootMargin: "0px 0px -12% 0px", threshold: 0.15 }
    );

    targets.forEach(function (el) {
      seen.observe(el);
    });
  }

  // navigation.instant swaps the document body without a reload, so the
  // pass runs again on every page change.
  if (window.document$) {
    window.document$.subscribe(watch);
  } else {
    document.addEventListener("DOMContentLoaded", watch);
  }
})();
