/*
 * Senbit web site: the only script (plus init.js, one line). Four small, independent features:
 *   1. the menu button on small screens;
 *   2. the "Copy" buttons next to commands;
 *   3. the published version, read from the update repository;
 *   4. the fade-in of cards when they scroll into view.
 *
 * Nothing here is needed to read the site: without JavaScript every page works.
 */
(function () {
  "use strict";

  // The update repository, published on GitHub Pages by .github/workflows/publish-updates.yml.
  var PACKAGES_URL =
    "https://yo-le-zz.github.io/senbit/dists/stable/main/binary-amd64/Packages";

  /* ---- 1. Menu ---------------------------------------------------------- */

  var toggle = document.querySelector(".menu-toggle");
  var nav = document.getElementById("site-nav");

  if (toggle && nav) {
    toggle.addEventListener("click", function () {
      var open = nav.classList.toggle("open");
      toggle.setAttribute("aria-expanded", String(open));
    });
  }

  /* ---- 2. Copy buttons -------------------------------------------------- */

  document.querySelectorAll("[data-copy]").forEach(function (button) {
    button.addEventListener("click", function () {
      if (!navigator.clipboard) {
        return;
      }
      navigator.clipboard.writeText(button.dataset.copy).then(function () {
        button.textContent = "Copied";
        setTimeout(function () {
          button.textContent = "Copy";
        }, 1500);
      });
    });
  });

  /* ---- 4. Scroll animation ----------------------------------------------- */

  var revealed = document.querySelectorAll(".reveal");

  if (window.IntersectionObserver) {
    var observer = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (entry.isIntersecting) {
            entry.target.classList.add("visible");
            observer.unobserve(entry.target);
          }
        });
      },
      // Any visible pixel is enough: elements at the very end of a page must not stay hidden.
      { threshold: 0 }
    );
    revealed.forEach(function (element) {
      observer.observe(element);
    });

    // A jump (anchor, reload halfway down) can carry the page past an element without it ever
    // crossing the screen: the observer is not called then. Reveal what is above the screen.
    var scrollPending = false;
    window.addEventListener(
      "scroll",
      function () {
        if (scrollPending) {
          return;
        }
        scrollPending = true;
        requestAnimationFrame(function () {
          scrollPending = false;
          revealed.forEach(function (element) {
            if (
              !element.classList.contains("visible") &&
              element.getBoundingClientRect().bottom < 0
            ) {
              element.classList.add("visible");
              observer.unobserve(element);
            }
          });
        });
      },
      { passive: true }
    );
  } else {
    // Very old browsers: no animation, show everything.
    revealed.forEach(function (element) {
      element.classList.add("visible");
    });
  }

  /* ---- 3. Published version --------------------------------------------- */

  // Returns the version of `senbit-userspace` found in an APT "Packages" file.
  function readVersion(packagesFile) {
    var stanzas = packagesFile.split("\n\n");

    for (var i = 0; i < stanzas.length; i++) {
      if (/^Package: senbit-userspace$/m.test(stanzas[i])) {
        var match = stanzas[i].match(/^Version: (\S+)$/m);
        // "0.4.0-1" is the package revision: show the Senbit version only.
        return match ? match[1].replace(/-\d+$/, "") : null;
      }
    }
    return null;
  }

  var targets = document.querySelectorAll("[data-version]");

  if (targets.length > 0 && window.fetch) {
    var controller = new AbortController();
    setTimeout(function () {
      controller.abort();
    }, 4000);

    fetch(PACKAGES_URL, { signal: controller.signal })
      .then(function (response) {
        return response.ok ? response.text() : "";
      })
      .then(function (text) {
        var version = readVersion(text);
        if (version) {
          targets.forEach(function (element) {
            element.textContent = version;
          });
        }
      })
      .catch(function () {
        // Offline or blocked: the version written in the page stays.
      });
  }
})();
