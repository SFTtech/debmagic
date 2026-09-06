all:
	gcc $(CFLAGS) $(CPPFLAGS) $(LDFLAGS) -fPIC -shared -Wl,-z,defs -Wl,-soname,libbasic.so.2 -o libbasic.so basic.c

VENDORARCH := $(shell perl -MConfig -wE'say substr($$Config{vendorarch},1)')

install:
	# install it under the correct triplet directory
	install -d $(DESTDIR)/$(VENDORARCH)
	install -m 644 -c -s libbasic.so $(DESTDIR)/$(VENDORARCH)/libbasic.so
	install -d $(DESTDIR)/usr/lib/php5
	install -m 644 -c -s libbasic.so $(DESTDIR)/usr/lib/php5/libbasic.so

clean distclean:
	rm -f libbasic.so.2

check test:
