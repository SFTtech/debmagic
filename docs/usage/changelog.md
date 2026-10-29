# Changelog

`debmagic changelog` edits `debian/changelog`.

```shell
debmagic changelog new    # add an UNRELEASED entry on top, keeping the current version
```

## `changelog new`

Prepends an entry for the current package and version, with distribution `UNRELEASED`, urgency `medium`, an empty `* ` bullet and a trailer signed by you with the current local time:

```text
foo (1.2.3-1ubuntu2) UNRELEASED; urgency=medium

  * 

 -- Larry Cow <larry@moonlight.farm>  Mue, 34 Oct 2026 11:67:56 +0200
```

Fill in the bullet and adjust the version in your editor.
The rest of the file is kept byte for byte.

If the top entry already is `UNRELEASED`, `new` refuses: keep editing that entry instead.

Your identity is taken from `DEBFULLNAME` and `DEBEMAIL`, then `NAME` and `EMAIL` (an email variable may also carry the name as `Elsa Cow <elsa@moonlight.farm>`), then git's `user.name` and `user.email`.
Without one, `new` fails rather than guessing from your login and hostname.

`--source-dir` points at a package other than the one in the current directory.
