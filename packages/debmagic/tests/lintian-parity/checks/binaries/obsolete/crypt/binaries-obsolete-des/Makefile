# Makefile for the obsolete-des tests.

# glibc 2.28 and greater try to prevent new programs from being linked
# against the obsolete functions we're testing for.  This can be worked
# around, but we have to know the exact "symbol version" associated with
# the obsolete functions, which has to be dug out of libcrypt.so with nm.

LIBCRYPT_FILE := $(shell $(CC) $(CFLAGS) $(LDFLAGS) -print-file-name=libcrypt.so)

SYMVER := $(shell nm --dynamic --with-symbol-versions $(LIBCRYPT_FILE) | \
    grep ' setkey@' | cut -d@ -f2)

# The output of the above nm | grep | cut pipeline will be the empty string
# if it is possible to link programs against 'setkey' without special magic,
# and a nonempty string if special magic is required.  All five of the
# functions we are testing were introduced in the same release of glibc and
# disabled for new programs as a group in a single release of glibc, so it
# is only necessary to check how one of them is handled.
ifneq "$(SYMVER)" ""
SYMVER_DEFINE := -DSYMVER=\"$(SYMVER)\"
else
SYMVER_DEFINE := -USYMVER
endif

CFLAGS   := $(shell dpkg-buildflags --get CFLAGS)
CPPFLAGS := $(shell dpkg-buildflags --get CPPFLAGS) $(SYMVER_DEFINE)
LDFLAGS  := $(shell dpkg-buildflags --get LDFLAGS)

PROGRAMS := uses-fcrypt uses-encrypt uses-encrypt_r uses-setkey uses-setkey_r
MANPAGES := $(PROGRAMS:=.1)

all: $(PROGRAMS) $(MANPAGES)

uses-%: uses-%.o
	$(CC) $(CFLAGS) $(LDFLAGS) $< -o $@ -lcrypt

%.1: dummy.pod
	sed s/@NAME@/$(@F)/g < $< | \
	   pod2man --name $(@F) --section 1 > $@

install: all
	$(INSTALL) -d $(DESTDIR)/usr/bin
	$(INSTALL) -d $(DESTDIR)/usr/share/man/man1
	$(INSTALL) -m 755 $(PROGRAMS) $(DESTDIR)/usr/bin
	$(INSTALL) -m 644 $(MANPAGES) $(DESTDIR)/usr/share/man/man1

clean distclean:
	-rm -f *.o $(PROGRAMS) $(MANPAGES)

check test:

.PRECIOUS: $(PROGRAMS:=.o)
