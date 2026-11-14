CC=gcc
CFLAGS+= -fPIC

SONAME:= libfoo-1.so
LSONAME:= libbar-1.so
devsymlink=$(shell echo $(1) | perl -pe 's:-[\d\.]*\.so$$:\.so:')
LIBFILES:=$(SONAME) $(LSONAME)

all: $(LIBFILES)

$(LIBFILES): code.o
	$(CC) $(LDFLAGS) -o $@ -shared -Wl,-soname,$@ $^ -lc

clean:
	rm -f *.a *.o *.so*

install: all
	install -m 0755 -d $(DESTDIR)/lib
	install -m 0755 -d $(DESTDIR)/usr/lib
	install -m 0755 -d $(DESTDIR)/usr/include
	install -m 0644 $(LSONAME) $(DESTDIR)/lib
	install -m 0644 $(SONAME) $(DESTDIR)/usr/lib
	ln -s /lib/$(LSONAME) $(DESTDIR)/usr/lib/$(call devsymlink,$(LSONAME))
	ln -s $(SONAME) $(DESTDIR)/usr/lib/$(call devsymlink,$(SONAME))
	install -m 0644 code.h $(DESTDIR)/usr/include

.PHONY: install clean
