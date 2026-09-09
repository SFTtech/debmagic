CC=gcc
MYCFLAGS=-Wall -Winline -O2 -fPIC
CFLAGS:=$(MYCFLAGS) $(CPPFLAGS) $(CFLAGS)

SONAMES:= libany.so.1 libany6gcc.so.1v6 libany5gcc.so.1 libanyQ16.so.6
LIBFILES:= $(patsubst %,%.0.1, $(SONAMES))

all: $(LIBFILES)

$(LIBFILES): code.o
	$(CC) $(LDFLAGS) -o $@ -shared -Wl,-soname,$(patsubst %.0.1,%,$@) $^ -lc

clean:
	rm -f *.a *.o *.so*

install: all
	install -m 0755 -d $(DESTDIR)/usr/lib
	install -m 0755 *.so* $(DESTDIR)/usr/lib
	for FILE in $(SONAMES) ; do \
	    ln -s $${FILE}.0.1 $(DESTDIR)/usr/lib/$$FILE ; \
	done

.PHONY: install clean
