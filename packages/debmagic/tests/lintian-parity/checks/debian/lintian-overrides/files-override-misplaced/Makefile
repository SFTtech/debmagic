ROOT = $(DESTDIR)/usr/share/doc/files-override-misplaced

default:
	:

install:
	mkdir -p $(DESTDIR)/usr/share/doc/files-override-misplaced
	echo 'files-override-misplaced: some-override' \
	    > $(DESTDIR)/usr/share/doc/files-override-misplaced/override.lintian
	echo 'files-override-misplaced: some-override' \
	    > $(DESTDIR)/usr/share/doc/files-override-misplaced/override.Lintian
	gzip -n -9 $(DESTDIR)/usr/share/doc/files-override-misplaced/override.Lintian
	mkdir -p $(DESTDIR)/usr/share/lintian/overrides/files-override-misplaced
	touch $(DESTDIR)/usr/share/lintian/overrides/files-override-misplaced/some-override
	mkdir -p $(DESTDIR)/usr/share/lintian/overrides/wrong-package
	touch $(DESTDIR)/usr/share/lintian/overrides/wrong-package/some-override

.PHONY: clean distclean realclean install check test
