CC=gcc

NOPICOBJS=non-pic.o
SONAME:=libbaz3.so.1

NOPIC_CFLAGS = $(CFLAGS) -fno-PIE
NOPIC_LDFLAGS = $(LDFLAGS) -fno-PIE -fno-pie

all: libbaz3.so.1.0.3b

# Non-PIC. We can't test this on all architectures
libbaz3.so.1.0.3b: $(NOPICOBJS)
	$(CC) $(NOPIC_LDFLAGS) -o $@ -shared -Wl,-soname,$(SONAME) $^ -lc

%.o: %.c
	$(CC) $(NOPIC_CFLAGS) $(CPPFLAGS) -o $@ -c $<

clean:
	rm -f *.a *.o *.so* *.sho

install: all
	install -m 0755 -d $(DESTDIR)/usr/lib
	install -m 0644 *.so* $(DESTDIR)/usr/lib
	ln -s libbaz3.so.1.0.3b $(DESTDIR)/usr/lib/$(SONAME)

.PHONY: install clean
