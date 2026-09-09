# turn off PIE in CC in case we have a PIEful toolchain:
ifneq ($(findstring -no-pie,$(shell gcc -no-pie 2>&1)),)
  CC := gcc
else
  CC := gcc -fno-pie -no-pie
endif

LINKER_FLAGS := -Wl,--no-copy-dt-needed-entries,--no-as-needed

all: basic basic.1 libbasic-nodeps libbasic-nolibc

basic.1: basic.pod
	pod2man --section 1 $< > $@

basic: basic.o
	$(CC) -nostdlib -o$@ $< $(LINKER_FLAGS) -lm $(LFLAGS)

libbasic-nodeps: basic.o
	$(CC) -nostdlib --shared -o$@ $<

libbasic-nolibc: basic.o
	$(CC) -nostdlib --shared -o$@ $< $(LINKER_FLAGS) -lm

install:
	install -d $(DESTDIR)/usr/bin
	install -d $(DESTDIR)/usr/lib/basic
	install -d $(DESTDIR)/usr/share/man/man1
	install -m 755 -c basic $(DESTDIR)/usr/bin/basic
	install -m 755 -c libbasic-* $(DESTDIR)/usr/lib/basic
	install -m 644 -c basic.1 $(DESTDIR)/usr/share/man/man1

clean distclean:
	rm -f basic libbasic-* basic.1

check test:
