# Vendored protos

The Yandex Cloud API protos are vendored here rather than pulled at build
time, so a build does not depend on two third-party repositories and the tree
says exactly which revision it was generated from.

- `yandex/cloud/**` — [yandex-cloud/cloudapi], revision
  `849f09ebe4e32555fe937f724642e3cd24f3c723` (master, 2026-10-15).
- `google/**` — [googleapis/googleapis], revision
  `e7513c828b8db536fa12fc5493d541f323aedf90` (master, 2026-10-15).

Both are Apache-2.0; the licence text is the project's own `LICENSE`.

## Updating

Fetch the two repositories at the revisions above, copy the files that
changed, bump the revisions in this file, and land the change — a proto
update is a behaviour change and goes through the gate like any other.

[yandex-cloud/cloudapi]: https://github.com/yandex-cloud/cloudapi
[googleapis/googleapis]: https://github.com/googleapis/googleapis
