CC=gcc
CFLAGS+= -fPIC

SONAMES:= libfoo.so.1
LIBFILES:= $(patsubst %,%.0.1, $(SONAMES))

all: $(LIBFILES)

$(LIBFILES): code.o
	$(CC) $(LDFLAGS) -o $@ -shared -Wl,-soname,$(patsubst %.0.1,%,$@) $^ -lc

clean:
	rm -f *.a *.o *.so*

install: all
	install -m 0755 -d $(DESTDIR)/usr/lib
	install -m 0644 *.so* $(DESTDIR)/usr/lib
	install -T -m 0644 *.so* $(DESTDIR)/usr/lib/libfoo.so.1

.PHONY: install clean
