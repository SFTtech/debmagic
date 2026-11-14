# turn off PIE in CC in case we have a PIEful toolchain:
ifneq ($(findstring -no-pie,$(shell gcc -no-pie 2>&1)),)
  CC := gcc
else
  CC := gcc -fno-pie -no-pie
endif

NOPIE_CFLAGS = $(filter-out -fPIE,$(CFLAGS))
NOPIE_LDFLAGS = $(filter-out -fPIE -pie,$(LDFLAGS))
COMPILE:= $(CC) $(CFLAGS) $(CPPFLAGS) $(LDFLAGS)
COMPILE_NOPIE:= $(CC) $(NOPIE_CFLAGS) $(CPPFLAGS) $(NOPIE_LDFLAGS)
# extract from readelf
GETBUILDID:=./getbuildid

all:
	# rpath not matching any of the exceptions to the rpath checks
	#  - with profiling enabled.
	$(COMPILE_NOPIE) -o basic basic.c -pg -Wl,--rpath,/usr/local/lib
	# rpath shipped in the package, but one of {/usr}?/lib
	$(COMPILE) -o basiclibrpath basic.c -Wl,--rpath,/usr/lib
	# non-special rpath shipped in the package
	$(COMPILE) -o basicshippedrpath basic.c -Wl,--rpath,/usr/share/foo
	# special rpath shipped in the package, multiple paths
	$(COMPILE) -o basicshippedrpathmore basic.c -Wl,--rpath,/usr/lib/binaries-general:/usr/lib/binaries-general/bar
	# static version of basic for debugging checks
	$(COMPILE_NOPIE) -static -o basic.static basic.c
	# static executable to trigger ocaml check
	$(COMPILE_NOPIE) -o ocaml-exec ocaml.c
	# version with debug
	$(COMPILE) -o basicdebug -g3 -Wl,--build-id basic.c

install:
	# according to local debian rules /usr/lib/debug is unstripped
	install -d $(DESTDIR)/usr/share/foo/
	install -d $(DESTDIR)/usr/lib/debug/usr/share/foo/
	install -d $(DESTDIR)/usr/lib/foo/
	install -d $(DESTDIR)/usr/bin

	install -m 755 -c basic $(DESTDIR)/usr/share/foo/basic
	objcopy --only-keep-debug basic $(DESTDIR)/usr/lib/debug/usr/share/foo/basic
	strip -s $(DESTDIR)/usr/lib/debug/usr/share/foo/basic
	install -m 755 -c basiclibrpath $(DESTDIR)/usr/lib/foo/basiclibrpath
	install -m 755 -c basicshippedrpath $(DESTDIR)/usr/lib/foo/basicshippedrpath
	install -m 755 -c ocaml-exec $(DESTDIR)/usr/lib/foo/ocaml-exec
	install -m 744 -c basicshippedrpathmore $(DESTDIR)/usr/lib/foo/basicshippedrpathmore
	objcopy --only-keep-debug basic $(DESTDIR)/usr/lib/debug/basic
	install -d "$(DESTDIR)/usr/lib/debug/.build-id/"`$(GETBUILDID) -s basicdebug`
	install -m 755 -c basicdebug $(DESTDIR)/usr/share/foo/basicdebug
	# force fake buildid in order to have tag matching ok (deadbeefdeadbeef)
	install -d "$(DESTDIR)/usr/lib/debug/.build-id/de"
	objcopy --compress-debug-sections basicdebug \
		"$(DESTDIR)/usr/lib/debug/.build-id/de/deadbeefdeadbeef.debug"
	install -d "$(DESTDIR)/usr/lib/debug/.build-id/"`$(GETBUILDID) -s basicdebug`
	objcopy --compress-debug-sections --only-keep-debug basicdebug \
		"$(DESTDIR)/usr/lib/debug/.build-id/"`$(GETBUILDID) -s basicdebug`"/"`$(GETBUILDID) -f basicdebug`.debug
	install -m 755 -c basic.static $(DESTDIR)/usr/lib/debug/
	# according to local debian rules unstripped in name avoid dh_strip to do the work
	install -m 755 basicdebug $(DESTDIR)/usr/bin/unstripped
	install -m 755 basic.static $(DESTDIR)/usr/bin/static

clean distclean:
	rm -f basic

check test:
