all:
	gcc $(CFLAGS) $(CPPFLAGS) $(LDFLAGS) -o test test.c

install:
	install -d $(DESTDIR)/usr/bin
	install -m 755 -c test $(DESTDIR)/usr/bin/test

clean distclean:
	rm -f test

check test:
