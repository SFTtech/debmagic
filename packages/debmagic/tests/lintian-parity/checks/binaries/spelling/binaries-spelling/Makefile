all:
	gcc $(CFLAGS) $(CPPFLAGS) $(LDFLAGS) -o basic basic.c

install:
	install -d $(DESTDIR)/usr/bin
	install -m 755 -c basic $(DESTDIR)/usr/bin/basic

clean distclean:
	rm -f basic

check test:
