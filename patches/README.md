# Senbit patches

Patches applied to the downloaded sources, by `devtool build`, in
alphabetical order:

```
patches/
  linux/        *.patch   (kernel)
  busybox/      *.patch
  util-linux/   *.patch
  parted/       *.patch
  systemd/      *.patch
```

* Format: unified diff, `-p1` (what `git diff` / `diff -ruN a/ b/` produce).
* Name them `NNNN-short-description.patch` (`0001-parted-fix-args.patch`) so the
  order is explicit.
* They live HERE (tracked by Git), not in `third_party/` (downloaded, ignored).
  The old location `third_party/patches/<name>/` is still read, but move your
  patches here.
* Applying twice is harmless: an already applied patch is detected and skipped.
* Changing a patch triggers a rebuild of that component.
* A patch that no longer applies stops the build with its file name: update it
  for the pinned version.

## Making a patch

```bash
./scripts/devtool get parted
cd third_party/parted
git init -q . && git add -A && git commit -qm base     # once, to diff against
# ... edit the sources ...
git diff > ../../patches/parted/0001-my-change.patch
```
