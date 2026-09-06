CC=gcc
CFLAGS+= -g -flto=auto
LIBFILES:= libcodeless.a

all: $(LIBFILES)

libcodeless.a: code.o
	ar rcs -o $@ $^

clean:
	rm -f *.a *.o

install: all
	install -m 0755 -d $(DESTDIR)/usr/lib
	install -m 0644 *.a $(DESTDIR)/usr/lib

.PHONY: install clean
