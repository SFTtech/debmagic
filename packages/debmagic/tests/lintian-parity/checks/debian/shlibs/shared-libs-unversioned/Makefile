CC = gcc

all: libfoo.so

libfoo.so: foo.o
	$(CC) $(LDFLAGS) -o $@ -shared -Wl,-soname,libfoo.so $^ -lc

%.o: %.c
	$(CC) $(CFLAGS) $(CPPFLAGS) -fPIC -o $@ -c $<

install:
	install -d $(DESTDIR)/usr/lib
	install -m644 libfoo.so $(DESTDIR)/usr/lib/

.PHONY: distclean realclean clean install test check
