/*
 * Senbit web site: the only script. Three small, independent features:
 *   1. the menu button on small screens;
 *   2. the "Copy" buttons next to commands;
 *   3. the published version, read from the update repository.
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
