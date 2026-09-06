CC=gcc
CFLAGS+= -fPIC

SONAMES:= libpublic.so.1
LIBFILES:= $(patsubst %,%.0.1, $(SONAMES))
LIBDIR := /usr/lib/$(shell dpkg-architecture -qDEB_HOST_MULTIARCH)

all: $(LIBFILES)

$(LIBFILES): code.o
	$(CC) $(LDFLAGS) -o $@ -shared -Wl,-soname,$(patsubst %.0.1,%,$@) $^ -lc

clean:
	rm -f *.a *.o *.so*

install: all
	install -m 0755 -d $(DESTDIR)$(LIBDIR)
	install -m 0644 *.so* $(DESTDIR)$(LIBDIR)
	set -e ; for FILE in $(SONAMES) ; do \
	    ln -s $${FILE}.0.1 $(DESTDIR)$(LIBDIR)/$$FILE ; \
	    ln -s $${FILE}.0.1 $(DESTDIR)$(LIBDIR)/$$(echo $$FILE | perl -pe 's/\.so\K.*//') ; \
	done

.PHONY: install clean
