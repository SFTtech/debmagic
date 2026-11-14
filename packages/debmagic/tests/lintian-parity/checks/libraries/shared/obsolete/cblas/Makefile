TRIPLET := $(shell dpkg-architecture -qDEB_HOST_MULTIARCH)

all:
	gcc $(CFLAGS) $(CPPFLAGS) $(LDFLAGS) -fPIC -shared -Wl,-z,defs -Wl,-soname,libcblas.so.1 -o libcblas.so libcblas.c
	gcc $(CFLAGS) $(CPPFLAGS) $(LDFLAGS) -o consumer consumer.c -L. -lcblas

install:
	install -d $(DESTDIR)/usr/lib/$(TRIPLET)
	install -m 644 -c -s libcblas.so $(DESTDIR)/usr/lib/$(TRIPLET)/libcblas.so.1
	install -d $(DESTDIR)/usr/bin
	install -m 644 -c consumer $(DESTDIR)/usr/bin/

clean distclean:
	rm -f consumer
	rm -f libcblas.so

check test:
