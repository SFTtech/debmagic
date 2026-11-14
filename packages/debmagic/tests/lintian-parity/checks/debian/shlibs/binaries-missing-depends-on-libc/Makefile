all:
	gcc $(CFLAGS) $(CPPFLAGS) $(LDFLAGS) -fPIC -shared -Wl,-z,defs -Wl,-soname,libbasic.so.2 -o libbasic.so.2 basic.c
	g++ $(CXXFLAGS) $(CPPFLAGS) $(LDFLAGS) -fPIC -shared -Wl,-z,defs \
             -Wl,-soname,libbasic++.so.2 -olibbasic++.so.2 cpp.cxx

install:
	install -d $(DESTDIR)/usr/lib/
	install -m 644 -c -s libbasic.so.2 $(DESTDIR)/usr/lib/libbasic.so.2
	install -m 644 -c -s libbasic++.so.2 $(DESTDIR)/usr/lib/libbasic++.so.2

clean distclean:
	rm -f libbasic.so.2 libbasic++.so.2

check test:
