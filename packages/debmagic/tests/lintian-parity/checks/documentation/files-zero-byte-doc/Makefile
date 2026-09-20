ROOT = $(DESTDIR)/usr/share/doc/files-zero-byte-doc

default:
	:

install:
	mkdir -p $(ROOT)
	# Should be ignored.
	mkdir -p $(ROOT)/examples
	touch $(ROOT)/examples/empty
	touch $(ROOT)/__init__.py
	mkdir -p $(ROOT)/python/foo
	touch $(ROOT)/python/foo/__init__.py
	mkdir -p $(ROOT)/html
	touch $(ROOT)/html/blah.map
	mkdir -p $(ROOT)/manual/1/html/foo
	touch $(ROOT)/manual/1/html/foo/blah.map
	# Should be diagnosed.
	touch $(ROOT)/NEWS
	touch $(ROOT)/foo__init__.py
	touch $(ROOT)/html.map
	touch $(ROOT)/htmlfoo.map
	mkdir -p $(ROOT)/manual
	touch $(ROOT)/manual/foo.map
	gzip -n -9 $(ROOT)/manual/foo.map
	touch $(ROOT)/some-really-long-file-name-to-make-the-gzip-file-larger
	gzip -n -9 $(ROOT)/some-really-long-file-name-to-make-the-gzip-file-larger

.PHONY: clean distclean realclean install check test
