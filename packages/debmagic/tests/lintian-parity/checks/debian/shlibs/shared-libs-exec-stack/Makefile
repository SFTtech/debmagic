CC=gcc
CFLAGS+= -fPIC

SONAMES:= libexecstack.so.1
LIBFILES:= $(patsubst %,%.0.1, $(SONAMES))

all: $(LIBFILES)

$(LIBFILES): foo.o get.o
	$(CC) $(LDFLAGS) -o $@ -shared -Wl,-soname,$(patsubst %.0.1,%,$@) $^

clean:
	rm -f *.a *.o *.so*

install: all
	install -m 0755 -d $(DESTDIR)/usr/lib
	install -m 0755 *.so* $(DESTDIR)/usr/lib
	for FILE in $(SONAMES) ; do \
	    ln -s $${FILE}.0.1 $(DESTDIR)/usr/lib/$$FILE ; \
	done

.PHONY: install clean
