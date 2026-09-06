CFLAGS += -fPIC

all: libhallo.so hallohelper

libhallo.so: code.o
	$(LINK.c) -o $@ -shared $^ -lc

hallohelper: main.o
	$(LINK.c) -o $@ -pie $^ -lc

clean:
	rm -f *.a *.o *.so *helper

install: all
	install -m 0755 -d $(DESTDIR)/usr/lib
	install -m 0644 *.so $(DESTDIR)/usr/lib
	install -m 0755 *helper $(DESTDIR)/usr/lib

.PHONY: install clean
