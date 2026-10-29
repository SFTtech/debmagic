NOPIE_CFLAGS = $(filter-out -fPIE,$(CFLAGS))
NOPIE_LDFLAGS = $(filter-out -fPIE -pie,$(LDFLAGS))
COMPILE:= $(CC) $(NOPIE_CFLAGS) $(CPPFLAGS) $(NOPIE_LDFLAGS)

all:
	# static version
	$(COMPILE) -std=c99 -static -o basic.static basic.c

install:
	install -d $(DESTDIR)/usr/lib/foo/
	install -m 755 basic.static $(DESTDIR)/usr/lib/foo/static

clean distclean:
	rm -f basic

check test:
