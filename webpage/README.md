# Senbit web site

Static site, deployed by Cloudflare Pages. No build step: the files in this folder are served as they are.

```text
webpage/
├── index.html              home
├── architecture/index.html layers, userspace, boot sequence
├── install/index.html      the installer
├── updates/index.html      apt updates, snapshots, rollback
├── roadmap/index.html      every phase
├── develop/index.html      devtool, building, QEMU
├── 404.html                page not found
├── assets/
│   ├── site.css            the only stylesheet
│   ├── site.js             menu, copy buttons, published version
│   └── logo.png, favicon.png
├── _headers                security and cache headers (Cloudflare)
└── robots.txt
```

## Cloudflare Pages settings

- Framework preset: **None**
- Build command: *(empty)*
- Build output directory: `webpage`

The repository must not contain nested Git repositories (submodules), otherwise Cloudflare stops
with `No url found for submodule path`. If that happens, run `./scripts/git-unify.sh`, commit, push.

## Working on it

- Open any page directly in a browser, or run `python3 -m http.server -d webpage` and visit
  `http://localhost:8000`.
- Pages share the same header and footer (copied in each file): when the menu changes, change it in
  every page.
- `site.js` reads the published version from the update repository
  (`dists/stable/main/binary-amd64/Packages`). If the request fails, the version written in the page
  is kept: update it by hand at each release.
- Keep the roadmap in `roadmap/index.html` in line with `docs/ROADMAP.md`.
